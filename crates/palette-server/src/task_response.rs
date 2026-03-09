use crate::priority_api::PriorityApi;
use crate::repository_api::RepositoryApi;
use crate::task_status_api::TaskStatusApi;
use crate::task_type_api::TaskTypeApi;
use chrono::{DateTime, Utc};
use palette_domain::Task;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TaskResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub task_type: TaskTypeApi,
    pub title: String,
    pub description: Option<String>,
    pub assignee: Option<String>,
    pub status: TaskStatusApi,
    pub priority: Option<PriorityApi>,
    pub repositories: Option<Vec<RepositoryApi>>,
    pub pr_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub notes: Option<String>,
    pub assigned_at: Option<DateTime<Utc>>,
}

impl From<Task> for TaskResponse {
    fn from(t: Task) -> Self {
        Self {
            id: t.id.to_string(),
            task_type: t.task_type.into(),
            title: t.title,
            description: t.description,
            assignee: t.assignee.map(|a| a.to_string()),
            status: t.status.into(),
            priority: t.priority.map(PriorityApi::from),
            repositories: t.repositories.map(|repos| {
                repos
                    .into_iter()
                    .map(|r| RepositoryApi {
                        name: r.name,
                        branch: r.branch,
                    })
                    .collect()
            }),
            pr_url: t.pr_url,
            created_at: t.created_at,
            updated_at: t.updated_at,
            notes: t.notes,
            assigned_at: t.assigned_at,
        }
    }
}
