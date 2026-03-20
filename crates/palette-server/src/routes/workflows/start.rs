use crate::AppState;
use crate::api_types::blueprint::task_node::{TaskNode, TaskTreeBlueprint};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palette_db::database::CreateTaskRequest;
use palette_domain::task::TaskId;
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

    // Recursively create child tasks
    let mut task_count = 1;
    create_child_tasks(
        &state,
        &workflow_id,
        &root_task_id,
        &blueprint.task.id,
        &blueprint.children,
        &mut task_count,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

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
    task_count: &mut usize,
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

        *task_count += 1;

        // Recurse into grandchildren
        if !child.children.is_empty() {
            create_child_tasks(
                state,
                workflow_id,
                &child_task_id,
                &child_id_str,
                &child.children,
                task_count,
            )?;
        }
    }
    Ok(())
}
