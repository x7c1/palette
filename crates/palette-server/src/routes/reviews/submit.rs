use crate::AppState;
use crate::api_types::{ReviewSubmissionResponse, SubmitReviewRequest};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use palette_domain::review::Verdict;
use palette_domain::server::ServerEvent;
use palette_domain::task::{TaskId, TaskType};
use std::sync::Arc;

pub async fn handle_submit_review(
    State(state): State<Arc<AppState>>,
    Path(review_task_id): Path<String>,
    Json(api_req): Json<SubmitReviewRequest>,
) -> Result<(StatusCode, Json<ReviewSubmissionResponse>), (StatusCode, String)> {
    let review_task_id = TaskId::new(review_task_id);
    let req: palette_domain::review::SubmitReviewRequest = api_req.into();

    // Verify the task exists and is a review
    let task = state
        .db
        .get_task(&review_task_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("review task not found: {review_task_id}"),
            )
        })?;

    if task.task_type != TaskType::Review {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("task {review_task_id} is not a review task"),
        ));
    }

    let submission = state
        .db
        .submit_review(&review_task_id, &req)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Apply rule engine
    let effects = state
        .rules
        .on_review_submitted(state.db.as_ref(), &review_task_id, &submission)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for effect in &effects {
        tracing::info!(?effect, "review rule engine effect");
    }

    // If changes_requested, enqueue feedback to the assignee member
    if submission.verdict == Verdict::ChangesRequested {
        let work_tasks = state
            .db
            .find_works_for_review(&review_task_id)
            .unwrap_or_default();
        for work in &work_tasks {
            if let Some(ref assignee) = work.assignee {
                let feedback = format!(
                    "[review-feedback] task={} verdict=changes_requested summary: {}",
                    work.id,
                    submission.summary.as_deref().unwrap_or("(no summary)")
                );
                let _ = state.db.enqueue_message(assignee, &feedback);
                tracing::info!(
                    task_id = %work.id,
                    assignee = %assignee,
                    "enqueued review feedback to member"
                );
            }
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
