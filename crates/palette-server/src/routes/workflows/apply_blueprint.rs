use super::task_activation::{activate_ready_children, internal_err};
use crate::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palette_domain::server::ServerEvent;
use palette_domain::task::TaskId;
use palette_domain::workflow::WorkflowId;
use palette_usecase::TaskRuleEngine;
use palette_usecase::reconciliation;
use palette_usecase::task_store::TaskStore;
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
) -> Result<Response, (StatusCode, String)> {
    let workflow_id = WorkflowId::new(&id);

    // Look up the workflow to get its blueprint_path
    let workflow = state
        .interactor
        .data_store
        .get_workflow(&workflow_id)
        .map_err(internal_err)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("workflow {id} not found")))?;

    // Read Blueprint file content for hashing
    let blueprint_path = std::path::Path::new(&workflow.blueprint_path);
    let blueprint_content = std::fs::read(blueprint_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to read blueprint file: {e}"),
        )
    })?;

    // Re-read Blueprint from disk (parsed into TaskTree)
    let tree = state
        .interactor
        .blueprint
        .read_blueprint(blueprint_path, &workflow_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to parse blueprint: {e}"),
            )
        })?;

    // Get current task statuses from DB
    let db_statuses = state
        .interactor
        .data_store
        .get_task_statuses(&workflow_id)
        .map_err(internal_err)?;

    // Compute diff
    let diff = reconciliation::compute_diff(&tree, &db_statuses);

    if diff.added_tasks.is_empty() && diff.removed_tasks.is_empty() {
        // No changes — just save the hash so resume can verify
        let hash = hex_sha256(&blueprint_content);
        state
            .interactor
            .data_store
            .update_blueprint_hash(&workflow_id, Some(&hash))
            .map_err(internal_err)?;

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
    .map_err(internal_err)?;

    tracing::info!(
        workflow_id = %workflow_id,
        tasks_created = result.tasks_created,
        tasks_deleted = result.tasks_deleted,
        tasks_demoted = result.tasks_demoted.len(),
        "reconciliation complete"
    );

    // Activate newly Ready tasks
    let updated_statuses = state
        .interactor
        .data_store
        .get_task_statuses(&workflow_id)
        .map_err(internal_err)?;

    let task_store = TaskStore::new(
        state.interactor.data_store.as_ref(),
        tree,
        workflow_id.clone(),
        updated_statuses,
    );
    let task_engine = TaskRuleEngine::new(&task_store);

    let pending_ids: Vec<TaskId> = task_store
        .tree()
        .task_ids()
        .filter(|id| {
            task_store
                .get_task(id)
                .ok()
                .flatten()
                .is_some_and(|t| t.status == palette_domain::task::TaskStatus::Pending)
        })
        .cloned()
        .collect();

    if !pending_ids.is_empty() {
        let effects = activate_ready_children(&state, &task_store, &task_engine, &pending_ids)?;
        if !effects.is_empty() {
            let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
        }
    }

    // Save Blueprint hash
    let hash = hex_sha256(&blueprint_content);
    state
        .interactor
        .data_store
        .update_blueprint_hash(&workflow_id, Some(&hash))
        .map_err(internal_err)?;

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
