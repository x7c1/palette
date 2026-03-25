use std::fmt;

/// Identifier for a worker's working session (e.g., a Claude Code session).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkerSessionId(String);

impl WorkerSessionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for WorkerSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl AsRef<str> for WorkerSessionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
