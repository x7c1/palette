use crate::AppState;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palette_db::CreateTaskRequest;
use palette_domain::rule::{RuleEffect, TaskEffect, TaskRuleEngine};
use palette_domain::server::ServerEvent;
use palette_domain::task::{TaskId, TaskStatus, TaskStore, TaskTree};
use palette_domain::workflow::WorkflowId;
use palette_fs::read_blueprint;
use palette_service::TaskStoreImpl;
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
) -> Result<Response, (StatusCode, String)> {
    let blueprint = read_blueprint(Path::new(&req.blueprint_path))
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let workflow_id = WorkflowId::generate();
    let tree = blueprint.to_task_tree(&workflow_id);

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

type HandlerResult<T> = Result<T, (StatusCode, String)>;

fn internal_err(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

/// Create the workflow and register all task IDs (with Pending status) in the DB.
fn register_tasks(
    state: &AppState,
    workflow_id: &WorkflowId,
    tree: &TaskTree,
    blueprint_path: &str,
) -> HandlerResult<usize> {
    state
        .db
        .create_workflow(workflow_id, blueprint_path)
        .map_err(internal_err)?;

    let mut count = 0;
    for task_id in tree.task_ids() {
        state
            .db
            .create_task(&CreateTaskRequest {
                id: task_id.clone(),
                workflow_id: workflow_id.clone(),
            })
            .map_err(internal_err)?;
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
) -> HandlerResult<Vec<RuleEffect>> {
    let statuses = tree
        .task_ids()
        .map(|id| (id.clone(), TaskStatus::Pending))
        .collect();
    let task_store = TaskStoreImpl::new(&state.db, tree.clone(), workflow_id.clone(), statuses);
    let task_engine = TaskRuleEngine::new(&task_store);

    // Root task → InProgress
    let root = task_store
        .get_task(task_store.root_id())
        .map_err(internal_err)?
        .ok_or_else(|| internal_err("root task not found"))?;
    task_store
        .update_task_status(&root.id, TaskStatus::InProgress)
        .map_err(internal_err)?;

    // Resolve children recursively and create jobs
    let child_ids: Vec<TaskId> = root.children.iter().map(|c| c.id.clone()).collect();
    activate_ready_children(state, &task_store, &task_engine, &child_ids)
}

/// Resolve which tasks become Ready, activate them, and recurse into composites.
fn activate_ready_children(
    state: &AppState,
    task_store: &TaskStoreImpl<'_>,
    task_engine: &TaskRuleEngine<&TaskStoreImpl<'_>>,
    child_ids: &[TaskId],
) -> HandlerResult<Vec<RuleEffect>> {
    let task_effects = task_engine
        .resolve_ready_tasks(child_ids)
        .map_err(internal_err)?;

    let mut effects = Vec::new();

    for effect in &task_effects {
        let TaskEffect::TaskStatusChanged {
            task_id,
            new_status,
        } = effect
        else {
            continue;
        };

        task_store
            .update_task_status(task_id, *new_status)
            .map_err(internal_err)?;
        tracing::info!(task_id = %task_id, status = ?new_status, "task status changed");

        if *new_status != TaskStatus::Ready {
            continue;
        }

        let Some(task) = task_store.get_task(task_id).map_err(internal_err)? else {
            continue;
        };
        let children = task_store.get_child_tasks(task_id).map_err(internal_err)?;

        if let Some(job_type) = task.job_type {
            // Task with job: create job and set composite to InProgress
            if !children.is_empty() {
                task_store
                    .update_task_status(task_id, TaskStatus::InProgress)
                    .map_err(internal_err)?;
            }
            let job_effects = create_job(state, &task, job_type)?;
            effects.extend(job_effects);
        } else if !children.is_empty() {
            // Pure composite: InProgress and recurse into children
            task_store
                .update_task_status(task_id, TaskStatus::InProgress)
                .map_err(internal_err)?;
            let ids: Vec<TaskId> = children.iter().map(|c| c.id.clone()).collect();
            let child_effects = activate_ready_children(state, task_store, task_engine, &ids)?;
            effects.extend(child_effects);
        }
    }

    Ok(effects)
}

/// Create a Job for a task and return AutoAssign effects.
fn create_job(
    state: &AppState,
    task: &palette_domain::task::Task,
    job_type: palette_domain::job::JobType,
) -> HandlerResult<Vec<RuleEffect>> {
    let job = state
        .db
        .create_job(&palette_domain::job::CreateJobRequest {
            id: Some(palette_domain::job::JobId::generate(job_type)),
            task_id: task.id.clone(),
            job_type,
            title: task.key.clone(),
            plan_path: task.plan_path.clone().unwrap_or_default(),
            description: task.description.clone(),
            assignee: None,
            priority: task.priority,
            repository: task.repository.clone(),
        })
        .map_err(internal_err)?;

    let todo_status = palette_domain::job::JobStatus::todo(job_type);
    let effects = state
        .rules
        .on_status_change(&job.id, todo_status)
        .map_err(internal_err)?;

    tracing::info!(
        job_id = %job.id,
        task_id = %task.id,
        job_type = ?job_type,
        "created job for task"
    );

    Ok(effects)
}
