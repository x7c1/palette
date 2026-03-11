use crate::AppState;
use crate::api_types::{CreateJobRequest, JobResponse};
use axum::{Json, extract::State, http::StatusCode};
use palette_domain as domain;
use std::sync::Arc;

pub async fn handle_create_job(
    State(state): State<Arc<AppState>>,
    Json(api_req): Json<CreateJobRequest>,
) -> Result<(StatusCode, Json<JobResponse>), (StatusCode, String)> {
    let req: domain::job::CreateJobRequest = api_req.into();
    let job = state
        .db
        .create_job(&req)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    tracing::info!(job_id = %job.id, "created job");
    Ok((StatusCode::CREATED, Json(JobResponse::from(job))))
}
