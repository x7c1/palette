use super::Priority;
use super::Repository;
use super::TaskType;
use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub title: String,
    pub description: Option<String>,
    pub assignee: Option<String>,
    pub priority: Option<Priority>,
    pub repositories: Option<Vec<Repository>>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

// TODO: Replace From with TryFrom to validate external input (see plan 009-api-input-validation)
impl From<CreateTaskRequest> for domain::CreateTaskRequest {
    fn from(api: CreateTaskRequest) -> Self {
        Self {
            id: api.id.map(domain::TaskId::new),
            task_type: api.task_type.into(),
            title: api.title,
            description: api.description,
            assignee: api.assignee.map(domain::AgentId::new),
            priority: api.priority.map(domain::Priority::from),
            repositories: api
                .repositories
                .map(|repos| repos.into_iter().map(Into::into).collect()),
            depends_on: api
                .depends_on
                .into_iter()
                .map(domain::TaskId::new)
                .collect(),
        }
    }
}
