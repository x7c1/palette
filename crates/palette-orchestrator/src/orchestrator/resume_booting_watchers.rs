use super::Orchestrator;
use palette_domain::{AgentStatus, PersistentState};
use std::sync::Arc;

impl Orchestrator {
    /// Start readiness watchers for any agents currently in Booting state.
    pub fn resume_booting_watchers(this: &Arc<Self>, infra: &PersistentState) {
        for leader in &infra.leaders {
            if leader.status == AgentStatus::Booting {
                Self::spawn_readiness_watcher(this, leader.id.clone());
            }
        }
        for member in &infra.members {
            if member.status == AgentStatus::Booting {
                Self::spawn_readiness_watcher(this, member.id.clone());
            }
        }
    }
}
