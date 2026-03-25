use crate::AppState;
use crate::api_types::{ReviewSubmissionResponse, SubmitReviewRequest};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use palette_domain::job::{JobId, JobType};
use palette_domain::server::ServerEvent;
use std::sync::Arc;

pub async fn handle_submit_review(
    State(state): State<Arc<AppState>>,
    Path(review_job_id): Path<String>,
    Json(api_req): Json<SubmitReviewRequest>,
) -> Result<(StatusCode, Json<ReviewSubmissionResponse>), (StatusCode, String)> {
    let review_job_id = JobId::new(review_job_id);
    let req: palette_domain::review::SubmitReviewRequest = api_req.into();

    // Verify the job exists and is a review
    let job = state
        .db
        .get_job(&review_job_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("review job not found: {review_job_id}"),
            )
        })?;

    if job.job_type != JobType::Review {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("job {review_job_id} is not a review job"),
        ));
    }

    let submission = state
        .db
        .submit_review(&review_job_id, &req)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Apply rule engine
    let effects = state
        .rules
        .on_review_submitted(&review_job_id, &submission)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for effect in &effects {
        tracing::info!(?effect, "review rule engine effect");
    }

    // Notify the review member's supervisor about review results
    if let Some(ref assignee) = job.assignee
        && let Ok(Some(member)) = state.db.find_worker(assignee)
        && let Ok(Some(supervisor)) = state.db.find_worker(&member.supervisor_id)
    {
        let verdict_str = match submission.verdict {
            palette_domain::review::Verdict::Approved => "approved",
            palette_domain::review::Verdict::ChangesRequested => "changes_requested",
        };
        let notification = format!("[event] review={review_job_id} type={verdict_str}");
        let _ = state.db.enqueue_message(&supervisor.id, &notification);
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
