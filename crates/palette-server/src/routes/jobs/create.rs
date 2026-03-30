use crate::api_types::{CreateJobRequest, ErrorCode, JobResponse};
use crate::{AppState, Error};
use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

pub async fn handle_create_job(
    State(state): State<Arc<AppState>>,
    Json(api_req): Json<CreateJobRequest>,
) -> crate::Result<(StatusCode, Json<JobResponse>)> {
    let req = api_req
        .validate()
        .map_err(|field_hints| Error::BadRequest {
            code: ErrorCode::InputValidationFailed,
            field_hints,
        })?;
    let job = state
        .interactor
        .data_store
        .create_job(&req)
        .map_err(Error::internal)?;
    tracing::info!(job_id = %job.id, "created job");
    Ok((StatusCode::CREATED, Json(JobResponse::from(job))))
}
