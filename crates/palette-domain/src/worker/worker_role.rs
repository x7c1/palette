use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerRole {
    Leader,
    ReviewIntegrator,
    Member,
}

impl WorkerRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkerRole::Leader => "leader",
            WorkerRole::ReviewIntegrator => "review_integrator",
            WorkerRole::Member => "member",
        }
    }

    /// Returns true if this role acts as a supervisor (can receive member events).
    pub fn is_supervisor(&self) -> bool {
        matches!(self, WorkerRole::Leader | WorkerRole::ReviewIntegrator)
    }
}

impl fmt::Display for WorkerRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}
