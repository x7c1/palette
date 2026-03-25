use super::Orchestrator;
use std::sync::Arc;

impl Orchestrator {
    pub(super) fn deliver_to_all_idle(self: &Arc<Self>) {
        loop {
            let idle_targets = match self.db.list_idle_or_waiting_agents() {
                Ok(agents) => agents,
                Err(e) => {
                    tracing::error!(error = %e, "delivery loop: failed to list idle agents");
                    break;
                }
            };

            let target_ids: Vec<_> = idle_targets.iter().map(|a| a.id.clone()).collect();

            let mut any_delivered = false;
            for target_id in &target_ids {
                match self.deliver_queued_messages(target_id) {
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
