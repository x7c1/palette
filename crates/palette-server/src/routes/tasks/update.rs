use crate::AppState;
use crate::api_types::{TaskResponse, UpdateTaskRequest};
use axum::{Json, extract::State, http::StatusCode};
use palette_domain::{self as domain, RuleEngine, ServerEvent};
use std::sync::Arc;

pub async fn handle_update_task(
    State(state): State<Arc<AppState>>,
    Json(api_req): Json<UpdateTaskRequest>,
) -> Result<Json<TaskResponse>, (StatusCode, String)> {
    let req: domain::UpdateTaskRequest = api_req.into();
    let current = state
        .db
        .get_task(&req.id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (StatusCode::NOT_FOUND, {
                let id = &req.id;
                format!("task not found: {id}")
            })
        })?;

    RuleEngine::validate_transition(current.task_type, current.status, req.status)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let task = state
        .db
        .update_task_status(&req.id, req.status)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Apply rule engine side effects
    let effects = state
        .rules
        .on_status_change(state.db.as_ref(), &req.id, req.status)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for effect in &effects {
        tracing::info!(?effect, "rule engine effect");
    }

    if !effects.is_empty() {
        let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    }

    Ok(Json(TaskResponse::from(task)))
}
