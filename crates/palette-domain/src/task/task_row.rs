use super::{TaskId, TaskStatus};
use crate::workflow::WorkflowId;

/// Flat representation of a Task as stored in the database.
/// Use `TaskStore::get_task()` to get a `Task` with children populated.
#[derive(Debug, Clone)]
pub struct TaskRow {
    pub id: TaskId,
    pub workflow_id: WorkflowId,
    pub parent_id: Option<TaskId>,
    pub title: String,
    pub plan_path: Option<String>,
    pub status: TaskStatus,
}
