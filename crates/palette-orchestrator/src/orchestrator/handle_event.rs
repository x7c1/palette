use super::Orchestrator;
use palette_domain::rule::RuleEffect;
use palette_domain::server::ServerEvent;
use std::sync::Arc;

impl Orchestrator {
    pub(super) async fn handle_event(self: &Arc<Self>, event: ServerEvent) {
        match event {
            ServerEvent::ProcessEffects { effects } => self.handle_process_effects(effects).await,
            ServerEvent::DeliverMessages { target_id } => {
                let _ = self.deliver_queued_messages(&target_id);
            }
            ServerEvent::NotifyDeliveryLoop => self.deliver_to_all_idle(),
            ServerEvent::ResumeWorkers { worker_ids } => {
                for worker_id in worker_ids {
                    self.spawn_readiness_watcher(worker_id);
                }
                // Re-assign jobs that were deferred during suspend.
                // Delayed to give workers time to boot and become ready.
                let this = Arc::clone(self);
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                    this.assign_deferred_jobs();
                });
            }
            ServerEvent::SuspendWorkflow => {
                let this = Arc::clone(self);
                tokio::task::spawn_blocking(move || this.suspend());
            }
        }
    }

    async fn handle_process_effects(self: &Arc<Self>, effects: Vec<RuleEffect>) {
        let result = match self.process_effects(&effects) {
            Ok(result) => result,
            Err(e) => {
                tracing::error!(error = %e, "failed to process effects");
                return;
            }
        };

        for d in &result.deliveries {
            let _ = self.deliver_queued_messages(&d.target_id);
        }

        for d in result.deliveries {
            self.spawn_readiness_watcher(d.target_id);
        }
        for sup_id in result.spawned_supervisors {
            self.spawn_readiness_watcher(sup_id);
        }
    }
}
