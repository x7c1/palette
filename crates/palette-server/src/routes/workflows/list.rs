use crate::{AppState, Error};
use axum::{Json, extract::Query, extract::State};
use palette_domain::workflow::WorkflowStatus;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct ListWorkflowsQuery {
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WorkflowResponse {
    pub id: String,
    pub blueprint_path: String,
    pub status: String,
    pub started_at: String,
}

pub async fn handle_list_workflows(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListWorkflowsQuery>,
) -> crate::Result<Json<Vec<WorkflowResponse>>> {
    let status_filter = query
        .status
        .as_deref()
        .map(WorkflowStatus::parse)
        .transpose()
        .map_err(Error::invalid_query("status"))?;

    let workflows = state
        .interactor
        .data_store
        .list_workflows(status_filter)
        .map_err(Error::internal)?;

    let response: Vec<WorkflowResponse> = workflows
        .into_iter()
        .map(|w| WorkflowResponse {
            id: w.id.to_string(),
            blueprint_path: w.blueprint_path,
            status: w.status.as_str().to_string(),
            started_at: w.started_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(response))
}
