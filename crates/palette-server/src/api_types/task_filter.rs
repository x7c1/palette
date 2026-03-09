use super::TaskStatus;
use super::TaskType;
use palette_domain as domain;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TaskFilter {
    #[serde(rename = "type")]
    pub task_type: Option<TaskType>,
    pub status: Option<TaskStatus>,
    pub assignee: Option<String>,
}

impl From<TaskFilter> for domain::TaskFilter {
    fn from(api: TaskFilter) -> Self {
        Self {
            task_type: api.task_type.map(domain::TaskType::from),
            status: api.status.map(domain::TaskStatus::from),
            assignee: api.assignee.map(domain::AgentId::new),
        }
    }
}
