use std::fmt;

/// A task key used in Blueprints to identify tasks within a tree.
/// Keys are local identifiers (e.g., "step-a", "review") used for
/// depends_on references and human-readable labels.
/// Must match `[a-z0-9-]+`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskKey(String);

impl TaskKey {
    pub fn parse(key: impl Into<String>) -> Result<Self, InvalidTaskKey> {
        let key = key.into();
        if key.is_empty() {
            return Err(InvalidTaskKey::Empty);
        }
        if !key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(InvalidTaskKey::InvalidFormat { key });
        }
        Ok(Self(key))
    }
}

#[derive(Debug, palette_macros::ReasonKey)]
pub enum InvalidTaskKey {
    Empty,
    InvalidFormat { key: String },
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
