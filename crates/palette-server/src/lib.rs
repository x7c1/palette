mod create_task_api;
pub use create_task_api::CreateTaskApi;

mod update_task_api;
pub use update_task_api::UpdateTaskApi;

mod submit_review_api;
pub use submit_review_api::SubmitReviewApi;

mod review_comment_input_api;
pub use review_comment_input_api::ReviewCommentInputApi;

mod task_filter_api;
pub use task_filter_api::TaskFilterApi;

mod task_response;
pub use task_response::TaskResponse;

mod review_submission_response;
pub use review_submission_response::ReviewSubmissionResponse;

mod review_comment_response;
pub use review_comment_response::ReviewCommentResponse;

mod task_type_api;
pub use task_type_api::TaskTypeApi;

mod task_status_api;
pub use task_status_api::TaskStatusApi;

mod priority_api;
pub use priority_api::PriorityApi;

mod verdict_api;
pub use verdict_api::VerdictApi;

mod repository_api;
pub use repository_api::RepositoryApi;

mod routes;

use axum::Router;
use palette_core::DockerConfig;
use palette_core::docker::DockerManager;
use palette_core::orchestrator;
use palette_core::state::{AgentStatus, PersistentState};
use palette_db::{AgentId, Database, RuleEngine};
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
/// Interval between readiness polls.
const READINESS_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3);

/// Maximum time to wait for Claude Code readiness.
const READINESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

pub fn spawn_readiness_watcher(target_id: AgentId, state: Arc<AppState>) {
    let max_polls = READINESS_TIMEOUT.as_secs() / READINESS_POLL_INTERVAL.as_secs();
    tokio::spawn(async move {
        for _ in 0..max_polls {
            tokio::time::sleep(READINESS_POLL_INTERVAL).await;

            let tmux_target = {
                let infra = state.infra.lock().await;
                let agent = infra
                    .find_member(&target_id)
                    .or_else(|| infra.find_leader(&target_id));
                match agent {
                    Some(m) => m.tmux_target.clone(),
                    None => return,
                }
            };

            let pane_content = match state.tmux.capture_pane(tmux_target.as_ref()) {
                Ok(content) => content,
                Err(e) => {
                    tracing::warn!(target_id = %target_id, error = %e, "failed to capture pane");
                    continue;
                }
            };

            if !pane_content.contains('❯') {
                continue;
            }

            tracing::info!(target_id = %target_id, "Claude Code is ready, delivering queued message");

            {
                let mut infra = state.infra.lock().await;
                let is_booting = infra
                    .find_member(&target_id)
                    .or_else(|| infra.find_leader(&target_id))
                    .is_some_and(|m| m.status == AgentStatus::Booting);
                if is_booting {
                    if let Some(m) = infra.find_member_mut(&target_id) {
                        m.status = AgentStatus::Idle;
                    } else if let Some(m) = infra.find_leader_mut(&target_id) {
                        m.status = AgentStatus::Idle;
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

        tracing::error!(target_id = %target_id, "timed out waiting for Claude Code readiness");
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
                    let idle_targets: Vec<AgentId> = infra
                        .leaders
                        .iter()
                        .chain(infra.members.iter())
                        .filter(|m| m.status == AgentStatus::Idle)
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
                                    target_id = %target_id,
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
