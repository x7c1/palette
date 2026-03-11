use crate::AppState;
use crate::api_types::ReviewSubmissionResponse;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use palette_domain::job::JobId;
use std::sync::Arc;

pub async fn handle_get_submissions(
    State(state): State<Arc<AppState>>,
    Path(review_job_id): Path<String>,
) -> Result<Json<Vec<ReviewSubmissionResponse>>, (StatusCode, String)> {
    let review_job_id = JobId::new(review_job_id);
    let submissions = state
        .db
        .get_review_submissions(&review_job_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(
        submissions
            .into_iter()
            .map(ReviewSubmissionResponse::from)
            .collect(),
    ))
}
