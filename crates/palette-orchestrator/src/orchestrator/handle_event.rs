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
