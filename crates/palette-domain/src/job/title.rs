use std::fmt;

const MAX_LEN: usize = 500;

/// Job title.
#[derive(Debug, Clone)]
pub struct Title(String);

impl Title {
    pub fn parse(s: impl Into<String>) -> Result<Self, InvalidTitle> {
        let s = s.into();
        if s.trim().is_empty() {
            return Err(InvalidTitle::Empty);
        }
        if s.len() > MAX_LEN {
            return Err(InvalidTitle::TooLong { len: s.len() });
        }
        Ok(Self(s))
    }
}

impl AsRef<str> for Title {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Title {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl From<Title> for String {
    fn from(t: Title) -> Self {
        t.0
    }
}

#[derive(Debug, palette_macros::ReasonKey)]
#[reason_namespace = "title"]
pub enum InvalidTitle {
    Empty,
    TooLong { len: usize },
}
