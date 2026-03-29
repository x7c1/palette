use crate::AppState;
use axum::{Json, extract::Query, extract::State, http::StatusCode};
use palette_domain::worker::WorkerId;
use std::sync::Arc;

use super::HookQuery;

pub async fn handle_session_start(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HookQuery>,
    Json(payload): Json<serde_json::Value>,
) -> StatusCode {
    let worker_id_str = query.worker_id.as_deref().unwrap_or("unknown");
    let worker_id = WorkerId::new(worker_id_str);

    tracing::info!(worker_id = worker_id_str, "received session-start hook");

    super::save_session_id(state.interactor.data_store.as_ref(), &worker_id, &payload);

    StatusCode::OK
}
