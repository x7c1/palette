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

/// Spawn a background task that polls a tmux pane for Claude Code readiness (`❯` prompt),
/// then transitions the target from Booting to Idle and delivers queued messages.
pub fn spawn_readiness_watcher(target_id: String, tmux_target: String, state: Arc<AppState>) {
    use palette_core::state::MemberStatus;

    tokio::spawn(async move {
        // Poll every 3 seconds for up to 120 seconds
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            let pane_content = match state.tmux.capture_pane(&tmux_target) {
                Ok(content) => content,
                Err(e) => {
                    tracing::warn!(target_id, error = %e, "failed to capture pane");
                    continue;
                }
            };

            if !pane_content.contains('❯') {
                continue;
            }

            tracing::info!(target_id, "Claude Code is ready, delivering queued message");

            {
                let mut infra = state.infra.lock().await;
                let is_booting = infra
                    .find_member(&target_id)
                    .or_else(|| infra.find_leader(&target_id))
                    .is_some_and(|m| m.status == MemberStatus::Booting);
                if is_booting {
                    if let Some(m) = infra.find_member_mut(&target_id) {
                        m.status = MemberStatus::Idle;
                    } else if let Some(m) = infra.find_leader_mut(&target_id) {
                        m.status = MemberStatus::Idle;
                    }
                    infra.touch();
                }
                let _ = orchestrator::deliver_queued_messages(
                    &target_id,
                    &state.db,
                    &mut infra,
                    &state.tmux,
                );
                let state_path = std::path::PathBuf::from(&state.state_path);
                if let Err(e) = infra.save(&state_path) {
                    tracing::error!(error = %e, "failed to save state after delivery");
                }
            }
            return;
        }

        tracing::error!(target_id, "timed out waiting for Claude Code readiness");
    });
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
