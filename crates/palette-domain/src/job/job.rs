use chrono::{DateTime, Utc};

use super::{JobId, JobStatus, JobType, Priority, Repository};
use crate::agent::AgentId;

#[derive(Debug, Clone)]
pub struct Job {
    pub id: JobId,
    pub job_type: JobType,
    pub title: String,
    pub description: Option<String>,
    pub assignee: Option<AgentId>,
    pub status: JobStatus,
    pub priority: Option<Priority>,
    pub repositories: Option<Vec<Repository>>,
    pub pr_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub notes: Option<String>,
    pub assigned_at: Option<DateTime<Utc>>,
}
