use crate::api_types::{ErrorCode, InputError, Location};
use crate::{AppState, Error, ValidJson};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palette_domain::job::JobDetail;
use palette_domain::server::ServerEvent;
use palette_domain::task::TaskTree;
use palette_domain::workflow::WorkflowId;
use palette_usecase::{CreateTaskRequest, validate_blueprint};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct StartWorkflowRequest {
    pub blueprint_path: String,
}

#[derive(Debug, Serialize)]
pub struct StartWorkflowResponse {
    pub workflow_id: String,
    pub task_count: usize,
}

pub async fn handle_start_workflow(
    State(state): State<Arc<AppState>>,
    ValidJson(req): ValidJson<StartWorkflowRequest>,
) -> crate::Result<Response> {
    let workflow_id = WorkflowId::generate();
    let tree = validate_blueprint(
        state.interactor.blueprint.as_ref(),
        Path::new(&req.blueprint_path),
        &workflow_id,
    )
    .map_err(super::blueprint_read_error_to_server_error)?;

    // Work-branch collision: reject the start if any active workflow already
    // owns one of the (repo, work_branch) pairs used by this blueprint's
    // craft tasks. Performed before `create_workflow` so the rejection
    // leaves no workflow row behind.
    let conflicts = collect_work_branch_conflicts(&state, &tree)?;
    if !conflicts.is_empty() {
        return Err(Error::BadRequest {
            code: ErrorCode::InputValidationFailed,
            errors: conflicts,
        });
    }

    let task_count = register_tasks(&state, &workflow_id, &tree, &req.blueprint_path)?;

    // Send domain event — orchestrator handles task activation
    let _ = state.event_tx.send(ServerEvent::ActivateWorkflow {
        workflow_id: workflow_id.clone(),
    });

    tracing::info!(
        workflow_id = %workflow_id,
        root_task = %tree.root_id(),
        task_count = task_count,
        "started workflow"
    );

    Ok((
        StatusCode::CREATED,
        Json(StartWorkflowResponse {
            workflow_id: workflow_id.to_string(),
            task_count,
        }),
    )
        .into_response())
}

/// Gather one [`InputError`] per craft task whose `(repo, work_branch)` pair
/// is already claimed by a non-terminal workflow.
///
/// The `hint` field carries `task_key:repo_name:work_branch` so clients can
/// pinpoint which craft task needs attention when a blueprint references
/// multiple repos.
fn collect_work_branch_conflicts(
    state: &AppState,
    tree: &TaskTree,
) -> crate::Result<Vec<InputError>> {
    let mut errors = Vec::new();
    for task_id in tree.task_ids() {
        let Some(node) = tree.get(task_id) else {
            continue;
        };
        let Some(JobDetail::Craft { repository }) = node.job_detail.as_ref() else {
            continue;
        };
        let in_use = state
            .interactor
            .data_store
            .find_active_workflows_using_work_branch(&repository.name, &repository.work_branch)
            .map_err(Error::internal)?;
        if !in_use.is_empty() {
            errors.push(InputError {
                location: Location::Body,
                hint: format!(
                    "{}:{}:{}",
                    node.key.as_ref(),
                    repository.name,
                    repository.work_branch
                ),
                reason: "workflow/work_branch_in_use".into(),
            });
        }
    }
    Ok(errors)
}

/// Create the workflow and register all task IDs (with Pending status) in the DB.
pub(super) fn register_tasks(
    state: &AppState,
    workflow_id: &WorkflowId,
    tree: &TaskTree,
    blueprint_path: &str,
) -> crate::Result<usize> {
    state
        .interactor
        .data_store
        .create_workflow(workflow_id, blueprint_path)
        .map_err(Error::internal)?;

    let mut count = 0;
    for task_id in tree.task_ids() {
        state
            .interactor
            .data_store
            .create_task(&CreateTaskRequest {
                id: task_id.clone(),
                workflow_id: workflow_id.clone(),
            })
            .map_err(Error::internal)?;
        count += 1;
    }

    Ok(count)
}
