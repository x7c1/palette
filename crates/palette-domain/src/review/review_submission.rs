use chrono::{DateTime, Utc};

use super::Verdict;
use crate::task::TaskId;

#[derive(Debug, Clone)]
pub struct ReviewSubmission {
    pub id: i64,
    pub review_task_id: TaskId,
    pub round: i32,
    pub verdict: Verdict,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
}
