use crate::api_types::{ErrorCode, InputError, Location, ResourceKind, SendRequest, SendResponse};
use crate::{AppState, Error, EventRecord};
use axum::{Json, extract::State};
use palette_domain::terminal::TerminalTarget;
use palette_domain::worker::{WorkerId, WorkerStatus};
use std::sync::Arc;

use super::now;

pub async fn handle_send(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SendRequest>,
) -> crate::Result<Json<SendResponse>> {
    // If using direct target (no worker_id), send immediately without queuing
    if req.worker_id.is_none() {
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
            .map_err(Error::internal)?;
            return Ok(Json(SendResponse { queued: false }));
        }
        return Err(Error::BadRequest {
            code: ErrorCode::InputValidationFailed,
            errors: vec![InputError {
                location: Location::Body,
                hint: "worker_id".into(),
                reason: "worker_id/required".into(),
            }],
        });
    }

    let worker_id_str = req.worker_id.as_ref().unwrap();
    let worker_id = WorkerId::parse(worker_id_str.as_str()).map_err(|e| Error::BadRequest {
        code: ErrorCode::InputValidationFailed,
        errors: vec![InputError {
            location: Location::Body,
            hint: "worker_id".into(),
            reason: palette_domain::ReasonKey::reason_key(&e),
        }],
    })?;

    // Check if target can receive input — idle or waiting for permission
    let worker = state
        .interactor
        .data_store
        .find_worker(&worker_id)
        .map_err(Error::internal)?;

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
        .has_pending_messages(&worker_id)
        .map_err(Error::internal)?;

    let queued = if can_receive && (!has_pending || is_waiting_permission) {
        // Send directly
        let terminal_target = worker
            .as_ref()
            .map(|a| a.terminal_target.clone())
            .ok_or_else(|| Error::NotFound {
                resource: ResourceKind::Worker,
                id: worker_id.to_string(),
            })?;

        tracing::info!(target = %terminal_target, message = %req.message, no_enter = req.no_enter, "sending keys via tmux");
        send_tmux_keys(
            state.interactor.terminal.as_ref(),
            &terminal_target,
            &req.message,
            req.no_enter,
        )
        .map_err(Error::internal)?;

        if let Err(e) = state
            .interactor
            .data_store
            .update_worker_status(&worker_id, WorkerStatus::Working)
        {
            tracing::error!(worker_id = worker_id_str.as_str(), error = %e, "failed to update worker status to Working");
        }

        false
    } else {
        // Queue the message
        state
            .interactor
            .data_store
            .enqueue_message(&worker_id, &req.message)
            .map_err(Error::internal)?;
        tracing::info!(worker_id = worker_id_str.as_str(), "message queued");
        true
    };

    let record = EventRecord {
        timestamp: now(),
        event_type: "send".to_string(),
        payload: serde_json::json!({
            "worker_id": worker_id_str,
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
