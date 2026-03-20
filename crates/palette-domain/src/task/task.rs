use super::{TaskId, TaskStatus};
use crate::job::JobType;
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
    pub job_type: Option<JobType>,
    pub status: TaskStatus,
    pub children: Vec<Task>,
}

impl Task {
    /// A Composite Task is a Task that has child Tasks.
    pub fn is_composite(&self) -> bool {
        !self.children.is_empty()
    }
}
