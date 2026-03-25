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
    pub plan_path: String,
    pub assignee_id: Option<String>,
    pub status: JobStatus,
    pub priority: Option<Priority>,
    pub repository: Option<Repository>,
    pub pr_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub notes: Option<String>,
    pub assigned_at: Option<DateTime<Utc>>,
}

impl From<domain::job::Job> for JobResponse {
    fn from(t: domain::job::Job) -> Self {
        Self {
            id: t.id.to_string(),
            job_type: t.job_type.into(),
            title: t.title,
            plan_path: t.plan_path,
            assignee_id: t.assignee_id.map(|a| a.to_string()),
            status: t.status.into(),
            priority: t.priority.map(Priority::from),
            repository: t.repository.map(Into::into),
            pr_url: t.pr_url,
            created_at: t.created_at,
            updated_at: t.updated_at,
            notes: t.notes,
            assigned_at: t.assigned_at,
        }
    }
}
