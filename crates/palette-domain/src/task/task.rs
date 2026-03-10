use chrono::{DateTime, Utc};

use super::{Priority, Repository, TaskId, TaskStatus, TaskType};
use crate::agent::AgentId;

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
