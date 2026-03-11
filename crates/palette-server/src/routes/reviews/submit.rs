use crate::AppState;
use crate::api_types::{ReviewSubmissionResponse, SubmitReviewRequest};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use palette_domain::job::{JobId, JobType};
use palette_domain::review::Verdict;
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

    // If changes_requested, enqueue feedback to the assignee member
    if submission.verdict == Verdict::ChangesRequested {
        let craft_jobs = state
            .db
            .find_crafts_for_review(&review_job_id)
            .unwrap_or_default();
        for craft in &craft_jobs {
            if let Some(ref assignee) = craft.assignee {
                let feedback = format!(
                    "[review-feedback] job={} verdict=changes_requested summary: {}",
                    craft.id,
                    submission.summary.as_deref().unwrap_or("(no summary)")
                );
                let _ = state.db.enqueue_message(assignee, &feedback);
                tracing::info!(
                    job_id = %craft.id,
                    assignee = %assignee,
                    "enqueued review feedback to member"
                );
            }
        }
    }

    // Notify leader about review results
    {
        let infra = state.infra.lock().await;
        if let Some(leader) = infra.find_leader() {
            let verdict_str = match submission.verdict {
                Verdict::Approved => "approved",
                Verdict::ChangesRequested => "changes_requested",
            };
            let craft_ids: Vec<String> = state
                .db
                .find_crafts_for_review(&review_job_id)
                .unwrap_or_default()
                .iter()
                .map(|w| w.id.to_string())
                .collect();
            let notification = format!(
                "[event] review={review_job_id} crafts={} type={verdict_str}",
                craft_ids.join(","),
            );
            let _ = state.db.enqueue_message(&leader.id, &notification);
            tracing::info!(
                review_job_id = %review_job_id,
                verdict = verdict_str,
                "notified leader of review result"
            );
        }
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
