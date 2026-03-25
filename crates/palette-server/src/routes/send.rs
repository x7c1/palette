use crate::api_types::{SendRequest, SendResponse};
use crate::{AppState, EventRecord};
use axum::{Json, extract::State, http::StatusCode};
use palette_domain::agent::{AgentId, AgentStatus};
use palette_domain::terminal::TerminalTarget;
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
            send_tmux_keys(&state.tmux, &terminal_target, &req.message, req.no_enter)
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
    let agent = state
        .db
        .find_agent(&member_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (can_receive, is_waiting_permission) = agent
        .as_ref()
        .map(|a| {
            let can = a.status == AgentStatus::Idle || a.status == AgentStatus::WaitingPermission;
            let waiting = a.status == AgentStatus::WaitingPermission;
            (can, waiting)
        })
        .unwrap_or((false, false));

    // Also check if there are already pending messages (maintain ordering).
    // However, permission approvals bypass the queue — they are tmux key presses
    // orthogonal to queued instruction messages. Without this bypass, a pending
    // instruction blocks the approval key, creating a deadlock.
    let has_pending = state
        .db
        .has_pending_messages(&member_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let queued = if can_receive && (!has_pending || is_waiting_permission) {
        // Send directly
        let terminal_target = agent
            .as_ref()
            .map(|a| a.terminal_target.clone())
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    format!("member not found: {member_id}"),
                )
            })?;

        tracing::info!(target = %terminal_target, message = %req.message, no_enter = req.no_enter, "sending keys via tmux");
        send_tmux_keys(&state.tmux, &terminal_target, &req.message, req.no_enter)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Update status to Working
        let _ = state
            .db
            .update_agent_status(&member_id, AgentStatus::Working);

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

fn send_tmux_keys(
    tmux: &palette_tmux::TmuxManager,
    target: &TerminalTarget,
    message: &str,
    no_enter: bool,
) -> palette_tmux::Result<()> {
    if no_enter {
        tmux.send_keys_no_enter(target, message)
    } else {
        tmux.send_keys(target, message)
    }
}
