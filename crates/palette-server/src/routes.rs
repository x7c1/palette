use crate::api_types::{
    CreateTaskApi, ReviewSubmissionResponse, SubmitReviewApi, TaskFile, TaskFilterApi,
    TaskResponse, UpdateTaskApi,
};
use crate::{AppState, EventRecord};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use palette_core::models::AgentStatus;
use palette_core::orchestrator;
use palette_core::persistent_state::PersistentState;
use palette_domain::{
    AgentId, CreateTaskRequest, RuleEngine, SubmitReviewRequest, TaskFilter, TaskId, TaskStatus,
    TaskType, UpdateTaskRequest, Verdict,
};
use palette_tmux::{TerminalManager as _, TmuxManagerImpl};
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
        .route("/tasks/load", post(handle_load_tasks))
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
    let member_id_str = query.member_id.as_deref().unwrap_or("unknown");
    let member_id = AgentId::new(member_id_str);
    tracing::info!(member_id = member_id_str, payload = %payload, "received stop hook");

    let record = EventRecord {
        timestamp: now(),
        event_type: "stop".to_string(),
        payload: serde_json::json!({
            "member_id": member_id_str,
            "original": payload,
        }),
    };
    state.event_log.lock().await.push(record);

    // Update member status to Idle and resolve leader ID
    let leader_id = {
        let mut infra = state.infra.lock().await;
        if let Some(member) = infra.find_member_mut(&member_id) {
            member.status = AgentStatus::Idle;
            let leader_id = member.leader_id.clone();
            infra.touch();
            Some(leader_id)
        } else {
            if let Some(leader) = infra.find_leader_mut(&member_id) {
                leader.status = AgentStatus::Idle;
                infra.touch();
            }
            None
        }
    };

    // Transition member's in_progress tasks to in_review and notify leader
    if let Some(ref leader_id) = leader_id {
        let member_tasks = state
            .db
            .list_tasks(&TaskFilter {
                assignee: Some(member_id.clone()),
                status: Some(TaskStatus::InProgress),
                ..Default::default()
            })
            .unwrap_or_default();

        for task in &member_tasks {
            if let Err(e) = state.db.update_task_status(&task.id, TaskStatus::InReview) {
                tracing::error!(task_id = %task.id, error = %e, "failed to transition task to in_review");
                continue;
            }
            let effects = state
                .rules
                .on_status_change(&state.db, &task.id, TaskStatus::InReview)
                .unwrap_or_default();
            for effect in &effects {
                tracing::info!(?effect, "rule engine effect (member stop)");
            }

            // Enqueue review instruction to leader
            let review_msg = format!(
                "[review] task={} member={} message: {}",
                task.id,
                member_id,
                payload
                    .get("last_assistant_message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(work completed)")
            );
            if let Err(e) = state.db.enqueue_message(leader_id, &review_msg) {
                tracing::error!(error = %e, "failed to enqueue review notification for leader");
            }
        }

        if member_tasks.is_empty() {
            // No tasks to transition; just send a stop event
            let notification = format!("[event] member={member_id} type=stop");
            if let Err(e) = state.db.enqueue_message(leader_id, &notification) {
                tracing::error!(error = %e, "failed to enqueue stop notification for leader");
            }
        }
    }

    // Deliver any queued messages to the now-idle member (own queue only)
    {
        let mut infra = state.infra.lock().await;
        let _ =
            orchestrator::deliver_queued_messages(&member_id, &state.db, &mut infra, &state.tmux);
    }

    // Notify background delivery loop (leader may have pending messages)
    state.delivery_notify.notify_one();

    StatusCode::OK
}

async fn handle_notification(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HookQuery>,
    Json(payload): Json<serde_json::Value>,
) -> StatusCode {
    let member_id_str = query.member_id.as_deref().unwrap_or("unknown");
    let member_id = AgentId::new(member_id_str);
    tracing::info!(member_id = member_id_str, payload = %payload, "received notification hook");

    let record = EventRecord {
        timestamp: now(),
        event_type: "notification".to_string(),
        payload: serde_json::json!({
            "member_id": member_id_str,
            "original": payload,
        }),
    };
    state.event_log.lock().await.push(record);

    // Update member status to WaitingPermission and resolve leader ID
    let leader_id = {
        let mut infra = state.infra.lock().await;
        if let Some(member) = infra.find_member_mut(&member_id) {
            member.status = AgentStatus::WaitingPermission;
            let leader_id = member.leader_id.clone();
            infra.touch();
            Some(leader_id)
        } else {
            None
        }
    };

    // Enqueue event notification to leader
    if let Some(ref leader_id) = leader_id {
        let notification = format!(
            "[event] member={} type=permission_prompt payload={}",
            member_id,
            serde_json::to_string(&payload).unwrap_or_default()
        );
        if let Err(e) = state.db.enqueue_message(leader_id, &notification) {
            tracing::error!(error = %e, "failed to enqueue notification for leader");
        }
    }

    // Notify background delivery loop (leader may have pending messages)
    state.delivery_notify.notify_one();

    StatusCode::OK
}

