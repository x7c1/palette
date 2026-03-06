use crate::{AppState, EventRecord};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use palette_core::orchestrator;
use palette_core::state::MemberStatus;
use palette_db::*;
use palette_tmux::TmuxManager as _;
use std::path::PathBuf;
use std::sync::Arc;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Hooks
        .route("/hooks/stop", post(handle_stop))
        .route("/hooks/notification", post(handle_notification))
        // Send
        .route("/send", post(handle_send))
        // Events
        .route("/events", get(handle_events))
        // Task API
        .route("/tasks/create", post(handle_create_task))
        .route("/tasks/update", post(handle_update_task))
        .route("/tasks", get(handle_list_tasks))
        // Review API
        .route("/reviews/{id}/submit", post(handle_submit_review))
        .route("/reviews/{id}/submissions", get(handle_get_submissions))
        .with_state(state)
}

// --- Hooks ---

#[derive(serde::Deserialize)]
struct HookQuery {
    member_id: Option<String>,
}

async fn handle_stop(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HookQuery>,
    Json(payload): Json<serde_json::Value>,
) -> StatusCode {
    let member_id = query.member_id.as_deref().unwrap_or("unknown");
    tracing::info!(member_id = member_id, payload = %payload, "received stop hook");

    let record = EventRecord {
        timestamp: now(),
        event_type: "stop".to_string(),
        payload: serde_json::json!({
            "member_id": member_id,
            "original": payload,
        }),
    };
    state.event_log.lock().await.push(record);

    // Update member status to Idle and resolve leader target
    let leader_notification = {
        let mut infra = state.infra.lock().await;
        if let Some(member) = infra.find_member_mut(member_id) {
            member.status = MemberStatus::Idle;
            let leader_id = member.leader_id.clone();
            infra.touch();
            infra.find_leader(&leader_id).map(|l| l.tmux_target.clone())
        } else {
            if let Some(leader) = infra.find_leader_mut(member_id) {
                leader.status = MemberStatus::Idle;
                infra.touch();
            }
            None
        }
    };

    // Deliver any queued messages to the now-idle member
    {
        let mut infra = state.infra.lock().await;
        let _ = orchestrator::deliver_queued_messages(
            member_id,
            &state.db,
            &mut infra,
            &state.tmux,
        );
    }

    if let Some(leader_target) = leader_notification {
        let notification = format!("[event] member={member_id} type=stop");
        if let Err(e) = state.tmux.send_keys(&leader_target, &notification) {
            tracing::error!(error = %e, "failed to notify leader");
        }
    }

    StatusCode::OK
}

async fn handle_notification(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HookQuery>,
    Json(payload): Json<serde_json::Value>,
) -> StatusCode {
    let member_id = query.member_id.as_deref().unwrap_or("unknown");
    tracing::info!(member_id = member_id, payload = %payload, "received notification hook");

    let record = EventRecord {
        timestamp: now(),
        event_type: "notification".to_string(),
        payload: serde_json::json!({
            "member_id": member_id,
            "original": payload,
        }),
    };
    state.event_log.lock().await.push(record);

    // Update member status to WaitingPermission and resolve leader target
    let leader_forward = {
        let mut infra = state.infra.lock().await;
        if let Some(member) = infra.find_member_mut(member_id) {
            member.status = MemberStatus::WaitingPermission;
            let leader_id = member.leader_id.clone();
            infra.touch();
            infra.find_leader(&leader_id).map(|l| l.tmux_target.clone())
        } else {
            None
        }
    };

    if let Some(leader_target) = leader_forward {
        let notification = format!(
            "[event] member={} type=permission_prompt payload={}",
            member_id,
            serde_json::to_string(&payload).unwrap_or_default()
        );
        if let Err(e) = state.tmux.send_keys(&leader_target, &notification) {
            tracing::error!(error = %e, "failed to forward notification to leader");
        }
    }

    StatusCode::OK
}

// --- Send ---

#[derive(serde::Deserialize)]
struct SendRequest {
    member_id: Option<String>,
    #[serde(default)]
    target: Option<String>,
    message: String,
}

#[derive(serde::Serialize)]
struct SendResponse {
    queued: bool,
}

