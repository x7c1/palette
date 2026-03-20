use crate::AppState;
use crate::api_types::blueprint::task_node::{TaskNode, TaskTreeBlueprint};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palette_db::database::CreateTaskRequest;
use palette_domain::job::{CreateJobRequest, JobId, JobStatus, JobType, Priority};
use palette_domain::rule::{TaskEffect, TaskRuleEngine};
use palette_domain::server::ServerEvent;
use palette_domain::task::{TaskId, TaskStatus};
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

/// Metadata about a task's job, collected during task tree creation.
struct TaskJobInfo {
    task_id: TaskId,
    job_type: JobType,
    plan_path: Option<String>,
    description: Option<String>,
    priority: Option<Priority>,
    repository: Option<palette_domain::job::Repository>,
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

    // Create the workflow
    state
        .db
        .create_workflow(&workflow_id, &format!("inline:{}", blueprint.task.id))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Create the root task
    let root_task_id = TaskId::new(&blueprint.task.id);
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
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Recursively create child tasks, collecting task IDs and job info
    let mut all_task_ids = vec![root_task_id.clone()];
    let mut job_infos: Vec<TaskJobInfo> = Vec::new();
    create_child_tasks(
        &state,
        &workflow_id,
        &root_task_id,
        &blueprint.task.id,
        &blueprint.children,
        &mut all_task_ids,
        &mut job_infos,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let task_count = all_task_ids.len();

    // Transition root task to InProgress
    state
        .db
        .update_task_status(&root_task_id, TaskStatus::InProgress)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Resolve initial ready tasks and cascade through composite tasks.
    // Composite tasks that become Ready are immediately transitioned to InProgress,
    // which unlocks their children for Ready resolution.
    // Leaf tasks that become Ready are collected for Job creation.
    let task_engine = TaskRuleEngine::new(&*state.db);
    let initial_effects = task_engine
        .resolve_ready_tasks(&all_task_ids)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut ready_leaf_task_ids: Vec<TaskId> = Vec::new();
    let mut pending_effects = initial_effects;

    while !pending_effects.is_empty() {
        let mut next_effects = Vec::new();

        for effect in &pending_effects {
            if let TaskEffect::TaskStatusChanged {
                task_id,
                new_status,
            } = effect
            {
                state
                    .db
                    .update_task_status(task_id, *new_status)
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                tracing::info!(task_id = %task_id, status = ?new_status, "task status changed");

                if *new_status == TaskStatus::Ready {
                    let children = state
                        .db
                        .get_child_tasks(task_id)
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                    if !children.is_empty() {
                        // Composite task: transition to InProgress and resolve children
                        state
                            .db
                            .update_task_status(task_id, TaskStatus::InProgress)
                            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                        let child_ids: Vec<TaskId> =
                            children.iter().map(|c| c.id.clone()).collect();
                        let child_effects = task_engine
                            .resolve_ready_tasks(&child_ids)
                            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                        next_effects.extend(child_effects);
                    } else {
                        // Leaf task: collect for Job creation
                        ready_leaf_task_ids.push(task_id.clone());
                    }
                }
            }
        }

        pending_effects = next_effects;
    }

    // Create Jobs for Ready leaf tasks that have a job_type
    for info in &job_infos {
        if !ready_leaf_task_ids.contains(&info.task_id) {
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
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Transition craft jobs to Ready to trigger auto-assign
        if info.job_type == JobType::Craft {
            state
                .db
                .update_job_status(&job.id, JobStatus::Ready)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            let job_effects = state
                .rules
                .on_status_change(&job.id, JobStatus::Ready)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            if !job_effects.is_empty() {
                let _ = state.event_tx.send(ServerEvent::ProcessEffects {
                    effects: job_effects,
                });
            }
        }

        tracing::info!(
            job_id = %job.id,
            task_id = %info.task_id,
            job_type = ?info.job_type,
            "created job for ready task"
        );
    }

    tracing::info!(
        workflow_id = %workflow_id,
        root_task = %blueprint.task.id,
        task_count = task_count,
        "started workflow"
    );

    let response = StartWorkflowResponse {
        workflow_id: workflow_id.to_string(),
        task_count,
    };

    Ok((StatusCode::CREATED, Json(response)).into_response())
}

fn create_child_tasks(
    state: &AppState,
    workflow_id: &WorkflowId,
    parent_task_id: &TaskId,
    parent_id_str: &str,
    children: &[TaskNode],
    all_task_ids: &mut Vec<TaskId>,
    job_infos: &mut Vec<TaskJobInfo>,
) -> Result<(), String> {
    for child in children {
        let child_id_str = format!("{parent_id_str}/{}", child.id);
        let child_task_id = TaskId::new(&child_id_str);

        let depends_on: Vec<TaskId> = child
            .depends_on
            .iter()
            .map(|dep| TaskId::new(format!("{parent_id_str}/{dep}")))
            .collect();

        let title = child.title.clone().unwrap_or_else(|| child.id.clone());

        state
            .db
            .create_task(&CreateTaskRequest {
                id: child_task_id.clone(),
                workflow_id: workflow_id.clone(),
                parent_id: Some(parent_task_id.clone()),
                title,
                plan_path: child.plan_path.clone(),
                depends_on,
            })
            .map_err(|e| e.to_string())?;

        all_task_ids.push(child_task_id.clone());

        // Collect job info for leaf tasks with a job type
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

        // Recurse into grandchildren
        if !child.children.is_empty() {
            create_child_tasks(
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
