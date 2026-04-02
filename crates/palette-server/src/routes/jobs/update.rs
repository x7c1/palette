use crate::api_types::{
    ErrorCode, InputError, JobResponse, Location, ResourceKind, UpdateJobRequest,
};
use crate::{AppState, Error, ValidJson};
use axum::{Json, extract::State};
use palette_core::ReasonKey;
use palette_domain::job::{CraftStatus, JobStatus};
use palette_domain::server::ServerEvent;
use std::sync::Arc;

pub async fn handle_update_job(
    State(state): State<Arc<AppState>>,
    ValidJson(api_req): ValidJson<UpdateJobRequest>,
) -> crate::Result<Json<JobResponse>> {
    let job_id = api_req.validate_id().map_err(|errors| Error::BadRequest {
        code: ErrorCode::InputValidationFailed,
        errors,
    })?;
    let current = state
        .interactor
        .data_store
        .get_job(&job_id)
        .map_err(Error::internal)?
        .ok_or_else(|| Error::NotFound {
            resource: ResourceKind::Job,
            id: job_id.to_string(),
        })?;

    // Convert API status to domain status using the job's type
    let new_status = api_req.status.to_domain(current.job_type);

    palette_domain::rule::validate_transition(current.status, new_status).map_err(|e| {
        Error::BadRequest {
            code: ErrorCode::InvalidStateTransition,
            errors: vec![InputError {
                location: Location::Body,
                hint: "status".into(),
                reason: e.reason_key(),
            }],
        }
    })?;

    let job = state
        .interactor
        .data_store
        .update_job_status(&job_id, new_status)
        .map_err(Error::internal)?;

    // Send domain event based on the new status
    match new_status {
        JobStatus::Craft(CraftStatus::Done) => {
            let _ = state.event_tx.send(ServerEvent::CraftDone {
                job_id: job_id.clone(),
            });
        }
        JobStatus::Craft(CraftStatus::InReview) => {
            let _ = state.event_tx.send(ServerEvent::CraftReadyForReview {
                craft_job_id: job_id,
            });
        }
        _ => {}
    }

    Ok(Json(JobResponse::from(job)))
}