async fn handle_send(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SendRequest>,
) -> Result<Json<SendResponse>, (StatusCode, String)> {
    // If using direct target (no member_id), send immediately without queuing
    if req.member_id.is_none() {
        if let Some(ref target) = req.target {
            tracing::info!(target = %target, message = %req.message, "sending keys via tmux (direct)");
            let record = EventRecord {
                timestamp: now(),
                event_type: "send".to_string(),
                payload: serde_json::json!({
                    "target": target,
                    "message": req.message,
                }),
            };
            state.event_log.lock().await.push(record);

            state
                .tmux
                .send_keys(target, &req.message)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            return Ok(Json(SendResponse { queued: false }));
        }
        return Err((
            StatusCode::BAD_REQUEST,
            "either member_id or target is required".to_string(),
        ));
    }

    let member_id = req.member_id.as_ref().unwrap();

    // Check if target is idle — if so, send directly; otherwise queue
    let is_idle = {
        let infra = state.infra.lock().await;
        infra
            .find_member(member_id)
            .or_else(|| infra.find_leader(member_id))
            .map(|m| m.status == MemberStatus::Idle)
            .unwrap_or(false)
    };

    // Also check if there are already pending messages (maintain ordering)
    let has_pending = state
        .db
        .has_pending_messages(member_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let queued = if is_idle && !has_pending {
        // Send directly
        let tmux_target = {
            let infra = state.infra.lock().await;
            infra
                .find_member(member_id)
                .or_else(|| infra.find_leader(member_id))
                .map(|m| m.tmux_target.clone())
                .ok_or_else(|| {
                    (
                        StatusCode::NOT_FOUND,
                        format!("member not found: {member_id}"),
                    )
                })?
        };

        tracing::info!(target = %tmux_target, message = %req.message, "sending keys via tmux");
        state
            .tmux
            .send_keys(&tmux_target, &req.message)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Update status to Working
        let mut infra = state.infra.lock().await;
        if let Some(member) = infra.find_member_mut(member_id) {
            member.status = MemberStatus::Working;
            infra.touch();
        } else if let Some(leader) = infra.find_leader_mut(member_id) {
            leader.status = MemberStatus::Working;
            infra.touch();
        }

        false
    } else {
        // Queue the message
        state
            .db
            .enqueue_message(member_id, &req.message)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        tracing::info!(member_id = member_id, "message queued");
        true
    };

    let record = EventRecord {
        timestamp: now(),
        event_type: "send".to_string(),
        payload: serde_json::json!({
            "member_id": member_id,
            "message": req.message,
            "queued": queued,
        }),
    };
    state.event_log.lock().await.push(record);

    Ok(Json(SendResponse { queued }))
}

// --- Events ---

async fn handle_events(State(state): State<Arc<AppState>>) -> Json<Vec<EventRecord>> {
    let events = state.event_log.lock().await;
    Json(events.clone())
}

// --- Task API ---

async fn handle_create_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<Task>), (StatusCode, String)> {
    let task = state
        .db
        .create_task(&req)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    tracing::info!(task_id = %task.id, "created task");
    Ok((StatusCode::CREATED, Json(task)))
}

async fn handle_update_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateTaskRequest>,
) -> Result<Json<Task>, (StatusCode, String)> {
    let current = state
        .db
        .get_task(&req.id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (StatusCode::NOT_FOUND, {
                let id = &req.id;
                format!("task not found: {id}")
            })
        })?;

    RuleEngine::validate_transition(current.task_type, current.status, req.status)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let task = state
        .db
        .update_task_status(&req.id, req.status)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Apply rule engine side effects
    let effects = state
        .rules
        .on_status_change(&state.db, &req.id, req.status)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for effect in &effects {
        tracing::info!(?effect, "rule engine effect");
    }

    // Process orchestrator effects (auto-assign, destroy members)
    {
        let mut infra = state.infra.lock().await;
        let state_path = PathBuf::from(&state.state_path);
        let deliveries = orchestrator::process_effects(
            &effects,
            &state.db,
            &mut infra,
            &state.docker,
            &state.tmux,
            &state.docker_config,
            &state_path,
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Attempt to deliver queued messages for newly spawned members
        for delivery in &deliveries {
            let _ = orchestrator::deliver_queued_messages(
                &delivery.target_id,
                &state.db,
                &mut infra,
                &state.tmux,
            );
        }
    }

    Ok(Json(task))
}

async fn handle_list_tasks(
    State(state): State<Arc<AppState>>,
    Query(filter): Query<TaskFilter>,
) -> Result<Json<Vec<Task>>, (StatusCode, String)> {
    let tasks = state
        .db
        .list_tasks(&filter)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(tasks))
}

// --- Review API ---

async fn handle_submit_review(
    State(state): State<Arc<AppState>>,
    Path(review_task_id): Path<String>,
    Json(req): Json<SubmitReviewRequest>,
) -> Result<(StatusCode, Json<ReviewSubmission>), (StatusCode, String)> {
    // Verify the task exists and is a review
    let task = state
        .db
        .get_task(&review_task_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("review task not found: {review_task_id}"),
            )
        })?;

    if task.task_type != TaskType::Review {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("task {review_task_id} is not a review task"),
        ));
    }

    let submission = state
        .db
        .submit_review(&review_task_id, &req)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Apply rule engine
    let effects = state
        .rules
        .on_review_submitted(&state.db, &review_task_id, &submission)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for effect in &effects {
        tracing::info!(?effect, "review rule engine effect");
    }

    // Process orchestrator effects
    {
        let mut infra = state.infra.lock().await;
        let state_path = PathBuf::from(&state.state_path);
        let deliveries = orchestrator::process_effects(
            &effects,
            &state.db,
            &mut infra,
            &state.docker,
            &state.tmux,
            &state.docker_config,
            &state_path,
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        for delivery in &deliveries {
            let _ = orchestrator::deliver_queued_messages(
                &delivery.target_id,
                &state.db,
                &mut infra,
                &state.tmux,
            );
        }
    }

    Ok((StatusCode::CREATED, Json(submission)))
}

async fn handle_get_submissions(
    State(state): State<Arc<AppState>>,
    Path(review_task_id): Path<String>,
) -> Result<Json<Vec<ReviewSubmission>>, (StatusCode, String)> {
    let submissions = state
        .db
        .get_review_submissions(&review_task_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(submissions))
}

fn now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string()
}
