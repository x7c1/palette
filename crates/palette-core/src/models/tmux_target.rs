use std::fmt;

/// Tmux pane reference (e.g., "%42" or "session:window").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TmuxTarget(String);

impl TmuxTarget {
    pub fn new(target: impl Into<String>) -> Self {
        Self(target.into())
    }
}

impl fmt::Display for TmuxTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl AsRef<str> for TmuxTarget {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
