mod activate_review;
mod assign_new_job;
mod complete_job;
mod destroy_member;
pub(crate) mod job_instruction;
mod reactivate_member;
mod review_verdict;
mod workflow_activation;

use super::Orchestrator;
use palette_domain::server::PendingDelivery;
use palette_domain::worker::WorkerId;

/// Accumulated results from direct effect execution.
///
/// Returned by orchestrator methods to describe what follow-up actions
/// (message delivery, readiness watchers) are needed. The caller
/// dispatches these via `dispatch_effect_result`.
pub(in crate::orchestrator) struct EffectResult {
    pub deliveries: Vec<PendingDelivery>,
    pub spawned_supervisors: Vec<WorkerId>,
}

impl EffectResult {
    pub fn new() -> Self {
        Self {
            deliveries: Vec::new(),
            spawned_supervisors: Vec::new(),
        }
    }

    /// Combine two results into one.
    pub fn merge(mut self, other: EffectResult) -> Self {
        self.deliveries.extend(other.deliveries);
        self.spawned_supervisors.extend(other.spawned_supervisors);
        self
    }
}
