use chrono::{DateTime, Utc};

use crate::agent_id::AgentId;
use crate::priority::Priority;
use crate::repository::Repository;
use crate::task_id::TaskId;
use crate::task_status::TaskStatus;
use crate::task_type::TaskType;

#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub task_type: TaskType,
    pub title: String,
    pub description: Option<String>,
    pub assignee: Option<AgentId>,
    pub status: TaskStatus,
    pub priority: Option<Priority>,
    pub repositories: Option<Vec<Repository>>,
    pub pr_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub notes: Option<String>,
    pub assigned_at: Option<DateTime<Utc>>,
}
