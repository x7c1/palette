use super::TaskId;
use crate::job::{JobType, Priority, Repository};
use std::collections::HashMap;

/// Static structure of a task hierarchy, extracted from a Blueprint.
/// Contains no execution state (status) — only structural information.
#[derive(Clone)]
pub struct TaskTree {
    root_id: TaskId,
    nodes: HashMap<TaskId, TaskTreeNode>,
}

/// A single node in the task tree.
#[derive(Clone)]
pub struct TaskTreeNode {
    pub id: TaskId,
    pub parent_id: Option<TaskId>,
    pub title: String,
    pub plan_path: Option<String>,
    pub job_type: Option<JobType>,
    pub description: Option<String>,
    pub priority: Option<Priority>,
    pub repository: Option<Repository>,
    pub children: Vec<TaskId>,
    pub depends_on: Vec<TaskId>,
}

impl TaskTree {
    pub fn new(root_id: TaskId, nodes: HashMap<TaskId, TaskTreeNode>) -> Self {
        Self { root_id, nodes }
    }

    pub fn root_id(&self) -> &TaskId {
        &self.root_id
    }

    pub fn get(&self, id: &TaskId) -> Option<&TaskTreeNode> {
        self.nodes.get(id)
    }

    /// Return all task IDs in this tree.
    pub fn task_ids(&self) -> impl Iterator<Item = &TaskId> {
        self.nodes.keys()
    }
}
