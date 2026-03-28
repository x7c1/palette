use crate::AppState;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse, response::Response};
use palette_domain::server::ServerEvent;
use palette_domain::worker::WorkerStatus;
use palette_domain::workflow::WorkflowStatus;
use serde::Serialize;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct ResumeWorkflowResponse {
    pub resumed_count: usize,
}

pub async fn handle_resume_workflow(
    State(state): State<Arc<AppState>>,
) -> Result<Response, (StatusCode, String)> {
    let workers = state
        .interactor
        .data_store
        .list_all_workers()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let suspended: Vec<_> = workers
        .into_iter()
        .filter(|w| w.status == WorkerStatus::Suspended)
        .collect();

    if suspended.is_empty() {
        tracing::info!("resume: no suspended workers found");
        return Ok((
            StatusCode::OK,
            Json(ResumeWorkflowResponse { resumed_count: 0 }),
        )
            .into_response());
    }

    // Collect workflow IDs to update their status
    let workflow_ids: HashSet<_> = suspended.iter().map(|w| w.workflow_id.clone()).collect();

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

        // Send resume command with session_id, or fresh start if no session
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
    for workflow_id in &workflow_ids {
        if let Err(e) = state
            .interactor
            .data_store
            .update_workflow_status(workflow_id, WorkflowStatus::Active)
        {
            tracing::warn!(
                workflow_id = %workflow_id,
                error = %e,
                "failed to update workflow status to Active"
            );
        }
    }

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
