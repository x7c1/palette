use crate::{AppState, Error};
use axum::{Json, extract::State};
use std::sync::Arc;

#[derive(serde::Serialize)]
pub struct WorkerResponse {
    pub id: String,
    pub workflow_id: String,
    pub role: String,
    pub status: String,
    pub supervisor_id: Option<String>,
    pub container_id: String,
    pub terminal_target: String,
    pub task_id: String,
}

pub async fn handle_list_workers(
    State(state): State<Arc<AppState>>,
) -> crate::Result<Json<Vec<WorkerResponse>>> {
    let workers = state
        .interactor
        .data_store
        .list_all_workers()
        .map_err(Error::internal)?;

    let response: Vec<WorkerResponse> = workers
        .into_iter()
        .map(|w| WorkerResponse {
            id: w.id.to_string(),
            workflow_id: w.workflow_id.to_string(),
            role: w.role.as_str().to_string(),
            status: w.status.as_str().to_string(),
            supervisor_id: w.supervisor_id.as_ref().map(|s| s.to_string()),
            container_id: w.container_id.to_string(),
            terminal_target: w.terminal_target.to_string(),
            task_id: w.task_id.to_string(),
        })
        .collect();

    Ok(Json(response))
}
