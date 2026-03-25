use super::Orchestrator;
use palette_domain::agent::{AgentId, AgentStatus};

impl Orchestrator {
    /// Delivers queued messages to idle targets.
    pub(super) fn deliver_queued_messages(&self, target_id: &AgentId) -> crate::Result<bool> {
        let agent = self.db.find_agent(target_id)?;

        let terminal_target = match agent {
            Some(ref m) if m.status == AgentStatus::Idle => m.terminal_target.clone(),
            _ => return Ok(false),
        };

        if let Some(msg) = self.db.dequeue_message(target_id)? {
            self.tmux.send_keys(&terminal_target, &msg.message)?;
            // Update status to Working
            self.db
                .update_agent_status(target_id, AgentStatus::Working)?;
            tracing::info!(target_id = %target_id, "delivered queued message");
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
