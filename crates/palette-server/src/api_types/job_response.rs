use super::JobStatus;
use super::JobType;
use super::Priority;
use super::Repository;
use chrono::{DateTime, Utc};
use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct JobResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub job_type: JobType,
    pub title: String,
    pub plan_path: Option<String>,
    pub assignee_id: Option<String>,
    pub status: JobStatus,
    pub priority: Option<Priority>,
    pub repository: Option<Repository>,
    pub command: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub notes: Option<String>,
    pub assigned_at: Option<DateTime<Utc>>,
}

impl From<domain::job::Job> for JobResponse {
    fn from(t: domain::job::Job) -> Self {
        Self {
            id: t.id.to_string(),
            job_type: t.detail.job_type().into(),
            title: t.title.into(),
            plan_path: t.plan_path.map(Into::into),
            assignee_id: t.assignee_id.map(|a| a.to_string()),
            status: t.status.into(),
            priority: t.priority.map(Priority::from),
            repository: t.detail.repository().cloned().map(Into::into),
            command: t.detail.command().map(String::from),
            created_at: t.created_at,
            updated_at: t.updated_at,
            notes: t.notes,
            assigned_at: t.assigned_at,
        }
    }
}
