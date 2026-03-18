use crate::AppState;
use crate::api_types::BlueprintResponse;
use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

pub async fn handle_list_blueprints(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<BlueprintResponse>>, (StatusCode, String)> {
    let blueprints = state
        .db
        .list_blueprints()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(
        blueprints
            .into_iter()
            .map(BlueprintResponse::from)
            .collect(),
    ))
}
