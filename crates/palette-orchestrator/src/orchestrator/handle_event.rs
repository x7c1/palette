use super::Orchestrator;
use palette_domain::rule::RuleEffect;
use palette_domain::server::ServerEvent;
use std::sync::Arc;

impl Orchestrator {
    pub(super) async fn handle_event(self: &Arc<Self>, event: ServerEvent) {
        match event {
            ServerEvent::ProcessEffects { effects } => self.handle_process_effects(effects).await,
            ServerEvent::DeliverMessages { target_id } => {
                let mut infra = self.infra.lock().await;
                let _ = self.deliver_queued_messages(&target_id, &mut infra);
            }
            ServerEvent::NotifyDeliveryLoop => self.deliver_to_all_idle().await,
        }
    }

    async fn handle_process_effects(self: &Arc<Self>, effects: Vec<RuleEffect>) {
        let mut infra = self.infra.lock().await;
        let deliveries = match self.process_effects(&effects, &mut infra) {
            Ok(deliveries) => deliveries,
            Err(e) => {
                tracing::error!(error = %e, "failed to process effects");
                return;
            }
        };

        for d in &deliveries {
            let _ = self.deliver_queued_messages(&d.target_id, &mut infra);
        }
        self.save_state(&infra);
        drop(infra);

        for d in deliveries {
            self.spawn_readiness_watcher(d.target_id);
        }
    }
}
