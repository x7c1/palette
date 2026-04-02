use super::Orchestrator;
use crate::orchestrator::handler::PendingActions;
use std::sync::Arc;

impl Orchestrator {
    /// Re-assign jobs that were deferred during suspend.
    ///
    /// During `Suspending`, `assign_new_job` skips job assignment to avoid
    /// spawning new members. After resume, these jobs remain in `Todo` with
    /// no assignee. This method finds them and fires assignments.
    pub(in crate::orchestrator) fn assign_deferred_jobs(self: &Arc<Self>) {
        let assignable = match self.interactor.data_store.find_assignable_jobs() {
            Ok(jobs) => jobs,
            Err(e) => {
                tracing::error!(error = %e, "failed to find assignable jobs for deferred assignment");
                return;
            }
        };

        if assignable.is_empty() {
            return;
        }

        tracing::info!(
            count = assignable.len(),
            "re-assigning jobs deferred during suspend"
        );

        let mut result = PendingActions::new();
        for job in &assignable {
            match self.assign_new_job(&job.id) {
                Ok(r) => result = result.merge(r),
                Err(e) => {
                    tracing::error!(error = %e, job_id = %job.id, "failed to assign deferred job")
                }
            }
        }

        self.dispatch_pending_actions(result);
    }
}
