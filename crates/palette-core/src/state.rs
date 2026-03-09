use anyhow::Context as _;
use chrono::{DateTime, Utc};
use palette_db::AgentId;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

/// Docker container identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContainerId(String);

impl ContainerId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for ContainerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ContainerId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Tmux pane reference (e.g., "%42" or "session:window").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TmuxTarget(String);

impl TmuxTarget {
    pub fn new(target: impl Into<String>) -> Self {
        Self(target.into())
    }
}

impl fmt::Display for TmuxTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for TmuxTarget {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Leader,
    Member,
}

impl AgentRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentRole::Leader => "leader",
            AgentRole::Member => "member",
        }
    }
}

impl fmt::Display for AgentRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Booting,
    Working,
    Idle,
    WaitingPermission,
    Crashed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub id: AgentId,
    pub role: AgentRole,
    pub leader_id: AgentId,
    pub container_id: ContainerId,
    pub tmux_target: TmuxTarget,
    pub status: AgentStatus,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentState {
    pub session_name: String,
    pub leaders: Vec<AgentState>,
    pub members: Vec<AgentState>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PersistentState {
    pub fn new(session_name: String) -> Self {
        let now = Utc::now();
        Self {
            session_name,
            leaders: Vec::new(),
            members: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Save state atomically (write to temp file, then rename).
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create state directory: {}", parent.display())
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, &json)
            .with_context(|| format!("failed to write temp state file: {}", tmp_path.display()))?;
        std::fs::rename(&tmp_path, path)
            .with_context(|| format!("failed to rename state file: {}", path.display()))?;
        Ok(())
    }

    /// Load state from file. Returns None if file doesn't exist.
    pub fn load(path: &Path) -> anyhow::Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read state file: {}", path.display()))?;
        let state: Self =
            serde_json::from_str(&content).with_context(|| "failed to parse state file")?;
        Ok(Some(state))
    }

    pub fn find_member(&self, id: &AgentId) -> Option<&AgentState> {
        self.members.iter().find(|m| m.id == *id)
    }

    pub fn find_member_mut(&mut self, id: &AgentId) -> Option<&mut AgentState> {
        self.members.iter_mut().find(|m| m.id == *id)
    }

    pub fn find_leader(&self, id: &AgentId) -> Option<&AgentState> {
        self.leaders.iter().find(|m| m.id == *id)
    }

    pub fn find_leader_mut(&mut self, id: &AgentId) -> Option<&mut AgentState> {
        self.leaders.iter_mut().find(|m| m.id == *id)
    }

    /// Find any agent (leader or member) by container_id.
    pub fn find_by_container(&self, container_id: &ContainerId) -> Option<&AgentState> {
        self.leaders
            .iter()
            .chain(self.members.iter())
            .find(|m| m.container_id == *container_id)
    }

    /// Remove a member by ID, returning the removed state.
    pub fn remove_member(&mut self, id: &AgentId) -> Option<AgentState> {
        if let Some(pos) = self.members.iter().position(|m| m.id == *id) {
            Some(self.members.remove(pos))
        } else {
            None
        }
    }

    /// Generate the next member ID (member-a, member-b, ..., member-z, member-aa, ...).
    pub fn next_member_id(&self) -> AgentId {
        AgentId::next_member(self.members.len())
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aid(s: &str) -> AgentId {
        AgentId::new(s)
    }

    #[test]
    fn save_and_load_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");

        let mut state = PersistentState::new("test-session".to_string());
        state.members.push(AgentState {
            id: aid("member-a"),
            role: AgentRole::Member,
            leader_id: aid("leader-1"),
            container_id: ContainerId::new("abc123"),
            tmux_target: TmuxTarget::new("test-session:member-a"),
            status: AgentStatus::Idle,
            session_id: None,
        });

        state.save(&path).unwrap();
        let loaded = PersistentState::load(&path).unwrap().unwrap();
        assert_eq!(loaded.session_name, "test-session");
        assert_eq!(loaded.members.len(), 1);
        assert_eq!(loaded.members[0].id, aid("member-a"));
    }

    #[test]
    fn load_nonexistent_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.json");
        assert!(PersistentState::load(&path).unwrap().is_none());
    }

    #[test]
    fn atomic_save_leaves_no_tmp_on_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let tmp_path = dir.path().join("state.json.tmp");

        let state = PersistentState::new("test".to_string());
        state.save(&path).unwrap();

        assert!(path.exists());
        assert!(!tmp_path.exists());
    }

    #[test]
    fn find_member() {
        let mut state = PersistentState::new("test".to_string());
        state.members.push(AgentState {
            id: aid("member-a"),
            role: AgentRole::Member,
            leader_id: aid("leader-1"),
            container_id: ContainerId::new("abc123"),
            tmux_target: TmuxTarget::new("test:member-a"),
            status: AgentStatus::Idle,
            session_id: None,
        });

        assert!(state.find_member(&aid("member-a")).is_some());
        assert!(state.find_member(&aid("member-b")).is_none());
    }
}
