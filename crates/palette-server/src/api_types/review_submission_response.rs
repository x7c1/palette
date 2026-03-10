use super::Verdict;
use chrono::{DateTime, Utc};
use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewSubmissionResponse {
    pub id: i64,
    pub review_task_id: String,
    pub round: i32,
    pub verdict: Verdict,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<domain::ReviewSubmission> for ReviewSubmissionResponse {
    fn from(s: domain::ReviewSubmission) -> Self {
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
