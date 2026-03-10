use std::fmt;

/// Identifier for an agent's working session (e.g., a Claude Code session).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentSessionId(String);

impl AgentSessionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for AgentSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl AsRef<str> for AgentSessionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
