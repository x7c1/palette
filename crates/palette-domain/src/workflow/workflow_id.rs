use std::fmt;

const MAX_ID_LEN: usize = 256;

/// Workflow identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkflowId(String);

impl WorkflowId {
    /// Parse and validate a WorkflowId from external input.
    ///
    /// Rejects `:` and `/` (which would collide with TaskId format)
    /// and enforces a maximum length.
    pub fn parse(id: impl Into<String>) -> Result<Self, InvalidWorkflowId> {
        let id = id.into();
        if id.is_empty() || id.len() > MAX_ID_LEN || id.contains(':') || id.contains('/') {
            return Err(InvalidWorkflowId { id });
        }
        Ok(Self(id))
    }

    pub fn generate() -> Self {
        let suffix = &uuid::Uuid::new_v4().as_simple().to_string()[..8];
        Self(format!("wf-{suffix}"))
    }
}

impl fmt::Display for WorkflowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl AsRef<str> for WorkflowId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Invalid format for a workflow ID.
#[derive(Debug)]
pub struct InvalidWorkflowId {
    pub id: String,
}

impl InvalidWorkflowId {
    pub fn reason_key(&self) -> &str {
        "invalid_format"
    }
}

impl fmt::Display for InvalidWorkflowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid workflow ID: {}", self.id)
    }
}
