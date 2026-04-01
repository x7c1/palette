use super::Orchestrator;
use palette_domain::worker::{WorkerId, WorkerStatus};
use palette_domain::workflow::WorkflowStatus;
use std::sync::Arc;

impl Orchestrator {
    /// Delivers queued messages to idle targets.
    /// Skipped when the worker's workflow is suspending (no new work during suspend).
    ///
    /// Before sending, verifies that Claude Code is actually running in the
    /// pane (`❯` prompt present). If Claude Code has exited, sends a fresh
    /// start command and spawns a readiness watcher instead of delivering.
    pub(super) fn deliver_queued_messages(
        self: &Arc<Self>,
        target_id: &WorkerId,
    ) -> crate::Result<bool> {
        let worker = self.interactor.data_store.find_worker(target_id)?;

        let worker = match worker {
            Some(ref m) if m.status == WorkerStatus::Idle => m,
            Some(ref m) => {
                tracing::debug!(
                    target_id = %target_id,
                    status = ?m.status,
                    "delivery skipped: worker not idle"
                );
                return Ok(false);
            }
            None => {
                tracing::debug!(target_id = %target_id, "delivery skipped: worker not found");
                return Ok(false);
            }
        };

        // During suspend, block delivery to members (prevents new work) but
        // allow delivery to supervisors — they must stay active to approve
        // permission prompts so in-progress members can finish.
        if !worker.role.is_supervisor() && self.is_workflow_suspending(&worker.workflow_id)? {
            tracing::debug!(target_id = %target_id, "delivery skipped: workflow suspending");
            return Ok(false);
        }

        let terminal_target = worker.terminal_target.clone();
        let container_id = worker.container_id.clone();
        let role = worker.role;

        // Verify Claude Code is actually running before delivering.
        // The DB may say Idle but Claude Code could have exited (e.g.
        // --resume succeeded briefly then quit). Delivering to a bare
        // shell would cause the message to be interpreted as commands.
        if !self.interactor.container.is_claude_running(&container_id) {
            tracing::warn!(
                target_id = %target_id,
                "Claude Code not running, restarting before delivery"
            );
            let cmd = self.interactor.container.claude_exec_command(
                &container_id,
                "/home/agent/prompt.md",
                role,
                None,
            );
            let _ = self.interactor.terminal.send_keys(&terminal_target, &cmd);
            self.interactor
                .data_store
                .update_worker_status(target_id, WorkerStatus::Booting)?;
            self.spawn_readiness_watcher(target_id.clone());
            return Ok(false);
        }

        if let Some(msg) = self.interactor.data_store.dequeue_message(target_id)? {
            self.interactor.terminal.send_keys(&terminal_target, &msg)?;
            self.interactor
                .data_store
                .update_worker_status(target_id, WorkerStatus::Working)?;
            tracing::info!(target_id = %target_id, "delivered queued message");
            Ok(true)
        } else {
            tracing::debug!(target_id = %target_id, "delivery skipped: no queued messages");
            Ok(false)
        }
    }

    /// Check whether the given workflow is in Suspending state.
    pub(super) fn is_workflow_suspending(
        &self,
        workflow_id: &palette_domain::workflow::WorkflowId,
    ) -> crate::Result<bool> {
        let workflow = self.interactor.data_store.get_workflow(workflow_id)?;
        Ok(workflow.is_some_and(|w| w.status == WorkflowStatus::Suspending))
    }
}
