use super::Orchestrator;
use std::sync::Arc;

impl Orchestrator {
    /// Start readiness watchers for any agents currently in Booting state.
    pub fn resume_booting_watchers(self: &Arc<Self>) {
        let booting = match self.db.list_booting_agents() {
            Ok(agents) => agents,
            Err(e) => {
                tracing::error!(error = %e, "failed to list booting agents");
                return;
            }
        };
        for agent in booting {
            self.spawn_readiness_watcher(agent.id);
        }
    }
}
