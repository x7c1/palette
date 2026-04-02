mod activate_review;
mod activate_task;
mod assign_new_job;
mod complete_task;
mod destroy_member;
mod handle_event;
pub(crate) mod job_instruction;
mod orchestrator_task;
mod reactivate_member;
mod review_verdict;
mod validate_artifacts;
mod workflow_activation;

use super::Orchestrator;
use palette_domain::worker::WorkerId;

/// Follow-up actions that the orchestrator event loop should perform
/// after an event handler completes.
///
/// Returned by handler methods; dispatched by `dispatch_pending_actions`.
pub(in crate::orchestrator) struct PendingActions {
    /// Workers to watch for readiness and deliver queued messages to.
    pub deliver_to: Vec<WorkerId>,
    /// Workers to watch for readiness only (no immediate messages).
    pub watch_only: Vec<WorkerId>,
}

impl PendingActions {
    pub fn new() -> Self {
        Self {
            deliver_to: Vec::new(),
            watch_only: Vec::new(),
        }
    }

    /// Combine two results into one.
    pub fn merge(mut self, other: PendingActions) -> Self {
        self.deliver_to.extend(other.deliver_to);
        self.watch_only.extend(other.watch_only);
        self
    }
}
