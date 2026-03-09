use crate::verdict_api::VerdictApi;
use chrono::{DateTime, Utc};
use palette_domain::ReviewSubmission;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ReviewSubmissionResponse {
    pub id: i64,
    pub review_task_id: String,
    pub round: i32,
    pub verdict: VerdictApi,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<ReviewSubmission> for ReviewSubmissionResponse {
    fn from(s: ReviewSubmission) -> Self {
        Self {
            id: s.id,
            review_task_id: s.review_task_id.to_string(),
            round: s.round,
            verdict: s.verdict.into(),
            summary: s.summary,
            created_at: s.created_at,
        }
    }
}
