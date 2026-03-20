use palette_domain::task::{TaskId, TaskStatus};
use palette_domain::workflow::WorkflowId;

/// Flat representation of a Task as stored in the database.
#[derive(Debug, Clone)]
pub struct TaskRow {
    pub id: TaskId,
    pub workflow_id: WorkflowId,
    pub parent_id: Option<TaskId>,
    pub title: String,
    pub plan_path: Option<String>,
    pub status: TaskStatus,
}
