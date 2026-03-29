use crate::AppState;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse, response::Response};
use palette_domain::server::ServerEvent;
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct SuspendWorkflowResponse {
    pub accepted: bool,
}

pub async fn handle_suspend_workflow(
    State(state): State<Arc<AppState>>,
) -> Result<Response, (StatusCode, String)> {
    let _ = state.event_tx.send(ServerEvent::SuspendWorkflow);

    tracing::info!("suspend request accepted");

    Ok((
        StatusCode::ACCEPTED,
        Json(SuspendWorkflowResponse { accepted: true }),
    )
        .into_response())
}
