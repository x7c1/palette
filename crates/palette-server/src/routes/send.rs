use crate::api_types::{SendRequest, SendResponse};
use crate::{AppState, EventRecord};
use axum::{Json, extract::State, http::StatusCode};
use palette_domain::terminal::TerminalTarget;
use palette_domain::worker::{WorkerId, WorkerStatus};
use std::sync::Arc;

use super::now;

pub async fn handle_send(
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

            let terminal_target = TerminalTarget::new(target);
            send_tmux_keys(
                state.interactor.terminal.as_ref(),
                &terminal_target,
                &req.message,
                req.no_enter,
            )
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            return Ok(Json(SendResponse { queued: false }));
        }
        return Err((
            StatusCode::BAD_REQUEST,
            "either member_id or target is required".to_string(),
        ));
    }

    let member_id_str = req.member_id.as_ref().unwrap();
    let member_id = WorkerId::new(member_id_str.as_str());

    // Check if target can receive input — idle or waiting for permission
    let worker = state
        .interactor
        .data_store
        .find_worker(&member_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (can_receive, is_waiting_permission) = worker
        .as_ref()
        .map(|a| {
            let can = a.status == WorkerStatus::Idle || a.status == WorkerStatus::WaitingPermission;
            let waiting = a.status == WorkerStatus::WaitingPermission;
            (can, waiting)
        })
        .unwrap_or((false, false));

    // Also check if there are already pending messages (maintain ordering).
    // However, permission approvals bypass the queue — they are tmux key presses
    // orthogonal to queued instruction messages. Without this bypass, a pending
    // instruction blocks the approval key, creating a deadlock.
    let has_pending = state
        .interactor
        .data_store
        .has_pending_messages(&member_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let queued = if can_receive && (!has_pending || is_waiting_permission) {
        // Send directly
        let terminal_target = worker
            .as_ref()
            .map(|a| a.terminal_target.clone())
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    format!("member not found: {member_id}"),
                )
            })?;

        tracing::info!(target = %terminal_target, message = %req.message, no_enter = req.no_enter, "sending keys via tmux");
        send_tmux_keys(
            state.interactor.terminal.as_ref(),
            &terminal_target,
            &req.message,
            req.no_enter,
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Update status to Working
        let _ = state
            .interactor
            .data_store
            .update_worker_status(&member_id, WorkerStatus::Working);

        false
    } else {
        // Queue the message
        state
            .interactor
            .data_store
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

fn send_tmux_keys(
    terminal: &dyn palette_usecase::TerminalSession,
    target: &TerminalTarget,
    message: &str,
    no_enter: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if no_enter {
        terminal.send_keys_no_enter(target, message)
    } else {
        terminal.send_keys(target, message)
    }
}
