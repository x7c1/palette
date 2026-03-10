use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRole {
    Leader,
    ReviewIntegrator,
    Member,
}

impl AgentRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentRole::Leader => "leader",
            AgentRole::ReviewIntegrator => "review_integrator",
            AgentRole::Member => "member",
        }
    }

    /// Returns true if this role acts as a leader (can receive member events).
    pub fn is_leader(&self) -> bool {
        matches!(self, AgentRole::Leader | AgentRole::ReviewIntegrator)
    }
}

impl fmt::Display for AgentRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}
