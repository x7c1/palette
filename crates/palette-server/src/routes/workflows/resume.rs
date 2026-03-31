use crate::api_types::ResourceKind;
use crate::{AppState, Error};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palette_domain::server::ServerEvent;
use palette_domain::worker::WorkerStatus;
use palette_domain::workflow::{WorkflowId, WorkflowStatus};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct ResumeWorkflowResponse {
    pub resumed_count: usize,
}

pub async fn handle_resume_workflow(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> crate::Result<Response> {
    let workflow_id = WorkflowId::parse(&id).map_err(Error::invalid_path("id"))?;

    // Verify Blueprint hasn't changed since the last apply
    verify_blueprint_hash(&state, &workflow_id)?;

    // --- Resume Workers ---
    let workers = state
        .interactor
        .data_store
        .list_all_workers()
        .map_err(Error::internal)?;

    let suspended: Vec<_> = workers
        .into_iter()
        .filter(|w| w.status == WorkerStatus::Suspended && w.workflow_id == workflow_id)
        .collect();

    if suspended.is_empty() {
        tracing::info!(workflow_id = %workflow_id, "resume: no suspended workers found");
        return Ok((
            StatusCode::OK,
            Json(ResumeWorkflowResponse { resumed_count: 0 }),
        )
            .into_response());
    }

    let mut resumed_ids = Vec::new();

    for worker in &suspended {
        tracing::info!(worker_id = %worker.id, "resuming worker");

        // Restart the stopped container
        if let Err(e) = state
            .interactor
            .container
            .start_container(&worker.container_id)
        {
            tracing::warn!(
                worker_id = %worker.id,
                error = %e,
                "failed to start container during resume"
            );
            continue;
        }

        // Resume the Claude Code session if a session_id is available.
        let cmd = if let Some(ref session_id) = worker.session_id {
            state.interactor.container.claude_resume_command(
                &worker.container_id,
                session_id,
                worker.role,
            )
        } else {
            state.interactor.container.claude_exec_command(
                &worker.container_id,
                "/home/agent/prompt.md",
                worker.role,
            )
        };

        if let Err(e) = state
            .interactor
            .terminal
            .send_keys(&worker.terminal_target, &cmd)
        {
            tracing::warn!(
                worker_id = %worker.id,
                error = %e,
                "failed to send resume command"
            );
            continue;
        }

        // Update status to Booting (readiness watcher will transition to Idle)
        if let Err(e) = state
            .interactor
            .data_store
            .update_worker_status(&worker.id, WorkerStatus::Booting)
        {
            tracing::warn!(
                worker_id = %worker.id,
                error = %e,
                "failed to update worker status to Booting"
            );
            continue;
        }

        resumed_ids.push(worker.id.clone());
    }

    let resumed_count = resumed_ids.len();

    // Update workflow status back to Active
    if let Err(e) = state
        .interactor
        .data_store
        .update_workflow_status(&workflow_id, WorkflowStatus::Active)
    {
        tracing::warn!(
            workflow_id = %workflow_id,
            error = %e,
            "failed to update workflow status to Active"
        );
    }

    // Clear the blueprint hash (no longer relevant once resumed)
    let _ = state
        .interactor
        .data_store
        .update_blueprint_hash(&workflow_id, None);

    // Send event to orchestrator to spawn readiness watchers
    if !resumed_ids.is_empty() {
        let _ = state.event_tx.send(ServerEvent::ResumeWorkers {
            worker_ids: resumed_ids,
        });
    }

    tracing::info!(resumed_count, "resume complete");

    Ok((
        StatusCode::OK,
        Json(ResumeWorkflowResponse { resumed_count }),
    )
        .into_response())
}

/// Verify that the Blueprint file hasn't changed since the last apply.
///
/// - If no blueprint_hash is stored: check that the Blueprint on disk matches
///   what's in the DB (no diff). If there are changes, require an apply first.
/// - If a blueprint_hash is stored: re-hash the file and compare.
fn verify_blueprint_hash(state: &AppState, workflow_id: &WorkflowId) -> crate::Result<()> {
    let workflow = state
        .interactor
        .data_store
        .get_workflow(workflow_id)
        .map_err(Error::internal)?
        .ok_or_else(|| Error::NotFound {
            resource: ResourceKind::Workflow,
            id: workflow_id.to_string(),
        })?;

    let blueprint_path = std::path::Path::new(&workflow.blueprint_path);

    match &workflow.blueprint_hash {
        Some(stored_hash) => {
            // Apply was called — verify the file hasn't changed since
            let content = std::fs::read(blueprint_path).map_err(Error::internal)?;
            let current_hash = hex_sha256(&content);
            if current_hash != *stored_hash {
                return Err(Error::internal(
                    "blueprint changed since last apply; run apply-blueprint again",
                ));
            }
        }
        None => {
            // No apply was called — check if the Blueprint has changed at all
            let tree = state
                .interactor
                .blueprint
                .read_blueprint(blueprint_path, workflow_id)
                .map_err(super::blueprint_read_error_to_server_error)?;
            let db_statuses = state
                .interactor
                .data_store
                .get_task_statuses(workflow_id)
                .map_err(Error::internal)?;

            let diff = palette_usecase::reconciliation::compute_diff(&tree, &db_statuses);
            if !diff.added_tasks.is_empty() || !diff.removed_tasks.is_empty() {
                return Err(Error::internal(
                    "blueprint has unapplied changes; run apply-blueprint first",
                ));
            }
        }
    }

    Ok(())
}

fn hex_sha256(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    format!("{digest:x}")
}
