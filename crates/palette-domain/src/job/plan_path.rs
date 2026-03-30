use std::fmt;

const MAX_LEN: usize = 1024;

/// Path to a plan document.
#[derive(Debug, Clone)]
pub struct PlanPath(String);

impl PlanPath {
    pub fn parse(s: impl Into<String>) -> Result<Self, InvalidPlanPath> {
        let s = s.into();
        if s.trim().is_empty() {
            return Err(InvalidPlanPath::Empty);
        }
        if s.len() > MAX_LEN {
            return Err(InvalidPlanPath::TooLong { len: s.len() });
        }
        Ok(Self(s))
    }
}

impl AsRef<str> for PlanPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PlanPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl From<PlanPath> for String {
    fn from(p: PlanPath) -> Self {
        p.0
    }
}

#[derive(Debug, palette_macros::ReasonKey)]
#[reason_namespace = "plan_path"]
pub enum InvalidPlanPath {
    Empty,
    TooLong { len: usize },
}
