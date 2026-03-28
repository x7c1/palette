use crate::AppState;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse, response::Response};
use palette_domain::worker::WorkerStatus;
use palette_domain::workflow::WorkflowStatus;
use serde::Serialize;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct SuspendWorkflowResponse {
    pub suspended_count: usize,
}

pub async fn handle_suspend_workflow(
    State(state): State<Arc<AppState>>,
) -> Result<Response, (StatusCode, String)> {
    let workers = state
        .interactor
        .data_store
        .list_all_workers()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Only suspend workers that are in a suspendable state
    let suspendable: Vec<_> = workers
        .iter()
        .filter(|w| {
            matches!(
                w.status,
                WorkerStatus::Working | WorkerStatus::Idle | WorkerStatus::WaitingPermission
            )
        })
        .collect();

    if suspendable.is_empty() {
        tracing::info!("suspend: no workers to suspend");
        return Ok((
            StatusCode::OK,
            Json(SuspendWorkflowResponse { suspended_count: 0 }),
        )
            .into_response());
    }

    // Collect workflow IDs to update their status
    let workflow_ids: HashSet<_> = suspendable.iter().map(|w| w.workflow_id.clone()).collect();

    let mut suspended_count = 0;

    for worker in &suspendable {
        tracing::info!(worker_id = %worker.id, status = ?worker.status, "suspending worker");

        // Stop the container but do not remove it (will be reused on resume)
        if let Err(e) = state
            .interactor
            .container
            .stop_container(&worker.container_id)
        {
            tracing::warn!(
                worker_id = %worker.id,
                error = %e,
                "failed to stop container during suspend"
            );
            continue;
        }

        // Update worker status to Suspended (session_id is already in DB)
        if let Err(e) = state
            .interactor
            .data_store
            .update_worker_status(&worker.id, WorkerStatus::Suspended)
        {
            tracing::warn!(
                worker_id = %worker.id,
                error = %e,
                "failed to update worker status to Suspended"
            );
            continue;
        }

        suspended_count += 1;
    }

    // Update workflow status to Suspended
    for workflow_id in &workflow_ids {
        if let Err(e) = state
            .interactor
            .data_store
            .update_workflow_status(workflow_id, WorkflowStatus::Suspended)
        {
            tracing::warn!(
                workflow_id = %workflow_id,
                error = %e,
                "failed to update workflow status to Suspended"
            );
        }
    }

    tracing::info!(suspended_count, "suspend complete");

    Ok((
        StatusCode::OK,
        Json(SuspendWorkflowResponse { suspended_count }),
    )
        .into_response())
}
