use super::{Priority, Repository, TaskId, TaskType};
use crate::agent::AgentId;

#[derive(Debug, Clone)]
pub struct CreateTaskRequest {
    pub id: Option<TaskId>,
    pub task_type: TaskType,
    pub title: String,
    pub description: Option<String>,
    pub assignee: Option<AgentId>,
    pub priority: Option<Priority>,
    pub repositories: Option<Vec<Repository>>,
    pub depends_on: Vec<TaskId>,
}
