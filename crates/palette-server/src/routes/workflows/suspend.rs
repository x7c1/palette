use crate::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palette_domain::server::ServerEvent;
use palette_domain::workflow::WorkflowId;
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct SuspendWorkflowResponse {
    pub accepted: bool,
}

pub async fn handle_suspend_workflow(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    let workflow_id = WorkflowId::new(id);
    let _ = state
        .event_tx
        .send(ServerEvent::SuspendWorkflow { workflow_id });

    tracing::info!("suspend request accepted");

    Ok((
        StatusCode::ACCEPTED,
        Json(SuspendWorkflowResponse { accepted: true }),
    )
        .into_response())
}
