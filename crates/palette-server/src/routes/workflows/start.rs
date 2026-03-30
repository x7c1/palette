use super::task_activation::activate_ready_children;
use crate::{AppState, Error};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palette_domain::rule::RuleEffect;
use palette_domain::server::ServerEvent;
use palette_domain::task::{TaskId, TaskStatus, TaskTree};
use palette_domain::workflow::WorkflowId;
use palette_usecase::TaskRuleEngine;
use palette_usecase::data_store::CreateTaskRequest;
use palette_usecase::task_store::TaskStore;
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
    Json(req): Json<StartWorkflowRequest>,
) -> crate::Result<Response> {
    let workflow_id = WorkflowId::generate();
    let tree = state
        .interactor
        .blueprint
        .read_blueprint(Path::new(&req.blueprint_path), &workflow_id)
        .map_err(super::blueprint_read_error_to_server_error)?;

    let task_count = register_tasks(&state, &workflow_id, &tree, &req.blueprint_path)?;

    let effects = initialize_root(&state, &tree, &workflow_id)?;

    if !effects.is_empty() {
        let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    }

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
fn register_tasks(
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

/// Set the root task to InProgress and cascade Ready resolution.
/// Creates jobs for Ready tasks with job_type and returns AutoAssign effects.
fn initialize_root(
    state: &AppState,
    tree: &TaskTree,
    workflow_id: &WorkflowId,
) -> crate::Result<Vec<RuleEffect>> {
    let statuses = tree
        .task_ids()
        .map(|id| (id.clone(), TaskStatus::Pending))
        .collect();
    let task_store = TaskStore::new(
        state.interactor.data_store.as_ref(),
        tree.clone(),
        workflow_id.clone(),
        statuses,
    );
    let task_engine = TaskRuleEngine::new(&task_store);

    // Root task: spawn supervisor, then → InProgress
    let root = task_store
        .get_task(task_store.root_id())
        .map_err(Error::internal)?
        .ok_or_else(|| Error::internal("root task not found"))?;

    let mut effects = vec![RuleEffect::SpawnSupervisor {
        task_id: root.id.clone(),
        role: palette_domain::worker::WorkerRole::Leader,
    }];

    task_store
        .update_task_status(&root.id, TaskStatus::InProgress)
        .map_err(Error::internal)?;

    // Resolve children recursively and create jobs
    let child_ids: Vec<TaskId> = root.children.iter().map(|c| c.id.clone()).collect();
    let child_effects = activate_ready_children(state, &task_store, &task_engine, &child_ids)?;
    effects.extend(child_effects);
    Ok(effects)
}
