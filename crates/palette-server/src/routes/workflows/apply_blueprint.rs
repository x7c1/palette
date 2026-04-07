use crate::api_types::ResourceKind;
use crate::{AppState, Error};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palette_domain::server::ServerEvent;
use palette_domain::workflow::WorkflowId;
use palette_usecase::reconciliation;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct ApplyBlueprintResponse {
    pub applied: bool,
    pub errors: Vec<ApplyBlueprintError>,
    pub tasks_created: usize,
    pub tasks_deleted: usize,
}

#[derive(Debug, Serialize)]
pub struct ApplyBlueprintError {
    pub task_id: String,
    pub message: String,
}

pub async fn handle_apply_blueprint(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> crate::Result<Response> {
    let workflow_id = WorkflowId::parse(&id).map_err(Error::invalid_path("id"))?;

    // Look up the workflow to get its blueprint_path
    let workflow = state
        .interactor
        .data_store
        .get_workflow(&workflow_id)
        .map_err(Error::internal)?
        .ok_or_else(|| Error::NotFound {
            resource: ResourceKind::Workflow,
            id: id.clone(),
        })?;

    // Read Blueprint file content for hashing
    let blueprint_path = std::path::Path::new(&workflow.blueprint_path);
    let blueprint_content = std::fs::read(blueprint_path).map_err(Error::internal)?;

    // Re-read Blueprint from disk (parsed into TaskTree)
    let tree = state
        .interactor
        .blueprint
        .read_blueprint(blueprint_path, &workflow_id)
        .map_err(super::blueprint_read_error_to_server_error)?;

    // Get current task statuses from DB
    let db_statuses = state
        .interactor
        .data_store
        .get_task_statuses(&workflow_id)
        .map_err(Error::internal)?;

    // Compute diff
    let diff = reconciliation::compute_diff(&tree, &db_statuses);

    if diff.added_tasks.is_empty() && diff.removed_tasks.is_empty() {
        // No changes — just save the hash so resume can verify
        let hash = hex_sha256(&blueprint_content);
        state
            .interactor
            .data_store
            .update_blueprint_hash(&workflow_id, Some(&hash))
            .map_err(Error::internal)?;

        tracing::info!(workflow_id = %workflow_id, "apply: no blueprint changes, hash saved");

        return Ok((
            StatusCode::OK,
            Json(ApplyBlueprintResponse {
                applied: true,
                errors: vec![],
                tasks_created: 0,
                tasks_deleted: 0,
            }),
        )
            .into_response());
    }

    // Validate
    let validation = reconciliation::validate_diff(&diff, &tree, &db_statuses);
    if !validation.is_valid() {
        let response = ApplyBlueprintResponse {
            applied: false,
            errors: validation
                .errors
                .into_iter()
                .map(|e| ApplyBlueprintError {
                    task_id: e.task_id,
                    message: e.message,
                })
                .collect(),
            tasks_created: 0,
            tasks_deleted: 0,
        };
        return Ok((StatusCode::OK, Json(response)).into_response());
    }

    // Execute reconciliation
    let result = reconciliation::reconcile(
        state.interactor.data_store.as_ref(),
        &diff,
        &workflow_id,
        &tree,
        &db_statuses,
    )
    .map_err(Error::internal)?;

    tracing::info!(
        workflow_id = %workflow_id,
        tasks_created = result.tasks_created,
        tasks_deleted = result.tasks_deleted,
        tasks_demoted = result.tasks_demoted.len(),
        "reconciliation complete"
    );

    // Send domain event — orchestrator handles task activation
    let _ = state.event_tx.send(ServerEvent::ActivateNewTasks {
        workflow_id: workflow_id.clone(),
    });

    // Save Blueprint hash
    let hash = hex_sha256(&blueprint_content);
    state
        .interactor
        .data_store
        .update_blueprint_hash(&workflow_id, Some(&hash))
        .map_err(Error::internal)?;

    Ok((
        StatusCode::OK,
        Json(ApplyBlueprintResponse {
            applied: true,
            errors: vec![],
            tasks_created: result.tasks_created,
            tasks_deleted: result.tasks_deleted,
        }),
    )
        .into_response())
}

fn hex_sha256(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    format!("{digest:x}")
}
