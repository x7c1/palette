use crate::AppState;
use crate::api_types::blueprint::task_node::{TaskNode, TaskTreeBlueprint};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palette_db::CreateTaskRequest;
use palette_domain::job::{CreateJobRequest, JobId, JobStatus, JobType, Priority};
use palette_domain::rule::{TaskEffect, TaskRuleEngine};
use palette_domain::server::ServerEvent;
use palette_domain::task::{TaskId, TaskStatus, TaskStore};
use palette_domain::workflow::WorkflowId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct StartWorkflowRequest {
    pub blueprint_yaml: String,
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
    let blueprint = TaskTreeBlueprint::parse(&req.blueprint_yaml).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid blueprint YAML: {e}"),
        )
    })?;

    let workflow_id = WorkflowId::generate();
    let root_task_id = TaskId::new(&blueprint.task.id);

    let (task_count, job_infos) =
        register_task_tree(&state, &workflow_id, &root_task_id, &blueprint)?;

    let ready_leaf_ids = resolve_ready_cascade(&state, &root_task_id)?;

    create_jobs_for_ready_tasks(&state, &job_infos, &ready_leaf_ids)?;

    tracing::info!(
        workflow_id = %workflow_id,
        root_task = %blueprint.task.id,
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

/// Metadata about a task's job, collected during task tree creation.
struct TaskJobInfo {
    task_id: TaskId,
    job_type: JobType,
    plan_path: Option<String>,
    description: Option<String>,
    priority: Option<Priority>,
    repository: Option<palette_domain::job::Repository>,
}

type HandlerResult<T> = Result<T, (StatusCode, String)>;

fn internal_err(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

/// Create the workflow and all tasks in the DB from a blueprint.
/// Returns (task_count, job_infos for leaf tasks with a job type).
fn register_task_tree(
    state: &AppState,
    workflow_id: &WorkflowId,
    root_task_id: &TaskId,
    blueprint: &TaskTreeBlueprint,
) -> HandlerResult<(usize, Vec<TaskJobInfo>)> {
    state
        .db
        .create_workflow(workflow_id, &format!("inline:{}", blueprint.task.id))
        .map_err(internal_err)?;

    state
        .db
        .create_task(&CreateTaskRequest {
            id: root_task_id.clone(),
            workflow_id: workflow_id.clone(),
            parent_id: None,
            title: blueprint.task.title.clone(),
            plan_path: blueprint.task.plan_path.clone(),
            depends_on: vec![],
        })
        .map_err(internal_err)?;

    let mut all_task_ids = vec![root_task_id.clone()];
    let mut job_infos = Vec::new();
    register_children(
        state,
        workflow_id,
        root_task_id,
        &blueprint.task.id,
        &blueprint.children,
        &mut all_task_ids,
        &mut job_infos,
    )?;

    Ok((all_task_ids.len(), job_infos))
}

fn register_children(
    state: &AppState,
    workflow_id: &WorkflowId,
    parent_task_id: &TaskId,
    parent_id_str: &str,
    children: &[TaskNode],
    all_task_ids: &mut Vec<TaskId>,
    job_infos: &mut Vec<TaskJobInfo>,
) -> HandlerResult<()> {
    for child in children {
        let child_id_str = format!("{parent_id_str}/{}", child.id);
        let child_task_id = TaskId::new(&child_id_str);

        state
            .db
            .create_task(&CreateTaskRequest {
                id: child_task_id.clone(),
                workflow_id: workflow_id.clone(),
                parent_id: Some(parent_task_id.clone()),
                title: child.title.clone().unwrap_or_else(|| child.id.clone()),
                plan_path: child.plan_path.clone(),
                depends_on: child
                    .depends_on
                    .iter()
                    .map(|dep| TaskId::new(format!("{parent_id_str}/{dep}")))
                    .collect(),
            })
            .map_err(internal_err)?;

        all_task_ids.push(child_task_id.clone());

        if let Some(ref jt) = child.job_type {
            job_infos.push(TaskJobInfo {
                task_id: child_task_id.clone(),
                job_type: JobType::from(*jt),
                plan_path: child.plan_path.clone(),
                description: child.description.clone(),
                priority: child.priority.map(Priority::from),
                repository: child
                    .repository
                    .as_ref()
                    .map(|r| palette_domain::job::Repository {
                        name: r.name.clone(),
                        branch: r.branch.clone(),
                    }),
            });
        }

        if !child.children.is_empty() {
            register_children(
                state,
                workflow_id,
                &child_task_id,
                &child_id_str,
                &child.children,
                all_task_ids,
                job_infos,
            )?;
        }
    }
    Ok(())
}

/// Resolve which tasks are Ready, cascading through composite tasks.
/// Returns the IDs of leaf tasks that became Ready.
fn resolve_ready_cascade(state: &AppState, root_task_id: &TaskId) -> HandlerResult<Vec<TaskId>> {
    state
        .db
        .update_task_status(root_task_id, TaskStatus::InProgress)
        .map_err(internal_err)?;

    let task_engine = TaskRuleEngine::new(&*state.db);

    let root_children = state
        .db
        .get_child_tasks(root_task_id)
        .map_err(internal_err)?;
    let child_ids: Vec<TaskId> = root_children.iter().map(|c| c.id.clone()).collect();
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

            state
                .db
                .update_task_status(task_id, *new_status)
                .map_err(internal_err)?;
            tracing::info!(task_id = %task_id, status = ?new_status, "task status changed");

            if *new_status == TaskStatus::Ready {
                let children = state.db.get_child_tasks(task_id).map_err(internal_err)?;
                if children.is_empty() {
                    ready_leaf_ids.push(task_id.clone());
                } else {
                    state
                        .db
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
    job_infos: &[TaskJobInfo],
    ready_leaf_ids: &[TaskId],
) -> HandlerResult<()> {
    for info in job_infos {
        if !ready_leaf_ids.contains(&info.task_id) {
            continue;
        }

        let job = state
            .db
            .create_job(&CreateJobRequest {
                id: Some(JobId::generate(info.job_type)),
                task_id: Some(info.task_id.clone()),
                job_type: info.job_type,
                title: info
                    .task_id
                    .as_ref()
                    .rsplit('/')
                    .next()
                    .unwrap_or("task")
                    .to_string(),
                plan_path: info.plan_path.clone().unwrap_or_default(),
                description: info.description.clone(),
                assignee: None,
                priority: info.priority,
                repository: info.repository.clone(),
                depends_on: vec![],
            })
            .map_err(internal_err)?;

        if info.job_type == JobType::Craft {
            state
                .db
                .update_job_status(&job.id, JobStatus::Ready)
                .map_err(internal_err)?;

            let effects = state
                .rules
                .on_status_change(&job.id, JobStatus::Ready)
                .map_err(internal_err)?;

            if !effects.is_empty() {
                let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
            }
        }

        tracing::info!(
            job_id = %job.id,
            task_id = %info.task_id,
            job_type = ?info.job_type,
            "created job for ready task"
        );
    }
    Ok(())
}
