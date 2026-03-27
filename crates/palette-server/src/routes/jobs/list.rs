use crate::AppState;
use crate::api_types::{JobFilter, JobResponse};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use std::sync::Arc;

pub async fn handle_list_jobs(
    State(state): State<Arc<AppState>>,
    Query(api_filter): Query<JobFilter>,
) -> Result<Json<Vec<JobResponse>>, (StatusCode, String)> {
    let filter = api_filter.into();
    let jobs = state
        .interactor
        .data_store
        .list_jobs(&filter)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(jobs.into_iter().map(JobResponse::from).collect()))
}
