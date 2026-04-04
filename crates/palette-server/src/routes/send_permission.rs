use crate::api_types::{ResourceKind, SendPermissionRequest, SendResponse};
use crate::{AppState, Error, EventRecord, ValidJson};
use axum::{Json, extract::State};
use palette_core::ReasonKey;
use palette_domain::worker::{WorkerId, WorkerStatus};
use std::sync::Arc;

use super::now;

pub async fn handle_send_permission(
    State(state): State<Arc<AppState>>,
    ValidJson(req): ValidJson<SendPermissionRequest>,
) -> crate::Result<Json<SendResponse>> {
    let worker_id = WorkerId::parse(&req.worker_id).map_err(Error::invalid_body("worker_id"))?;
    if req.event_id.trim().is_empty() {
        return Err(Error::invalid_body("event_id")(PermissionSendError::Empty));
    }
    if req.choice.trim().is_empty() {
        return Err(Error::invalid_body("choice")(PermissionSendError::Empty));
    }

    let expected_event_id = {
        let events = state.pending_permission_events.lock().await;
        events.get(worker_id.as_ref()).cloned()
    };
    let Some(expected_event_id) = expected_event_id else {
        return Err(Error::invalid_body("event_id")(
            PermissionSendError::NotFound,
        ));
    };
    if expected_event_id != req.event_id {
        return Err(Error::invalid_body("event_id")(
            PermissionSendError::Mismatched,
        ));
    }

    let worker = state
        .interactor
        .data_store
        .find_worker(&worker_id)
        .map_err(Error::internal)?
        .ok_or_else(|| Error::NotFound {
            resource: ResourceKind::Worker,
            id: worker_id.to_string(),
        })?;

    if worker.status != WorkerStatus::WaitingPermission {
        return Err(Error::invalid_body("worker_id")(
            PermissionSendError::NotWaitingPermission,
        ));
    }

    state
        .interactor
        .terminal
        .send_keys_no_enter(&worker.terminal_target, &req.choice)
        .map_err(Error::internal)?;

    if let Err(e) = state
        .interactor
        .data_store
        .update_worker_status(&worker_id, WorkerStatus::Working)
    {
        tracing::error!(
            worker_id = worker_id.as_ref(),
            error = %e,
            "failed to update worker status to Working after permission send"
        );
    }

    {
        let mut events = state.pending_permission_events.lock().await;
        events.remove(worker_id.as_ref());
    }

    let record = EventRecord {
        timestamp: now(),
        event_type: "send_permission".to_string(),
        payload: serde_json::json!({
            "worker_id": worker_id.as_ref(),
            "event_id": req.event_id,
            "choice": req.choice,
        }),
    };
    state.event_log.lock().await.push(record);

    Ok(Json(SendResponse { queued: false }))
}

enum PermissionSendError {
    Empty,
    NotFound,
    Mismatched,
    NotWaitingPermission,
}

impl ReasonKey for PermissionSendError {
    fn namespace(&self) -> &str {
        "send_permission"
    }

    fn value(&self) -> &str {
        match self {
            PermissionSendError::Empty => "empty",
            PermissionSendError::NotFound => "event_not_found",
            PermissionSendError::Mismatched => "event_id_mismatched",
            PermissionSendError::NotWaitingPermission => "worker_not_waiting_permission",
        }
    }
}
