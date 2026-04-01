use super::Orchestrator;
use std::sync::Arc;

impl Orchestrator {
    pub(super) fn deliver_to_all_idle(self: &Arc<Self>) {
        loop {
            let idle_targets = match self.interactor.data_store.list_idle_or_waiting_workers() {
                Ok(workers) => workers,
                Err(e) => {
                    tracing::error!(error = %e, "delivery loop: failed to list idle workers");
                    break;
                }
            };

            if idle_targets.is_empty() {
                tracing::debug!("delivery loop: no idle/waiting workers");
                break;
            }

            let target_ids: Vec<_> = idle_targets.iter().map(|a| a.id.clone()).collect();
            tracing::debug!(
                targets = ?target_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                "delivery loop: attempting delivery"
            );

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
                tracing::debug!("delivery loop: no messages delivered, exiting");
                break;
            }
        }
    }
}
