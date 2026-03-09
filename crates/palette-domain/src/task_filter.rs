use crate::agent_id::AgentId;
use crate::task_status::TaskStatus;
use crate::task_type::TaskType;

#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    pub task_type: Option<TaskType>,
    pub status: Option<TaskStatus>,
    pub assignee: Option<AgentId>,
}
