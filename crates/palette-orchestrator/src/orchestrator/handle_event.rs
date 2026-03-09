use super::Orchestrator;
use palette_domain::ServerEvent;
use std::sync::Arc;

use crate::{deliver_queued_messages, process_effects};

impl Orchestrator {
    pub(super) async fn handle_event(this: &Arc<Self>, event: ServerEvent) {
        match event {
            ServerEvent::ProcessEffects { effects } => {
                let mut infra = this.infra.lock().await;
                match process_effects(
                    &effects,
                    &this.db,
                    &mut infra,
                    &this.docker,
                    &*this.tmux,
                    &this.docker_config,
                ) {
                    Ok(deliveries) => {
                        for d in &deliveries {
                            let _ = deliver_queued_messages(
                                &d.target_id,
                                &this.db,
                                &mut infra,
                                &*this.tmux,
                            );
                        }
                        Self::save_state(this, &infra);
                        drop(infra);
                        for d in deliveries {
                            Self::spawn_readiness_watcher(this, d.target_id);
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to process effects");
                    }
                }
            }
            ServerEvent::DeliverMessages { target_id } => {
                let mut infra = this.infra.lock().await;
                let _ = deliver_queued_messages(&target_id, &this.db, &mut infra, &*this.tmux);
            }
            ServerEvent::NotifyDeliveryLoop => {
                Self::deliver_to_all_idle(this).await;
            }
        }
    }
}
