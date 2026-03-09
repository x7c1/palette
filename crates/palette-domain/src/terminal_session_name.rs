use std::fmt;

/// Name of a terminal session (e.g., a tmux session name).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalSessionName(String);

impl TerminalSessionName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

impl fmt::Display for TerminalSessionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl AsRef<str> for TerminalSessionName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
