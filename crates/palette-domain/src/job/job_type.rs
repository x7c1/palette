use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobType {
    Craft,
    Review,
    /// A composite review task that integrates child review results.
    ReviewIntegrate,
    /// Orchestrator executes a command on the host (no container spawned).
    Orchestrator,
    /// Operator (human) decision point (no container spawned).
    Operator,
}

impl JobType {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobType::Craft => "craft",
            JobType::Review => "review",
            JobType::ReviewIntegrate => "review_integrate",
            JobType::Orchestrator => "orchestrator",
            JobType::Operator => "operator",
        }
    }

    /// Whether this job type requires a worker container.
    pub fn needs_worker(&self) -> bool {
        matches!(self, JobType::Craft | JobType::Review)
    }
}

impl fmt::Display for JobType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}
