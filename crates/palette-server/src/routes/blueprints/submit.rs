use crate::AppState;
use crate::api_types::{Blueprint, BlueprintResponse};
use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

pub async fn handle_submit_blueprint(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Result<(StatusCode, Json<BlueprintResponse>), (StatusCode, String)> {
    let blueprint = Blueprint::parse(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid YAML: {e}")))?;

    let task_id = blueprint.task.id.clone();
    let title = blueprint.task.title.clone();

    let stored = state
        .db
        .save_blueprint(&task_id, &title, &body)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!(task_id = %task_id, "blueprint submitted");
    Ok((StatusCode::CREATED, Json(BlueprintResponse::from(stored))))
}
