use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberStatus {
    Booting,
    Working,
    Idle,
    WaitingPermission,
    Crashed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberState {
    pub id: String,
    pub role: String,
    pub leader_id: String,
    pub container_id: String,
    pub tmux_target: String,
    pub status: MemberStatus,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentState {
    pub session_name: String,
    pub leaders: Vec<MemberState>,
    pub members: Vec<MemberState>,
    pub created_at: String,
    pub updated_at: String,
}

impl PersistentState {
    pub fn new(session_name: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            session_name,
            leaders: Vec::new(),
            members: Vec::new(),
            created_at: now.clone(),
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

    pub fn find_member(&self, id: &str) -> Option<&MemberState> {
        self.members.iter().find(|m| m.id == id)
    }

    pub fn find_member_mut(&mut self, id: &str) -> Option<&mut MemberState> {
        self.members.iter_mut().find(|m| m.id == id)
    }

    pub fn find_leader(&self, id: &str) -> Option<&MemberState> {
        self.leaders.iter().find(|m| m.id == id)
    }

    pub fn find_leader_mut(&mut self, id: &str) -> Option<&mut MemberState> {
        self.leaders.iter_mut().find(|m| m.id == id)
    }

    /// Find any agent (leader or member) by container_id.
    pub fn find_by_container(&self, container_id: &str) -> Option<&MemberState> {
        self.leaders
            .iter()
            .chain(self.members.iter())
            .find(|m| m.container_id == container_id)
    }

    /// Remove a member by ID, returning the removed state.
    pub fn remove_member(&mut self, id: &str) -> Option<MemberState> {
        if let Some(pos) = self.members.iter().position(|m| m.id == id) {
            Some(self.members.remove(pos))
        } else {
            None
        }
    }

    /// Generate the next member ID (member-a, member-b, ..., member-z, member-aa, ...).
    pub fn next_member_id(&self) -> String {
        let existing_count = self.members.len();
        let suffix = member_id_suffix(existing_count);
        format!("member-{suffix}")
    }

    pub fn touch(&mut self) {
        self.updated_at = chrono::Utc::now().to_rfc3339();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");

        let mut state = PersistentState::new("test-session".to_string());
        state.members.push(MemberState {
            id: "member-a".to_string(),
            role: "member".to_string(),
            leader_id: "leader-1".to_string(),
            container_id: "abc123".to_string(),
            tmux_target: "test-session:member-a".to_string(),
            status: MemberStatus::Idle,
            session_id: None,
        });

        state.save(&path).unwrap();
        let loaded = PersistentState::load(&path).unwrap().unwrap();
        assert_eq!(loaded.session_name, "test-session");
        assert_eq!(loaded.members.len(), 1);
        assert_eq!(loaded.members[0].id, "member-a");
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
        state.members.push(MemberState {
            id: "member-a".to_string(),
            role: "member".to_string(),
            leader_id: "leader-1".to_string(),
            container_id: "abc123".to_string(),
            tmux_target: "test:member-a".to_string(),
            status: MemberStatus::Idle,
            session_id: None,
        });

        assert!(state.find_member("member-a").is_some());
        assert!(state.find_member("member-b").is_none());
    }
}
