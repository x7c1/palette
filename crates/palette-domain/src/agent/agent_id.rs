use std::fmt;

/// Agent identifier for both leaders and members (e.g., "leader-1", "member-0-a3f2").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a unique member ID with a sequence number and random suffix
    /// (e.g., "member-0-a3f2", "member-1-b7e1").
    /// The random suffix prevents collisions across Palette restarts
    /// and future parallel workflow executions.
    pub fn next_member(sequence: usize) -> Self {
        let random = random_hex(8);
        Self(format!("member-{sequence}-{random}"))
    }

    /// Generate a unique supervisor ID with a sequence number and random suffix.
    /// The prefix is determined by the role (e.g., "leader-0-a3f2", "review-integrator-1-b7e1").
    pub fn next_supervisor(sequence: usize, role: super::AgentRole) -> Self {
        let prefix = match role {
            super::AgentRole::Leader => "leader",
            super::AgentRole::ReviewIntegrator => "review-integrator",
            super::AgentRole::Member => "member",
        };
        let random = random_hex(8);
        Self(format!("{prefix}-{sequence}-{random}"))
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl AsRef<str> for AgentId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

fn random_hex(len: usize) -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64,
    );
    let hash = hasher.finish();
    format!("{hash:016x}")[..len].to_string()
}
