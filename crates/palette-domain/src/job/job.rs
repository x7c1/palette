use chrono::{DateTime, Utc};

use super::{JobDetail, JobId, JobStatus, PlanPath, Priority, Title};
use crate::task::TaskId;
use crate::worker::WorkerId;

#[derive(Debug, Clone)]
pub struct Job {
    pub id: JobId,
    pub task_id: TaskId,
    pub title: Title,
    pub plan_path: PlanPath,
    pub assignee_id: Option<WorkerId>,
    pub status: JobStatus,
    pub priority: Option<Priority>,
    pub detail: JobDetail,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub notes: Option<String>,
    pub assigned_at: Option<DateTime<Utc>>,
}
