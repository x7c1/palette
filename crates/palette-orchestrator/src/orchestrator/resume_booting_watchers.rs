use super::Orchestrator;
use std::sync::Arc;

impl Orchestrator {
    /// Start readiness watchers for any workers currently in Booting state.
    pub fn resume_booting_watchers(self: &Arc<Self>) {
        let booting = match self.interactor.data_store.list_booting_workers() {
            Ok(workers) => workers,
            Err(e) => {
                tracing::error!(error = %e, "failed to list booting workers");
                return;
            }
        };
        for worker in booting {
            self.spawn_readiness_watcher(worker.id);
        }
    }
}
