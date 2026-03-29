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
use palette_domain::worker::WorkerStatus;
use palette_domain::workflow::{WorkflowId, WorkflowStatus};
use palette_usecase::TaskRuleEngine;
use palette_usecase::reconciliation;
use palette_usecase::task_store::TaskStore;
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct ResumeWorkflowResponse {
    pub resumed_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconciliation: Option<ReconciliationSummary>,
}

#[derive(Debug, Serialize)]
pub struct ReconciliationSummary {
    pub tasks_created: usize,
    pub tasks_deleted: usize,
}

pub async fn handle_resume_workflow(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    let workflow_id = WorkflowId::new(&id);

    // --- Blueprint Reconciliation ---
    let reconciliation_summary = reconcile_blueprint(&state, &workflow_id)?;

    // --- Resume Workers ---
    let workers = state
        .interactor
        .data_store
        .list_all_workers()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let suspended: Vec<_> = workers
        .into_iter()
        .filter(|w| w.status == WorkerStatus::Suspended && w.workflow_id == workflow_id)
        .collect();

    if suspended.is_empty() {
        tracing::info!(workflow_id = %workflow_id, "resume: no suspended workers found");
        return Ok((
            StatusCode::OK,
            Json(ResumeWorkflowResponse {
                resumed_count: 0,
                reconciliation: reconciliation_summary,
            }),
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

    // Send event to orchestrator to spawn readiness watchers
    if !resumed_ids.is_empty() {
        let _ = state.event_tx.send(ServerEvent::ResumeWorkers {
            worker_ids: resumed_ids,
        });
    }

    tracing::info!(resumed_count, "resume complete");

    Ok((
        StatusCode::OK,
        Json(ResumeWorkflowResponse {
            resumed_count,
            reconciliation: reconciliation_summary,
        }),
    )
        .into_response())
}

/// Run Blueprint reconciliation: validate changes and apply them to the DB.
/// Returns None if no changes were detected, or a summary of the reconciliation.
fn reconcile_blueprint(
    state: &AppState,
    workflow_id: &WorkflowId,
) -> Result<Option<ReconciliationSummary>, (StatusCode, String)> {
    // Load workflow to get blueprint path
    let workflow = state
        .interactor
        .data_store
        .get_workflow(workflow_id)
        .map_err(internal_err)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("workflow {} not found", workflow_id),
            )
        })?;

    // Re-read Blueprint from disk
    let tree = state
        .interactor
        .blueprint
        .read_blueprint(std::path::Path::new(&workflow.blueprint_path), workflow_id)
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
        .get_task_statuses(workflow_id)
        .map_err(internal_err)?;

    // Compute diff
    let diff = reconciliation::compute_diff(&tree, &db_statuses);

    if diff.added_tasks.is_empty() && diff.removed_tasks.is_empty() {
        tracing::info!(workflow_id = %workflow_id, "reconciliation: no blueprint changes");
        return Ok(None);
    }

    // Validate
    let validation = reconciliation::validate_diff(&diff, &tree, &db_statuses);
    if !validation.is_valid() {
        let messages: Vec<String> = validation
            .errors
            .iter()
            .map(|e| format!("{}: {}", e.task_id, e.message))
            .collect();
        return Err((
            StatusCode::BAD_REQUEST,
            format!("blueprint validation failed: {}", messages.join("; ")),
        ));
    }

    // Execute reconciliation
    let result = reconciliation::reconcile(
        state.interactor.data_store.as_ref(),
        &diff,
        workflow_id,
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

    // Activate newly Ready tasks (Pending tasks whose dependencies are now met)
    let updated_statuses = state
        .interactor
        .data_store
        .get_task_statuses(workflow_id)
        .map_err(internal_err)?;

    let task_store = TaskStore::new(
        state.interactor.data_store.as_ref(),
        tree,
        workflow_id.clone(),
        updated_statuses,
    );
    let task_engine = TaskRuleEngine::new(&task_store);

    // Collect all Pending task IDs for ready-resolution
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
        let effects = activate_ready_children(state, &task_store, &task_engine, &pending_ids)?;
        if !effects.is_empty() {
            let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
        }
    }

    Ok(Some(ReconciliationSummary {
        tasks_created: result.tasks_created,
        tasks_deleted: result.tasks_deleted,
    }))
}
