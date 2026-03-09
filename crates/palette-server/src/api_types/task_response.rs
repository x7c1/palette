use super::Priority;
use super::Repository;
use super::TaskStatus;
use super::TaskType;
use chrono::{DateTime, Utc};
use palette_domain as domain;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TaskResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub title: String,
    pub description: Option<String>,
    pub assignee: Option<String>,
    pub status: TaskStatus,
    pub priority: Option<Priority>,
    pub repositories: Option<Vec<Repository>>,
    pub pr_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub notes: Option<String>,
    pub assigned_at: Option<DateTime<Utc>>,
}

impl From<domain::Task> for TaskResponse {
    fn from(t: domain::Task) -> Self {
        Self {
            id: t.id.to_string(),
            task_type: t.task_type.into(),
            title: t.title,
            description: t.description,
            assignee: t.assignee.map(|a| a.to_string()),
            status: t.status.into(),
            priority: t.priority.map(Priority::from),
            repositories: t
                .repositories
                .map(|repos| repos.into_iter().map(Into::into).collect()),
            pr_url: t.pr_url,
            created_at: t.created_at,
            updated_at: t.updated_at,
            notes: t.notes,
            assigned_at: t.assigned_at,
        }
    }
}
