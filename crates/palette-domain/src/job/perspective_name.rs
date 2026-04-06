use std::fmt;

/// Perspective name — identifies a set of review criteria documents.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PerspectiveName(String);

impl PerspectiveName {
    pub fn parse(s: impl Into<String>) -> Result<Self, InvalidPerspectiveName> {
        let s = s.into();
        if s.trim().is_empty() {
            return Err(InvalidPerspectiveName::Empty);
        }
        Ok(Self(s))
    }
}

impl AsRef<str> for PerspectiveName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PerspectiveName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl From<PerspectiveName> for String {
    fn from(p: PerspectiveName) -> Self {
        p.0
    }
}

#[derive(Debug, palette_macros::ReasonKey)]
pub enum InvalidPerspectiveName {
    Empty,
}
