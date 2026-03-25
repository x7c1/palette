use std::fmt;

/// Terminal target where a worker runs (e.g., a tmux pane or window).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalTarget(String);

impl TerminalTarget {
    pub fn new(target: impl Into<String>) -> Self {
        Self(target.into())
    }
}

impl fmt::Display for TerminalTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl AsRef<str> for TerminalTarget {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
