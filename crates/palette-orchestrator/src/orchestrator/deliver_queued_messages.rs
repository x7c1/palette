use super::Orchestrator;
use palette_domain::agent::{AgentId, AgentStatus};
use palette_domain::server::PersistentState;

impl Orchestrator {
    /// Delivers queued messages to idle targets.
    pub(super) fn deliver_queued_messages(
        &self,
        target_id: &AgentId,
        infra: &mut PersistentState,
    ) -> crate::Result<bool> {
        let member = infra
            .find_member(target_id)
            .or_else(|| infra.find_leader(target_id));

        let terminal_target = match member {
            Some(m) if m.status == AgentStatus::Idle => m.terminal_target.clone(),
            _ => return Ok(false),
        };

        if let Some(msg) = self.db.dequeue_message(target_id)? {
            self.tmux.send_keys(&terminal_target, &msg.message)?;
            // Update status to Working
            if let Some(m) = infra.find_member_mut(target_id) {
                m.status = AgentStatus::Working;
            } else if let Some(l) = infra.find_leader_mut(target_id) {
                l.status = AgentStatus::Working;
            }
            infra.touch();
            tracing::info!(target_id = %target_id, "delivered queued message");
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
