use super::TaskKey;
use crate::workflow::WorkflowId;
use std::fmt;

/// Task identifier in the format `{workflow_id}:{key_path}`.
///
/// The key_path is a `/`-separated path of task keys from root to the node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId(String);

impl TaskId {
    // TODO: validate that id matches the {workflow_id}:{key_path} format
    //       (see plan 011-api-input-validation)
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Create a root task ID from a workflow ID and root key.
    pub fn root(workflow_id: &WorkflowId, key: &TaskKey) -> Self {
        Self(format!("{}:{}", workflow_id, key))
    }

    /// Create a child task ID by appending a key to this task ID.
    pub fn child(&self, key: &TaskKey) -> Self {
        Self(format!("{}/{}", self.0, key))
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl AsRef<str> for TaskId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
