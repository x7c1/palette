use crate::AppState;
use crate::api_types::Blueprint;
use axum::{extract::State, http::StatusCode};
use chrono::Utc;
use palette_domain::blueprint::SaveBlueprintRequest;
use std::sync::Arc;

pub async fn handle_submit_blueprint(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Result<StatusCode, (StatusCode, String)> {
    let blueprint = Blueprint::parse(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid YAML: {e}")))?;

    let req = SaveBlueprintRequest {
        task_id: blueprint.task.id.clone(),
        title: blueprint.task.title.clone(),
        yaml: body,
        created_at: Utc::now(),
    };

    state
        .db
        .save_blueprint(&req)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!(task_id = %req.task_id, "blueprint submitted");
    Ok(StatusCode::CREATED)
}
