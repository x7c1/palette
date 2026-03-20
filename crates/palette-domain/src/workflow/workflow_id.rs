use std::fmt;

/// Workflow identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkflowId(String);

impl WorkflowId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
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
