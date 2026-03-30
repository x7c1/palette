use crate::api_types::ReviewSubmissionResponse;
use crate::{AppState, Error};
use axum::{
    Json,
    extract::{Path, State},
};
use palette_domain::job::JobId;
use std::sync::Arc;

pub async fn handle_get_submissions(
    State(state): State<Arc<AppState>>,
    Path(review_job_id): Path<String>,
) -> crate::Result<Json<Vec<ReviewSubmissionResponse>>> {
    let review_job_id = JobId::new(review_job_id);
    let submissions = state
        .interactor
        .data_store
        .get_review_submissions(&review_job_id)
        .map_err(Error::internal)?;
    Ok(Json(
        submissions
            .into_iter()
            .map(ReviewSubmissionResponse::from)
            .collect(),
    ))
}
