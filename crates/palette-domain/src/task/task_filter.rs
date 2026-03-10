use super::{TaskStatus, TaskType};
use crate::agent::AgentId;

#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    pub task_type: Option<TaskType>,
    pub status: Option<TaskStatus>,
    pub assignee: Option<AgentId>,
}
