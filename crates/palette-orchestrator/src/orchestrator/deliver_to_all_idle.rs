use super::Orchestrator;
use palette_domain::agent::{AgentId, AgentStatus};
use std::sync::Arc;

use crate::deliver_queued_messages;

impl Orchestrator {
    pub(super) async fn deliver_to_all_idle(self: &Arc<Self>) {
        loop {
            let mut infra = self.infra.lock().await;
            let idle_targets: Vec<AgentId> = infra
                .leaders
                .iter()
                .chain(infra.members.iter())
                .filter(|m| m.status == AgentStatus::Idle)
                .map(|m| m.id.clone())
                .collect();

            let mut any_delivered = false;
            for target_id in &idle_targets {
                match deliver_queued_messages(target_id, &self.db, &mut infra, &self.tmux) {
                    Ok(true) => any_delivered = true,
                    Ok(false) => {}
                    Err(e) => {
                        tracing::error!(
                            target_id = %target_id,
                            error = %e,
                            "delivery loop: failed to deliver"
                        );
                    }
                }
            }
            if !any_delivered {
                break;
            }
        }
    }
}
