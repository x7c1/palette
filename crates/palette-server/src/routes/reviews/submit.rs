use crate::api_types::{
    ErrorCode, InputError, Location, ResourceKind, ReviewSubmissionResponse, SubmitReviewRequest,
};
use crate::{AppState, Error};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use palette_domain::ReasonKey;
use palette_domain::job::{JobId, JobType};
use palette_domain::server::ServerEvent;
use palette_usecase::RuleEngine;
use std::sync::Arc;

pub async fn handle_submit_review(
    State(state): State<Arc<AppState>>,
    Path(review_job_id): Path<String>,
    Json(api_req): Json<SubmitReviewRequest>,
) -> crate::Result<(StatusCode, Json<ReviewSubmissionResponse>)> {
    let review_job_id = JobId::parse(review_job_id).map_err(|e| Error::BadRequest {
        code: crate::api_types::ErrorCode::InputValidationFailed,
        errors: vec![crate::api_types::InputError {
            location: crate::api_types::Location::Path,
            hint: "review_job_id".into(),
            reason: e.reason_key(),
        }],
    })?;
    let req = api_req.validate().map_err(|errors| Error::BadRequest {
        code: ErrorCode::InputValidationFailed,
        errors,
    })?;

    // Verify the job exists and is a review
    let job = state
        .interactor
        .data_store
        .get_job(&review_job_id)
        .map_err(Error::internal)?
        .ok_or_else(|| Error::NotFound {
            resource: ResourceKind::Job,
            id: review_job_id.to_string(),
        })?;

    if job.job_type != JobType::Review {
        return Err(Error::BadRequest {
            code: ErrorCode::NotReviewJob,
            errors: vec![InputError {
                location: Location::Path,
                hint: "review_job_id".into(),
                reason: "job/not_review_job".into(),
            }],
        });
    }

    let submission = state
        .interactor
        .data_store
        .submit_review(&review_job_id, &req)
        .map_err(Error::internal)?;

    // Apply rule engine
    let effects = RuleEngine::new(
        state.interactor.data_store.as_ref(),
        state.max_review_rounds,
    )
    .on_review_submitted(&review_job_id, &submission)
    .map_err(Error::internal)?;

    for effect in &effects {
        tracing::info!(?effect, "review rule engine effect");
    }

    // Notify the review member's supervisor about review results
    if let Some(ref assignee) = job.assignee_id
        && let Ok(Some(member)) = state.interactor.data_store.find_worker(assignee)
        && let Some(ref supervisor_id) = member.supervisor_id
        && let Ok(Some(supervisor)) = state.interactor.data_store.find_worker(supervisor_id)
    {
        let verdict_str = match submission.verdict {
            palette_domain::review::Verdict::Approved => "approved",
            palette_domain::review::Verdict::ChangesRequested => "changes_requested",
        };
        let notification = format!("[event] review={review_job_id} type={verdict_str}");
        let _ = state
            .interactor
            .data_store
            .enqueue_message(&supervisor.id, &notification);
        tracing::info!(
            review_job_id = %review_job_id,
            verdict = verdict_str,
            supervisor_id = %supervisor.id,
            "notified supervisor of review result"
        );
    }

    // Fire-and-forget: orchestrator processes effects and delivers messages
    if !effects.is_empty() {
        let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    }
    let _ = state.event_tx.send(ServerEvent::NotifyDeliveryLoop);

    Ok((
        StatusCode::CREATED,
        Json(ReviewSubmissionResponse::from(submission)),
    ))
}
