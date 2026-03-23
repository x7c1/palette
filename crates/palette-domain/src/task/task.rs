use super::{TaskId, TaskStatus};
use crate::job::{JobType, Priority, Repository};
use crate::workflow::WorkflowId;

/// A Task is a goal to achieve. Tasks form a tree structure where a Composite
/// Task has child Tasks. A Task can also have a Job assigned to it.
///
/// Constructed by combining structural information (from Blueprint / TaskTree)
/// with execution state (from DB / TaskState).
#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub workflow_id: WorkflowId,
    pub parent_id: Option<TaskId>,
    pub key: String,
    pub plan_path: Option<String>,
    pub job_type: Option<JobType>,
    pub description: Option<String>,
    pub priority: Option<Priority>,
    pub repository: Option<Repository>,
    pub status: TaskStatus,
    pub children: Vec<Task>,
    pub depends_on: Vec<TaskId>,
}

impl Task {
    /// A Composite Task is a Task that has child Tasks.
    pub fn is_composite(&self) -> bool {
        !self.children.is_empty()
    }
}
