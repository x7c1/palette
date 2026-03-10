use super::Orchestrator;
use palette_domain::server::ServerEvent;
use std::sync::Arc;

use crate::{deliver_queued_messages, process_effects};

impl Orchestrator {
    pub(super) async fn handle_event(self: &Arc<Self>, event: ServerEvent) {
        match event {
            ServerEvent::ProcessEffects { effects } => {
                let mut infra = self.infra.lock().await;
                match process_effects(
                    &effects,
                    &self.db,
                    &mut infra,
                    &self.docker,
                    &self.tmux,
                    &self.docker_config,
                ) {
                    Ok(deliveries) => {
                        for d in &deliveries {
                            let _ = deliver_queued_messages(
                                &d.target_id,
                                &self.db,
                                &mut infra,
                                &self.tmux,
                            );
                        }
                        self.save_state(&infra);
                        drop(infra);
                        for d in deliveries {
                            self.spawn_readiness_watcher(d.target_id);
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to process effects");
                    }
                }
            }
            ServerEvent::DeliverMessages { target_id } => {
                let mut infra = self.infra.lock().await;
                let _ = deliver_queued_messages(&target_id, &self.db, &mut infra, &self.tmux);
            }
            ServerEvent::NotifyDeliveryLoop => {
                self.deliver_to_all_idle().await;
            }
        }
    }
}
