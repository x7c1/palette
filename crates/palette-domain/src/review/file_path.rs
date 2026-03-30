use std::fmt;

const MAX_LEN: usize = 1024;

/// File path in a review comment.
#[derive(Debug, Clone)]
pub struct FilePath(String);

impl FilePath {
    pub fn parse(s: impl Into<String>) -> Result<Self, InvalidFilePath> {
        let s = s.into();
        if s.trim().is_empty() {
            return Err(InvalidFilePath::Empty);
        }
        if s.len() > MAX_LEN {
            return Err(InvalidFilePath::TooLong { len: s.len() });
        }
        Ok(Self(s))
    }
}

impl AsRef<str> for FilePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl From<FilePath> for String {
    fn from(p: FilePath) -> Self {
        p.0
    }
}

#[derive(Debug, palette_macros::ReasonKey)]
#[reason_namespace = "file_path"]
pub enum InvalidFilePath {
    Empty,
    TooLong { len: usize },
}
