use super::Orchestrator;
use palette_domain::agent::AgentId;

impl Orchestrator {
    pub(super) fn destroy_member(&self, member_id: &AgentId) {
        if let Ok(Some(member)) = self.db.remove_agent(member_id) {
            tracing::info!(member_id = %member_id, "destroying member container");
            let _ = self.docker.stop_container(&member.container_id);
            let _ = self.docker.remove_container(&member.container_id);
        }
    }
}
