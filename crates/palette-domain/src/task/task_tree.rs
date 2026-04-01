use super::{TaskId, TaskKey};
use crate::job::{JobType, Priority, Repository};
use std::collections::HashMap;

/// Static structure of a task hierarchy, extracted from a Blueprint.
/// Contains no execution state (status) — only structural information.
#[derive(Debug, Clone)]
pub struct TaskTree {
    root_id: TaskId,
    nodes: HashMap<TaskId, TaskTreeNode>,
}

/// A single node in the task tree.
#[derive(Debug, Clone)]
pub struct TaskTreeNode {
    pub id: TaskId,
    pub parent_id: Option<TaskId>,
    pub key: TaskKey,
    pub plan_path: Option<String>,
    pub job_type: Option<JobType>,
    pub priority: Option<Priority>,
    pub repository: Option<Repository>,
    /// Command for orchestrator tasks (e.g., "docker compose run --rm check").
    pub command: Option<String>,
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

    /// Find sibling nodes that share the same parent as the given task.
    /// Returns an empty vec if the task has no parent (root) or is not found.
    pub fn siblings(&self, id: &TaskId) -> Vec<&TaskTreeNode> {
        let Some(node) = self.nodes.get(id) else {
            return vec![];
        };
        let Some(ref parent_id) = node.parent_id else {
            return vec![];
        };
        let Some(parent) = self.nodes.get(parent_id) else {
            return vec![];
        };
        parent
            .children
            .iter()
            .filter(|child_id| *child_id != id)
            .filter_map(|child_id| self.nodes.get(child_id))
            .collect()
    }

    /// Find the sibling craft task for a given task (typically a review task).
    /// Returns None if no sibling with job_type Craft exists.
    pub fn sibling_craft(&self, id: &TaskId) -> Option<&TaskTreeNode> {
        self.siblings(id)
            .into_iter()
            .find(|s| s.job_type == Some(JobType::Craft))
    }

    /// Find a node by its key.
    pub fn find_by_key(&self, key: &str) -> Option<&TaskTreeNode> {
        self.nodes.values().find(|n| n.key.as_ref() == key)
    }
}
