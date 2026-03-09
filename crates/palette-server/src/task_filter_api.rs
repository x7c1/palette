use crate::task_status_api::TaskStatusApi;
use crate::task_type_api::TaskTypeApi;
use palette_domain::{AgentId, TaskFilter, TaskStatus, TaskType};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TaskFilterApi {
    #[serde(rename = "type")]
    pub task_type: Option<TaskTypeApi>,
    pub status: Option<TaskStatusApi>,
    pub assignee: Option<String>,
}

impl From<TaskFilterApi> for TaskFilter {
    fn from(api: TaskFilterApi) -> Self {
        Self {
            task_type: api.task_type.map(TaskType::from),
            status: api.status.map(TaskStatus::from),
            assignee: api.assignee.map(AgentId::new),
        }
    }
}
