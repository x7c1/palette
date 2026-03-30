use super::TaskKey;
use crate::workflow::WorkflowId;
use std::fmt;

const MAX_ID_LEN: usize = 512;

/// Task identifier in the format `{workflow_id}:{key_path}`.
///
/// The key_path is a `/`-separated path of task keys from root to the node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId(String);

impl TaskId {
    /// Parse and validate a TaskId from external input.
    ///
    /// Requires the `{workflow_id}:{key_path}` format (must contain `:`).
    pub fn parse(id: impl Into<String>) -> Result<Self, InvalidTaskId> {
        let id = id.into();
        if id.is_empty() || id.len() > MAX_ID_LEN || !id.contains(':') {
            return Err(InvalidTaskId { id });
        }
        Ok(Self(id))
    }

    /// Create a root task ID from a workflow ID and root key.
    pub fn root(workflow_id: &WorkflowId, key: &TaskKey) -> Self {
        Self(format!("{}:{}", workflow_id, key))
    }

    /// Create a child task ID by appending a key to this task ID.
    pub fn child(&self, key: &TaskKey) -> Self {
        Self(format!("{}/{}", self.0, key))
    }

    /// Return the parent task ID by stripping the last path component.
    ///
    /// Returns `None` if this is a root task ID (no `/` separator).
    pub fn parent(&self) -> Option<Self> {
        self.0.rfind('/').map(|pos| Self(self.0[..pos].to_string()))
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

/// Invalid format for a task ID.
#[derive(Debug)]
pub struct InvalidTaskId {
    pub id: String,
}

impl InvalidTaskId {
    pub fn reason_key(&self) -> &str {
        "invalid_format"
    }
}

impl fmt::Display for InvalidTaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid task ID: {}", self.id)
    }
}
