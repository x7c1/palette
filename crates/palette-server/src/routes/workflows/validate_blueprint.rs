use crate::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palette_domain::workflow::WorkflowId;
use palette_usecase::reconciliation;
use serde::Serialize;
use std::path;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct ValidateBlueprintResponse {
    pub valid: bool,
    pub errors: Vec<ValidateBlueprintError>,
    pub added_tasks: Vec<String>,
    pub removed_tasks: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ValidateBlueprintError {
    pub task_id: String,
    pub message: String,
}

pub async fn handle_validate_blueprint(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    let workflow_id = WorkflowId::new(&id);

    // Look up the workflow to get its blueprint_path
    let workflow = state
        .interactor
        .data_store
        .get_workflow(&workflow_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("workflow {id} not found")))?;

    // Re-read the Blueprint from disk (it may have been edited during suspend)
    let tree = state
        .interactor
        .blueprint
        .read_blueprint(path::Path::new(&workflow.blueprint_path), &workflow_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to read blueprint: {e}"),
            )
        })?;

    // Get current task statuses from DB
    let db_statuses = state
        .interactor
        .data_store
        .get_task_statuses(&workflow_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Compute diff and validate
    let diff = reconciliation::compute_diff(&tree, &db_statuses);
    let result = reconciliation::validate_diff(&diff, &tree, &db_statuses);

    let response = ValidateBlueprintResponse {
        valid: result.is_valid(),
        errors: result
            .errors
            .into_iter()
            .map(|e| ValidateBlueprintError {
                task_id: e.task_id,
                message: e.message,
            })
            .collect(),
        added_tasks: diff.added_tasks.iter().map(|id| id.to_string()).collect(),
        removed_tasks: diff.removed_tasks.iter().map(|id| id.to_string()).collect(),
    };

    Ok((StatusCode::OK, Json(response)).into_response())
}
