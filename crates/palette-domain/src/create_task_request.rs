use crate::agent_id::AgentId;
use crate::priority::Priority;
use crate::repository::Repository;
use crate::task_id::TaskId;
use crate::task_type::TaskType;

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