// --- Send ---

#[derive(serde::Deserialize)]
struct SendRequest {
    member_id: Option<String>,
    #[serde(default)]
    target: Option<String>,
    message: String,
    /// If true, send the message without appending Enter key.
    /// Use for permission prompt responses (e.g., "2" to approve).
    #[serde(default)]
    no_enter: bool,
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

            send_tmux_keys(&state.tmux, target, &req.message, req.no_enter)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            return Ok(Json(SendResponse { queued: false }));
        }
        return Err((
            StatusCode::BAD_REQUEST,
            "either member_id or target is required".to_string(),
        ));
    }

    let member_id_str = req.member_id.as_ref().unwrap();
    let member_id = AgentId::new(member_id_str.as_str());

    // Check if target can receive input — idle or waiting for permission
    let is_idle = {
        let infra = state.infra.lock().await;
        infra
            .find_member(&member_id)
            .or_else(|| infra.find_leader(&member_id))
            .map(|m| m.status == AgentStatus::Idle || m.status == AgentStatus::WaitingPermission)
            .unwrap_or(false)
    };

    // Also check if there are already pending messages (maintain ordering)
    let has_pending = state
        .db
        .has_pending_messages(&member_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let queued = if is_idle && !has_pending {
        // Send directly
        let terminal_target = {
            let infra = state.infra.lock().await;
            infra
                .find_member(&member_id)
                .or_else(|| infra.find_leader(&member_id))
                .map(|m| m.terminal_target.clone())
                .ok_or_else(|| {
                    (
                        StatusCode::NOT_FOUND,
                        format!("member not found: {member_id}"),
                    )
                })?
        };

        tracing::info!(target = %terminal_target, message = %req.message, "sending keys via tmux");
        send_tmux_keys(
            &state.tmux,
            terminal_target.as_ref(),
            &req.message,
            req.no_enter,
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Update status to Working
        let mut infra = state.infra.lock().await;
        if let Some(member) = infra.find_member_mut(&member_id) {
            member.status = AgentStatus::Working;
            infra.touch();
        } else if let Some(leader) = infra.find_leader_mut(&member_id) {
            leader.status = AgentStatus::Working;
            infra.touch();
        }

        false
    } else {
        // Queue the message
        state
            .db
            .enqueue_message(&member_id, &req.message)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        tracing::info!(member_id = member_id_str.as_str(), "message queued");
        true
    };

    let record = EventRecord {
        timestamp: now(),
        event_type: "send".to_string(),
        payload: serde_json::json!({
            "member_id": member_id_str,
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

async fn handle_load_tasks(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Result<(StatusCode, Json<Vec<TaskResponse>>), (StatusCode, String)> {
    let task_file = TaskFile::parse(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid YAML: {e}")))?;

    let requests = task_file.into_requests();
    let mut created_tasks = Vec::new();

    // Create all tasks as draft
    for req in &requests {
        let task = state
            .db
            .create_task(req)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        tracing::info!(task_id = %task.id, "created task from YAML");
        created_tasks.push(task);
    }

    // Transition work tasks to ready, triggering auto-assign via rule engine
    let work_ids: Vec<TaskId> = created_tasks
        .iter()
        .filter(|t| t.task_type == TaskType::Work)
        .map(|t| t.id.clone())
        .collect();

    for work_id in &work_ids {
        RuleEngine::validate_transition(TaskType::Work, TaskStatus::Draft, TaskStatus::Ready)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        state
            .db
            .update_task_status(work_id, TaskStatus::Ready)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let effects = state
            .rules
            .on_status_change(&state.db, work_id, TaskStatus::Ready)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        for effect in &effects {
            tracing::info!(?effect, "rule engine effect (task load)");
        }

        let deliveries = {
            let mut infra = state.infra.lock().await;
            let deliveries = orchestrator::process_effects(
                &effects,
                &state.db,
                &mut infra,
                &state.docker,
                &state.tmux,
                &state.docker_config,
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

            save_state(&state, &infra);
            deliveries
        };

        for delivery in deliveries {
            crate::spawn_readiness_watcher(delivery.target_id, Arc::clone(&state));
        }
    }

    // Re-fetch all tasks to return updated statuses
    let final_tasks: Vec<TaskResponse> = created_tasks
        .iter()
        .filter_map(|t| state.db.get_task(&t.id).ok().flatten())
        .map(TaskResponse::from)
        .collect();

    Ok((StatusCode::CREATED, Json(final_tasks)))
}

async fn handle_create_task(
    State(state): State<Arc<AppState>>,
    Json(api_req): Json<CreateTaskApi>,
) -> Result<(StatusCode, Json<TaskResponse>), (StatusCode, String)> {
    let req: CreateTaskRequest = api_req.into();
    let task = state
        .db
        .create_task(&req)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    tracing::info!(task_id = %task.id, "created task");
    Ok((StatusCode::CREATED, Json(TaskResponse::from(task))))
}

async fn handle_update_task(
    State(state): State<Arc<AppState>>,
    Json(api_req): Json<UpdateTaskApi>,
) -> Result<Json<TaskResponse>, (StatusCode, String)> {
    let req: UpdateTaskRequest = api_req.into();
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
    let deliveries = {
        let mut infra = state.infra.lock().await;
        let deliveries = orchestrator::process_effects(
            &effects,
            &state.db,
            &mut infra,
            &state.docker,
            &state.tmux,
            &state.docker_config,
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Deliver queued messages for non-booting members
        for delivery in &deliveries {
            let _ = orchestrator::deliver_queued_messages(
                &delivery.target_id,
                &state.db,
                &mut infra,
                &state.tmux,
            );
        }

        save_state(&state, &infra);
        deliveries
    };

    // Spawn background readiness watchers for booting members
    for delivery in deliveries {
        crate::spawn_readiness_watcher(delivery.target_id, Arc::clone(&state));
    }

    Ok(Json(TaskResponse::from(task)))
}

async fn handle_list_tasks(
    State(state): State<Arc<AppState>>,
    Query(api_filter): Query<TaskFilterApi>,
) -> Result<Json<Vec<TaskResponse>>, (StatusCode, String)> {
    let filter: TaskFilter = api_filter.into();
    let tasks = state
        .db
        .list_tasks(&filter)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(tasks.into_iter().map(TaskResponse::from).collect()))
}

// --- Review API ---

async fn handle_submit_review(
    State(state): State<Arc<AppState>>,
    Path(review_task_id): Path<String>,
    Json(api_req): Json<SubmitReviewApi>,
) -> Result<(StatusCode, Json<ReviewSubmissionResponse>), (StatusCode, String)> {
    let review_task_id = TaskId::new(review_task_id);
    let req: SubmitReviewRequest = api_req.into();

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

    // If changes_requested, enqueue feedback to the assignee member
    if submission.verdict == Verdict::ChangesRequested {
        let work_tasks = state
            .db
            .find_works_for_review(&review_task_id)
            .unwrap_or_default();
        for work in &work_tasks {
            if let Some(ref assignee) = work.assignee {
                let feedback = format!(
                    "[review-feedback] task={} verdict=changes_requested summary: {}",
                    work.id,
                    submission.summary.as_deref().unwrap_or("(no summary)")
                );
                let _ = state.db.enqueue_message(assignee, &feedback);
                tracing::info!(
                    task_id = %work.id,
                    assignee = %assignee,
                    "enqueued review feedback to member"
                );
            }
        }
    }

    // Process orchestrator effects
    let deliveries = {
        let mut infra = state.infra.lock().await;
        let deliveries = orchestrator::process_effects(
            &effects,
            &state.db,
            &mut infra,
            &state.docker,
            &state.tmux,
            &state.docker_config,
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

        save_state(&state, &infra);
        deliveries
    };

    // Notify delivery loop (member may be Idle and have pending feedback)
    state.delivery_notify.notify_one();

    // Spawn background readiness watchers for booting members
    for delivery in deliveries {
        crate::spawn_readiness_watcher(delivery.target_id, Arc::clone(&state));
    }

    Ok((
        StatusCode::CREATED,
        Json(ReviewSubmissionResponse::from(submission)),
    ))
}

async fn handle_get_submissions(
    State(state): State<Arc<AppState>>,
    Path(review_task_id): Path<String>,
) -> Result<Json<Vec<ReviewSubmissionResponse>>, (StatusCode, String)> {
    let review_task_id = TaskId::new(review_task_id);
    let submissions = state
        .db
        .get_review_submissions(&review_task_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(
        submissions
            .into_iter()
            .map(ReviewSubmissionResponse::from)
            .collect(),
    ))
}

/// Spawn a background task that polls a member's tmux pane for Claude Code readiness,
/// then delivers the queued message.
fn send_tmux_keys(
    tmux: &TmuxManagerImpl,
    target: &str,
    message: &str,
    no_enter: bool,
) -> palette_tmux::Result<()> {
    if no_enter {
        tmux.send_keys_literal(target, message)
    } else {
        tmux.send_keys(target, message)
    }
}

fn save_state(state: &AppState, infra: &PersistentState) {
    let path = std::path::PathBuf::from(&state.state_path);
    if let Err(e) = palette_file_state::save(infra, &path) {
        tracing::error!(error = %e, "failed to save state");
    }
}

fn now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string()
}
