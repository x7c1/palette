use crate::agent::{AgentId, AgentRole, AgentState, ContainerId};
use crate::task::TaskType;
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

    /// Find the main leader (AgentRole::Leader).
    pub fn find_main_leader(&self) -> Option<&AgentState> {
        self.leaders.iter().find(|l| l.role == AgentRole::Leader)
    }

    /// Find the review integrator leader.
    pub fn find_review_integrator(&self) -> Option<&AgentState> {
        self.leaders
            .iter()
            .find(|l| l.role == AgentRole::ReviewIntegrator)
    }

    /// Determine the leader_id for a new member based on task type.
    /// Work tasks → main leader, Review tasks → review integrator (fallback to main leader).
    pub fn leader_id_for_task_type(&self, task_type: TaskType) -> AgentId {
        match task_type {
            TaskType::Review => self
                .find_review_integrator()
                .or_else(|| self.find_main_leader())
                .map(|l| l.id.clone())
                .unwrap_or_else(|| AgentId::new("")),
            TaskType::Work => self
                .find_main_leader()
                .map(|l| l.id.clone())
                .unwrap_or_else(|| AgentId::new("")),
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentStatus;
    use crate::terminal::TerminalTarget;

    fn make_leader(id: &str, role: AgentRole) -> AgentState {
        AgentState {
            id: AgentId::new(id),
            role,
            leader_id: AgentId::new(""),
            container_id: ContainerId::new(format!("container-{id}")),
            terminal_target: TerminalTarget::new(format!("pane-{id}")),
            status: AgentStatus::Idle,
            session_id: None,
        }
    }

    #[test]
    fn leader_id_for_work_returns_main_leader() {
        let mut state = PersistentState::new("test".to_string());
        state
            .leaders
            .push(make_leader("leader-1", AgentRole::Leader));
        state
            .leaders
            .push(make_leader("ri-1", AgentRole::ReviewIntegrator));

        assert_eq!(
            state.leader_id_for_task_type(TaskType::Work),
            AgentId::new("leader-1")
        );
    }

    #[test]
    fn leader_id_for_review_returns_review_integrator() {
        let mut state = PersistentState::new("test".to_string());
        state
            .leaders
            .push(make_leader("leader-1", AgentRole::Leader));
        state
            .leaders
            .push(make_leader("ri-1", AgentRole::ReviewIntegrator));

        assert_eq!(
            state.leader_id_for_task_type(TaskType::Review),
            AgentId::new("ri-1")
        );
    }

    #[test]
    fn leader_id_for_review_falls_back_to_main_leader() {
        let mut state = PersistentState::new("test".to_string());
        state
            .leaders
            .push(make_leader("leader-1", AgentRole::Leader));

        assert_eq!(
            state.leader_id_for_task_type(TaskType::Review),
            AgentId::new("leader-1")
        );
    }

    #[test]
    fn find_main_leader_and_review_integrator() {
        let mut state = PersistentState::new("test".to_string());
        state
            .leaders
            .push(make_leader("leader-1", AgentRole::Leader));
        state
            .leaders
            .push(make_leader("ri-1", AgentRole::ReviewIntegrator));

        assert_eq!(
            state.find_main_leader().unwrap().id,
            AgentId::new("leader-1")
        );
        assert_eq!(
            state.find_review_integrator().unwrap().id,
            AgentId::new("ri-1")
        );
    }
}
