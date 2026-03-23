use std::fmt;

/// A task key used in Blueprints to identify tasks within a tree.
/// Keys are local identifiers (e.g., "step-a", "review") used for
/// depends_on references and human-readable labels.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskKey(String);

impl TaskKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }
}

impl fmt::Display for TaskKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl AsRef<str> for TaskKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<&str> for TaskKey {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}
