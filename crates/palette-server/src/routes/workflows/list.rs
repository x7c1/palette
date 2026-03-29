use crate::AppState;
use axum::{Json, extract::Query, extract::State, http::StatusCode};
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
) -> Result<Json<Vec<WorkflowResponse>>, (StatusCode, String)> {
    let status_filter = match query.status.as_deref() {
        Some("active") => Some(WorkflowStatus::Active),
        Some("suspending") => Some(WorkflowStatus::Suspending),
        Some("suspended") => Some(WorkflowStatus::Suspended),
        Some("completed") => Some(WorkflowStatus::Completed),
        Some(unknown) => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("unknown workflow status: {unknown}"),
            ));
        }
        None => None,
    };

    let workflows = state
        .interactor
        .data_store
        .list_workflows(status_filter)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

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
