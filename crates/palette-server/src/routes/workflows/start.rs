use crate::AppState;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palette_db::CreateTaskRequest;
use palette_domain::job::{CreateJobRequest, JobId, JobStatus, JobType};
use palette_domain::rule::TaskEffect;
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
    let tree = blueprint.to_task_tree();

    let task_count = register_tasks(&state, &workflow_id, &tree, &req.blueprint_path)?;

    let task_store = build_task_store(&state, &tree, &workflow_id);

    let ready_leaf_ids = resolve_ready_cascade(&task_store)?;

    create_jobs_for_ready_tasks(&state, &ready_leaf_ids, &task_store)?;

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

/// Build a TaskStoreImpl backed by the database.
fn build_task_store<'a>(
    state: &'a AppState,
    tree: &TaskTree,
    workflow_id: &WorkflowId,
) -> TaskStoreImpl<'a> {
    let statuses = tree
        .task_ids()
        .map(|id| (id.clone(), TaskStatus::Pending))
        .collect();
    TaskStoreImpl::new(&state.db, tree.clone(), workflow_id.clone(), statuses)
}

/// Resolve which tasks are Ready, cascading through composite tasks.
/// Returns the IDs of leaf tasks that became Ready.
fn resolve_ready_cascade(task_store: &TaskStoreImpl<'_>) -> HandlerResult<Vec<TaskId>> {
    use palette_domain::rule::TaskRuleEngine;

    // Root task → InProgress
    let root = task_store
        .get_task(task_store.root_id())
        .map_err(internal_err)?
        .ok_or_else(|| internal_err("root task not found"))?;
    task_store
        .update_task_status(&root.id, TaskStatus::InProgress)
        .map_err(internal_err)?;

    let task_engine = TaskRuleEngine::new(task_store);

    let child_ids: Vec<TaskId> = root.children.iter().map(|c| c.id.clone()).collect();
    let initial_effects = task_engine
        .resolve_ready_tasks(&child_ids)
        .map_err(internal_err)?;

    let mut ready_leaf_ids = Vec::new();
    let mut pending = initial_effects;

    while !pending.is_empty() {
        let mut next = Vec::new();

        for effect in &pending {
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

            if *new_status == TaskStatus::Ready {
                let children = task_store.get_child_tasks(task_id).map_err(internal_err)?;
                if children.is_empty() {
                    ready_leaf_ids.push(task_id.clone());
                } else {
                    task_store
                        .update_task_status(task_id, TaskStatus::InProgress)
                        .map_err(internal_err)?;
                    let ids: Vec<TaskId> = children.iter().map(|c| c.id.clone()).collect();
                    let effects = task_engine
                        .resolve_ready_tasks(&ids)
                        .map_err(internal_err)?;
                    next.extend(effects);
                }
            }
        }

        pending = next;
    }

    Ok(ready_leaf_ids)
}

/// Create Jobs for ready leaf tasks that have a job type.
fn create_jobs_for_ready_tasks(
    state: &AppState,
    ready_leaf_ids: &[TaskId],
    task_store: &TaskStoreImpl<'_>,
) -> HandlerResult<()> {
    for task_id in ready_leaf_ids {
        let Some(task) = task_store.get_task(task_id).map_err(internal_err)? else {
            continue;
        };
        if task.job_type.is_none() {
            continue;
        }
        create_job_for_task(state, &task)?;
    }
    Ok(())
}

/// Create a Job for a task and trigger auto-assign for craft jobs.
pub(crate) fn create_job_for_task(
    state: &AppState,
    task: &palette_domain::task::Task,
) -> HandlerResult<()> {
    let job_type = task
        .job_type
        .ok_or_else(|| internal_err("task has no job_type"))?;
    let job = state
        .db
        .create_job(&CreateJobRequest {
            id: Some(JobId::generate(job_type)),
            task_id: Some(task.id.clone()),
            job_type,
            title: task
                .id
                .as_ref()
                .rsplit('/')
                .next()
                .unwrap_or("task")
                .to_string(),
            plan_path: task.plan_path.clone().unwrap_or_default(),
            description: task.description.clone(),
            assignee: None,
            priority: task.priority,
            repository: task.repository.clone(),
            depends_on: vec![],
        })
        .map_err(internal_err)?;

    let initial_status = match job_type {
        JobType::Craft => JobStatus::Ready,
        JobType::Review => JobStatus::Todo,
    };
    state
        .db
        .update_job_status(&job.id, initial_status)
        .map_err(internal_err)?;

    let effects = state
        .rules
        .on_status_change(&job.id, initial_status)
        .map_err(internal_err)?;

    if !effects.is_empty() {
        let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    }

    tracing::info!(
        job_id = %job.id,
        task_id = %task.id,
        job_type = ?job_type,
        "created job for task"
    );
    Ok(())
}
