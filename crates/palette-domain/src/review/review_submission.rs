use chrono::{DateTime, Utc};

use super::Verdict;
use crate::job::JobId;

#[derive(Debug, Clone)]
pub struct ReviewSubmission {
    pub id: i64,
    pub review_job_id: JobId,
    pub round: i32,
    pub verdict: Verdict,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
}
