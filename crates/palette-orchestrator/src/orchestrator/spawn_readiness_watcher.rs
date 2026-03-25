use super::Orchestrator;
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
            for _ in 0..max_polls {
                tokio::time::sleep(READINESS_POLL_INTERVAL).await;
                if this.poll_readiness(&target_id).is_break() {
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
    fn poll_readiness(self: &Arc<Self>, target_id: &WorkerId) -> ControlFlow<()> {
        let terminal_target = {
            let worker = match self.db.find_worker(target_id) {
                Ok(Some(w)) => w,
                _ => return ControlFlow::Break(()),
            };
            worker.terminal_target.clone()
        };

        let pane_content = match self.tmux.capture_pane(&terminal_target) {
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

        if !pane_content.contains('❯') {
            return ControlFlow::Continue(());
        }

        tracing::info!(
            target_id = %target_id,
            "Claude Code is ready, delivering queued message"
        );
        self.activate_worker(target_id);
        ControlFlow::Break(())
    }

    /// Transition a booting worker to Idle and deliver queued messages.
    fn activate_worker(self: &Arc<Self>, target_id: &WorkerId) {
        let is_booting = self
            .db
            .find_worker(target_id)
            .ok()
            .flatten()
            .is_some_and(|a| a.status == WorkerStatus::Booting);

        if is_booting && let Err(e) = self.db.update_worker_status(target_id, WorkerStatus::Idle) {
            tracing::error!(error = %e, target_id = %target_id, "failed to update worker status to idle");
        }
        let _ = self.deliver_queued_messages(target_id);
    }
}
