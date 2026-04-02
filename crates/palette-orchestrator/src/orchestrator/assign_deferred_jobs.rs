use super::Orchestrator;
use std::sync::Arc;

impl Orchestrator {
    /// Re-assign jobs that were deferred during suspend.
    ///
    /// During `Suspending`, `assign_new_job` skips job assignment to avoid
    /// spawning new members. After resume, these jobs remain in `Todo` with
    /// no assignee. This method finds them and fires assignments.
    pub(super) fn assign_deferred_jobs(self: &Arc<Self>) {
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

        let mut deliveries = Vec::new();
        for job in &assignable {
            if let Err(e) = self.assign_new_job(&job.id, &mut deliveries) {
                tracing::error!(error = %e, job_id = %job.id, "failed to assign deferred job");
            }
        }

        for d in &deliveries {
            let _ = self.deliver_queued_messages(&d.target_id);
        }
        for d in deliveries {
            self.spawn_readiness_watcher(d.target_id);
        }
    }
}
