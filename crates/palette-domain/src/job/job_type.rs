use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobType {
    Craft,
    Review,
}

impl JobType {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobType::Craft => "craft",
            JobType::Review => "review",
        }
    }
}

impl fmt::Display for JobType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}
