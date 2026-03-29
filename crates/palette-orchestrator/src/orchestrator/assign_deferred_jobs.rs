use super::Orchestrator;
use palette_domain::rule::RuleEffect;
use std::sync::Arc;

impl Orchestrator {
    /// Re-assign jobs that were deferred during suspend.
    ///
    /// During `Suspending`, `assign_new_job` skips job assignment to avoid
    /// spawning new members. After resume, these jobs remain in `Todo` with
    /// no assignee. This method finds them and fires `AssignNewJob` effects.
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

        let effects: Vec<_> = assignable
            .iter()
            .map(|j| RuleEffect::AssignNewJob {
                job_id: j.id.clone(),
            })
            .collect();

        let result = match self.process_effects(&effects) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(error = %e, "failed to process deferred job assignments");
                return;
            }
        };

        for d in &result.deliveries {
            let _ = self.deliver_queued_messages(&d.target_id);
        }
        for d in result.deliveries {
            self.spawn_readiness_watcher(d.target_id);
        }
        for sup_id in result.spawned_supervisors {
            self.spawn_readiness_watcher(sup_id);
        }
    }
}
