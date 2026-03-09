use chrono::{DateTime, Utc};

use crate::task_id::TaskId;
use crate::verdict::Verdict;

#[derive(Debug, Clone)]
pub struct ReviewSubmission {
    pub id: i64,
    pub review_task_id: TaskId,
    pub round: i32,
    pub verdict: Verdict,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
}
