use palette_domain::task::{TaskId, TaskStatus};
use palette_domain::workflow::WorkflowId;

/// Flat representation of a Task as stored in the database.
/// Contains only execution state — structural information comes from the Blueprint.
#[derive(Debug, Clone)]
pub struct TaskRow {
    pub id: TaskId,
    pub workflow_id: WorkflowId,
    pub status: TaskStatus,
}
