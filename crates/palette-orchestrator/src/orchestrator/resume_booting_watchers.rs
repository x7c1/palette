use super::Orchestrator;
use palette_domain::agent::AgentStatus;
use palette_domain::server::PersistentState;
use std::sync::Arc;

impl Orchestrator {
    /// Start readiness watchers for any agents currently in Booting state.
    pub fn resume_booting_watchers(self: &Arc<Self>, infra: &PersistentState) {
        for supervisor in &infra.supervisors {
            if supervisor.status == AgentStatus::Booting {
                self.spawn_readiness_watcher(supervisor.id.clone());
            }
        }
        for member in &infra.members {
            if member.status == AgentStatus::Booting {
                self.spawn_readiness_watcher(member.id.clone());
            }
        }
    }
}
