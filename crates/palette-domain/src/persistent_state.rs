use crate::{AgentId, AgentState, ContainerId};
use chrono::{DateTime, Utc};

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
