use std::fmt;

use super::JobType;

/// Job identifier (e.g., "C-XXXXXXXX" for craft, "R-XXXXXXXX" for review).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JobId(String);

impl JobId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn generate(job_type: JobType) -> Self {
        let prefix = match job_type {
            JobType::Craft => 'C',
            JobType::Review => 'R',
        };
        let suffix = &uuid::Uuid::new_v4().as_simple().to_string()[..8];
        Self(format!("{prefix}-{suffix}"))
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl AsRef<str> for JobId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
