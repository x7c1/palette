use crate::AppState;
use crate::api_types::{TaskFilterApi, TaskResponse};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use std::sync::Arc;

pub async fn handle_list_tasks(
    State(state): State<Arc<AppState>>,
    Query(api_filter): Query<TaskFilterApi>,
) -> Result<Json<Vec<TaskResponse>>, (StatusCode, String)> {
    let filter = api_filter.into();
    let tasks = state
        .db
        .list_tasks(&filter)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(tasks.into_iter().map(TaskResponse::from).collect()))
}
