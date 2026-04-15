use crate::AppState;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use std::sync::Arc;

pub async fn handle_shutdown(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    tracing::info!("shutdown requested via API");
    state.shutdown_notify.notify_one();
    (
        StatusCode::OK,
        Json(serde_json::json!({"status": "shutting_down"})),
    )
}
