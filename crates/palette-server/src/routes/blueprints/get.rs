use crate::AppState;
use crate::api_types::BlueprintResponse;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use std::sync::Arc;

pub async fn handle_get_blueprint(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<String>,
) -> Result<Json<BlueprintResponse>, (StatusCode, String)> {
    let blueprint = state
        .db
        .get_blueprint(&task_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("blueprint not found: {task_id}"),
            )
        })?;

    Ok(Json(BlueprintResponse::from(blueprint)))
}
