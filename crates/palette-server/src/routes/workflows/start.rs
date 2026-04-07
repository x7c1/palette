use crate::{AppState, Error, ValidJson};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palette_domain::server::ServerEvent;
use palette_domain::task::TaskTree;
use palette_domain::workflow::WorkflowId;
use palette_usecase::CreateTaskRequest;
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
    let tree = state
        .interactor
        .blueprint
        .read_blueprint(Path::new(&req.blueprint_path), &workflow_id)
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
