use std::fmt;

/// Agent identifier for both leaders and members (e.g., "leader-1", "member-a").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate the next member ID from a sequence number
    /// (0 → "member-a", 25 → "member-z", 26 → "member-aa", ...).
    pub fn next_member(sequence: usize) -> Self {
        let suffix = member_id_suffix(sequence);
        Self(format!("member-{suffix}"))
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for AgentId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

fn member_id_suffix(n: usize) -> String {
    let mut n = n;
    let mut result = String::new();
    loop {
        result.insert(0, (b'a' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    result
}
