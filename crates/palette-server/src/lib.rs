mod routes;

use axum::Router;
use palette_core::DockerConfig;
use palette_core::docker::DockerManager;
use palette_core::orchestrator;
use palette_core::state::PersistentState;
use palette_db::{Database, RuleEngine};
use palette_tmux::{TmuxManager as _, TmuxManagerImpl};
use std::sync::Arc;

pub struct AppState {
    pub tmux: TmuxManagerImpl,
    pub db: Database,
    pub rules: RuleEngine,
    pub docker: DockerManager,
    pub docker_config: DockerConfig,
    pub infra: tokio::sync::Mutex<PersistentState>,
    pub state_path: String,
    pub event_log: tokio::sync::Mutex<Vec<EventRecord>>,
    /// Notifies the delivery loop that there may be Idle workers with pending messages.
    pub delivery_notify: tokio::sync::Notify,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EventRecord {
    pub timestamp: String,
    pub event_type: String,
    pub payload: serde_json::Value,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    routes::create_router(state)
}

/// Spawn a background loop that delivers queued messages to Idle workers.
pub fn spawn_delivery_loop(state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            state.delivery_notify.notified().await;

            // Drain: keep delivering until no more Idle workers have pending messages
            loop {
                let delivered = {
                    let mut infra = state.infra.lock().await;

                    // Collect Idle targets (leaders + members)
                    let idle_targets: Vec<String> = infra
                        .leaders
                        .iter()
                        .chain(infra.members.iter())
                        .filter(|m| m.status == palette_core::state::MemberStatus::Idle)
                        .map(|m| m.id.clone())
                        .collect();

                    let mut any_delivered = false;
                    for target_id in &idle_targets {
                        match orchestrator::deliver_queued_messages(
                            target_id,
                            &state.db,
                            &mut infra,
                            &state.tmux,
                        ) {
                            Ok(true) => any_delivered = true,
                            Ok(false) => {}
                            Err(e) => {
                                tracing::error!(
                                    target_id = target_id,
                                    error = %e,
                                    "delivery loop: failed to deliver"
                                );
                            }
                        }
                    }
                    any_delivered
                };

                if !delivered {
                    break;
                }
            }
        }
    });
}
