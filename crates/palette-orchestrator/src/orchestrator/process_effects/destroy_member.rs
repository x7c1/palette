use super::Orchestrator;
use palette_domain::agent::AgentId;
use palette_domain::server::PersistentState;

impl Orchestrator {
    pub(super) fn destroy_member(&self, member_id: &AgentId, infra: &mut PersistentState) {
        if let Some(member) = infra.remove_member(member_id) {
            tracing::info!(member_id = %member_id, "destroying member container");
            let _ = self.docker.stop_container(&member.container_id);
            let _ = self.docker.remove_container(&member.container_id);
            infra.touch();
        }
    }
}
