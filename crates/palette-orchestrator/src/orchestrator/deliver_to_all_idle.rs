use super::Orchestrator;
use palette_domain::agent::{AgentId, AgentStatus};
use std::sync::Arc;

impl Orchestrator {
    pub(super) async fn deliver_to_all_idle(self: &Arc<Self>) {
        loop {
            let mut infra = self.infra.lock().await;
            let idle_targets: Vec<AgentId> = infra
                .supervisors
                .iter()
                .chain(infra.members.iter())
                .filter(|m| m.status == AgentStatus::Idle)
                .map(|m| m.id.clone())
                .collect();

            let mut any_delivered = false;
            for target_id in &idle_targets {
                match self.deliver_queued_messages(target_id, &mut infra) {
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
