use crate::api_types::{JobFilter, JobResponse};
use crate::{AppState, Error};
use axum::{
    Json,
    extract::{Query, State},
};
use std::sync::Arc;

pub async fn handle_list_jobs(
    State(state): State<Arc<AppState>>,
    Query(api_filter): Query<JobFilter>,
) -> crate::Result<Json<Vec<JobResponse>>> {
    let filter = api_filter.into();
    let jobs = state
        .interactor
        .data_store
        .list_jobs(&filter)
        .map_err(Error::internal)?;
    Ok(Json(jobs.into_iter().map(JobResponse::from).collect()))
}
