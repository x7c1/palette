use super::Orchestrator;
use palette_domain::job::JobFilter;
use palette_domain::worker::{WorkerId, WorkerStatus};
use std::ops::ControlFlow;
use std::sync::Arc;

/// Interval between readiness polls.
const READINESS_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3);

/// Maximum time to wait for Claude Code readiness.
const READINESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

impl Orchestrator {
    pub(super) fn spawn_readiness_watcher(self: &Arc<Self>, target_id: WorkerId) {
        let this = Arc::clone(self);
        let max_polls = READINESS_TIMEOUT.as_secs() / READINESS_POLL_INTERVAL.as_secs();

        tokio::spawn(async move {
            let mut fallback_sent = false;
            for _ in 0..max_polls {
                tokio::time::sleep(READINESS_POLL_INTERVAL).await;
                if this
                    .poll_readiness(&target_id, &mut fallback_sent)
                    .is_break()
                {
                    return;
                }
            }
            tracing::error!(
                target_id = %target_id,
                "timed out waiting for Claude Code readiness"
            );
        });
    }

    /// Poll once for worker readiness.
    /// Returns `Break` if the worker is ready (activated) or gone, `Continue` to keep polling.
    ///
    /// If the pane shows a shell prompt instead of `❯`, Claude Code has exited
    /// (e.g. `--resume` failed with "No conversation found"). In that case,
    /// send a fresh-start command as a one-time fallback.
    fn poll_readiness(
        self: &Arc<Self>,
        target_id: &WorkerId,
        fallback_sent: &mut bool,
    ) -> ControlFlow<()> {
        let worker = match self.interactor.data_store.find_worker(target_id) {
            Ok(Some(w)) => w,
            _ => return ControlFlow::Break(()),
        };

        let pane_content = match self
            .interactor
            .terminal
            .capture_pane(&worker.terminal_target)
        {
            Ok(content) => content,
            Err(e) => {
                tracing::warn!(
                    target_id = %target_id,
                    error = %e,
                    "failed to capture pane"
                );
                return ControlFlow::Continue(());
            }
        };

        if pane_content.contains('❯') {
            tracing::info!(
                target_id = %target_id,
                "Claude Code is ready, delivering queued message"
            );
            self.activate_worker(target_id);
            return ControlFlow::Break(());
        }

        // Detect Claude Code exit (e.g. `--resume` failed with "No
        // conversation found"). Fall back to a fresh start once.
        if !*fallback_sent
            && !self
                .interactor
                .container
                .is_claude_running(&worker.container_id)
        {
            tracing::warn!(
                target_id = %target_id,
                "Claude Code not running, falling back to fresh start"
            );
            let cmd = self.interactor.container.claude_exec_command(
                &worker.container_id,
                "/home/agent/prompt.md",
                worker.role,
            );
            if let Err(e) = self
                .interactor
                .terminal
                .send_keys(&worker.terminal_target, &cmd)
            {
                tracing::error!(
                    target_id = %target_id,
                    error = %e,
                    "failed to send fresh-start fallback command"
                );
            }
            *fallback_sent = true;
        }

        ControlFlow::Continue(())
    }

    /// Transition a booting worker to Idle and deliver queued messages.
    /// If no messages are queued but the worker has an in-progress job
    /// (resume after suspend), send a continuation prompt.
    fn activate_worker(self: &Arc<Self>, target_id: &WorkerId) {
        let is_booting = match self.interactor.data_store.find_worker(target_id) {
            Ok(Some(w)) => w.status == WorkerStatus::Booting,
            Ok(None) => false,
            Err(e) => {
                tracing::error!(error = %e, target_id = %target_id, "failed to find worker during activation");
                false
            }
        };

        if is_booting
            && let Err(e) = self
                .interactor
                .data_store
                .update_worker_status(target_id, WorkerStatus::Idle)
        {
            tracing::error!(error = %e, target_id = %target_id, "failed to update worker status to idle");
        }

        let delivered = match self.deliver_queued_messages(target_id) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(error = %e, target_id = %target_id, "failed to deliver queued messages during activation");
                false
            }
        };
        if !delivered {
            self.nudge_resumed_worker(target_id);
        }
    }

    /// If the worker has an in-progress job (i.e. it was suspended mid-work),
    /// send a continuation prompt so Claude Code resumes working.
    fn nudge_resumed_worker(self: &Arc<Self>, target_id: &WorkerId) {
        let jobs = match self.interactor.data_store.list_jobs(&JobFilter {
            assignee_id: Some(target_id.clone()),
            ..Default::default()
        }) {
            Ok(jobs) => jobs,
            Err(e) => {
                tracing::error!(error = %e, target_id = %target_id, "failed to list jobs for resume nudge");
                return;
            }
        };

        let has_in_progress = jobs.iter().any(|j| j.status.is_in_progress());
        if !has_in_progress {
            return;
        }

        tracing::info!(target_id = %target_id, "nudging resumed worker to continue in-progress job");

        let msg = "[system] Session resumed after suspend. Continue your current task.";
        if let Err(e) = self.interactor.data_store.enqueue_message(target_id, msg) {
            tracing::error!(error = %e, target_id = %target_id, "failed to enqueue resume nudge");
            return;
        }
        let _ = self.deliver_queued_messages(target_id);
    }
}
