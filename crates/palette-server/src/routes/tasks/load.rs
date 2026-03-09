use crate::AppState;
use crate::api_types::{TaskFile, TaskResponse};
use axum::{Json, extract::State, http::StatusCode};
use palette_domain::{RuleEngine, ServerEvent, TaskId, TaskStatus, TaskType};
use std::sync::Arc;

pub async fn handle_load_tasks(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Result<(StatusCode, Json<Vec<TaskResponse>>), (StatusCode, String)> {
    let task_file = TaskFile::parse(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid YAML: {e}")))?;

    let requests = task_file.into_requests();
    let mut created_tasks = Vec::new();

    // Create all tasks as draft
    for req in &requests {
        let task = state
            .db
            .create_task(req)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        tracing::info!(task_id = %task.id, "created task from YAML");
        created_tasks.push(task);
    }

    // Transition work tasks to ready, triggering auto-assign via rule engine
    let work_ids: Vec<TaskId> = created_tasks
        .iter()
        .filter(|t| t.task_type == TaskType::Work)
        .map(|t| t.id.clone())
        .collect();

    for work_id in &work_ids {
        RuleEngine::validate_transition(TaskType::Work, TaskStatus::Draft, TaskStatus::Ready)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        state
            .db
            .update_task_status(work_id, TaskStatus::Ready)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let effects = state
            .rules
            .on_status_change(state.db.as_ref(), work_id, TaskStatus::Ready)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        for effect in &effects {
            tracing::info!(?effect, "rule engine effect (task load)");
        }

        if !effects.is_empty() {
            let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
        }
    }

    // Re-fetch all tasks to return updated statuses
    let final_tasks: Vec<TaskResponse> = created_tasks
        .iter()
        .filter_map(|t| state.db.get_task(&t.id).ok().flatten())
        .map(TaskResponse::from)
        .collect();

    Ok((StatusCode::CREATED, Json(final_tasks)))
}
