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

/// Extract the distinct `(repo_name, work_branch)` pairs used by every Craft
/// task in the tree so the data store can claim them atomically with workflow
/// creation. A single workflow may legitimately run multiple Craft tasks on
/// the same branch (sequential phases), so duplicates are collapsed here.
fn collect_branch_claims(tree: &TaskTree) -> Vec<(String, String)> {
    let mut seen = std::collections::HashSet::new();
    let mut claims = Vec::new();
    for task_id in tree.task_ids() {
        let Some(node) = tree.get(task_id) else {
            continue;
        };
        if let Some(JobDetail::Craft { repository }) = node.job_detail.as_ref() {
            let pair = (repository.name.clone(), repository.work_branch.clone());
            if seen.insert(pair.clone()) {
                claims.push(pair);
            }
        }
    }
    claims
}

/// Build one [`InputError`] per conflicting `(repo, work_branch)` pair. The
/// `hint` field carries `repo_name:work_branch` so clients can see exactly
/// which pair collided when a blueprint references multiple repos.
fn conflicts_to_errors(conflicts: Vec<(String, String)>) -> Vec<InputError> {
    conflicts
        .into_iter()
        .map(|(repo_name, work_branch)| InputError {
            location: Location::Body,
            hint: format!("{repo_name}:{work_branch}"),
            reason: "workflow/work_branch_in_use".into(),
        })
        .collect()
}

/// Create the workflow and register all task IDs (with Pending status) in the
/// DB. Branch claims for every Craft task are inserted atomically with the
/// workflow row, so a second start sharing any `(repo, work_branch)` pair
/// fails at the DB level — no race window between check and insert.
pub(super) fn register_tasks(
    state: &AppState,
    workflow_id: &WorkflowId,
    tree: &TaskTree,
    blueprint_path: &str,
) -> crate::Result<usize> {
    let claims = collect_branch_claims(tree);
    let conflicts = state
        .interactor
        .data_store
        .create_workflow_with_branch_claims(workflow_id, blueprint_path, &claims)
        .map_err(Error::internal)?;
    if !conflicts.is_empty() {
        return Err(Error::BadRequest {
            code: ErrorCode::InputValidationFailed,
            errors: conflicts_to_errors(conflicts),
        });
    }

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
