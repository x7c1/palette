use crate::api_types::{
    ErrorCode, InputError, JobResponse, Location, ResourceKind, UpdateJobRequest,
};
use crate::{AppState, Error, ValidJson};
use axum::{Json, extract::State};
use palette_core::ReasonKey;
use palette_domain::job::{CraftStatus, JobStatus};
use palette_domain::rule::RuleEffect;
use palette_domain::server::ServerEvent;
use palette_usecase::RuleEngine;
use std::sync::Arc;

pub async fn handle_update_job(
    State(state): State<Arc<AppState>>,
    ValidJson(api_req): ValidJson<UpdateJobRequest>,
) -> crate::Result<Json<JobResponse>> {
    let job_id = api_req.validate_id().map_err(|errors| Error::BadRequest {
        code: ErrorCode::InputValidationFailed,
        errors,
        message: None,
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
            message: None,
        }
    })?;

    let job = state
        .interactor
        .data_store
        .update_job_status(&job_id, new_status)
        .map_err(Error::internal)?;

    // Produce effects based on the new status
    let effects = match new_status {
        JobStatus::Craft(CraftStatus::Done) => RuleEngine::new(
            state.interactor.data_store.as_ref(),
            state.max_review_rounds,
        )
        .on_craft_done(&job_id)
        .map_err(Error::internal)?,
        JobStatus::Craft(CraftStatus::InReview) => {
            vec![RuleEffect::CraftReadyForReview {
                craft_job_id: job_id,
            }]
        }
        _ => vec![],
    };

    for effect in &effects {
        tracing::info!(?effect, "rule engine effect");
    }

    if !effects.is_empty() {
        let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    }

    Ok(Json(JobResponse::from(job)))
}
