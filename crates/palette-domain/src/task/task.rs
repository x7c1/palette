use super::{TaskId, TaskStatus};
use crate::workflow::WorkflowId;

/// A Task is a goal to achieve. Tasks form a tree structure where a Composite
/// Task has child Tasks. A Task can also have a Job assigned to it.
#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub workflow_id: WorkflowId,
    pub parent_id: Option<TaskId>,
    pub title: String,
    pub plan_path: Option<String>,
    pub status: TaskStatus,
}
