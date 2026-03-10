use crate::AppState;
use crate::api_types::{CreateTaskRequest, TaskResponse};
use axum::{Json, extract::State, http::StatusCode};
use palette_domain as domain;
use std::sync::Arc;

pub async fn handle_create_task(
    State(state): State<Arc<AppState>>,
    Json(api_req): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), (StatusCode, String)> {
    let req: domain::task::CreateTaskRequest = api_req.into();
    let task = state
        .db
        .create_task(&req)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    tracing::info!(task_id = %task.id, "created task");
    Ok((StatusCode::CREATED, Json(TaskResponse::from(task))))
}
