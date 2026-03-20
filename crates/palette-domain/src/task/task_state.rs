use super::{TaskId, TaskStatus};
use crate::workflow::WorkflowId;

/// Execution state of a task, as stored in the database.
/// Contains no structural information — only runtime state.
pub struct TaskState {
    pub id: TaskId,
    pub workflow_id: WorkflowId,
    pub status: TaskStatus,
}
