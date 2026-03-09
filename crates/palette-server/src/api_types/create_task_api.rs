use super::PriorityApi;
use super::RepositoryApi;
use super::TaskTypeApi;
use palette_domain::{AgentId, CreateTaskRequest, Priority, Repository, TaskId};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateTaskApi {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub task_type: TaskTypeApi,
    pub title: String,
    pub description: Option<String>,
    pub assignee: Option<String>,
    pub priority: Option<PriorityApi>,
    pub repositories: Option<Vec<RepositoryApi>>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

impl From<CreateTaskApi> for CreateTaskRequest {
    fn from(api: CreateTaskApi) -> Self {
        Self {
            id: api.id.map(TaskId::new),
            task_type: api.task_type.into(),
            title: api.title,
            description: api.description,
            assignee: api.assignee.map(AgentId::new),
            priority: api.priority.map(Priority::from),
            repositories: api.repositories.map(|repos| {
                repos
                    .into_iter()
                    .map(|r| Repository {
                        name: r.name,
                        branch: r.branch,
                    })
                    .collect()
            }),
            depends_on: api.depends_on.into_iter().map(TaskId::new).collect(),
        }
    }
}
