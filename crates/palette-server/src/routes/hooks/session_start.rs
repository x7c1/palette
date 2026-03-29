use crate::AppState;
use axum::{Json, extract::Query, extract::State, http::StatusCode};
use palette_domain::worker::{WorkerId, WorkerSessionId};
use std::sync::Arc;

use super::HookQuery;

pub async fn handle_session_start(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HookQuery>,
    Json(payload): Json<serde_json::Value>,
) -> StatusCode {
    let worker_id_str = query.worker_id.as_deref().unwrap_or("unknown");
    let worker_id = WorkerId::new(worker_id_str);

    let Some(session_id) = payload.get("session_id").and_then(|v| v.as_str()) else {
        tracing::warn!(
            worker_id = worker_id_str,
            "session-start hook: no session_id in payload"
        );
        return StatusCode::OK;
    };

    let sid = WorkerSessionId::new(session_id);
    if let Err(e) = state
        .interactor
        .data_store
        .update_worker_session_id(&worker_id, &sid)
    {
        tracing::error!(
            worker_id = worker_id_str,
            error = %e,
            "failed to save session_id on session start"
        );
    } else {
        tracing::info!(
            worker_id = worker_id_str,
            session_id = session_id,
            "saved session_id from session start"
        );
    }

    StatusCode::OK
}
