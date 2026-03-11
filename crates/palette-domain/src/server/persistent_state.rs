use crate::agent::{AgentId, AgentRole, AgentState, ContainerId};
use crate::job::JobType;
use chrono::{DateTime, Utc};

pub struct PersistentState {
    pub session_name: String,
    pub supervisors: Vec<AgentState>,
    pub members: Vec<AgentState>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PersistentState {
    pub fn new(session_name: String) -> Self {
        let now = Utc::now();
        Self {
            session_name,
            supervisors: Vec::new(),
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

    pub fn find_supervisor(&self, id: &AgentId) -> Option<&AgentState> {
        self.supervisors.iter().find(|m| m.id == *id)
    }

    pub fn find_supervisor_mut(&mut self, id: &AgentId) -> Option<&mut AgentState> {
        self.supervisors.iter_mut().find(|m| m.id == *id)
    }

    /// Find any agent (supervisor or member) by container_id.
    pub fn find_by_container(&self, container_id: &ContainerId) -> Option<&AgentState> {
        self.supervisors
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

    /// Find the leader (AgentRole::Leader).
    pub fn find_leader(&self) -> Option<&AgentState> {
        self.supervisors
            .iter()
            .find(|l| l.role == AgentRole::Leader)
    }

    /// Find the review integrator.
    pub fn find_review_integrator(&self) -> Option<&AgentState> {
        self.supervisors
            .iter()
            .find(|l| l.role == AgentRole::ReviewIntegrator)
    }

    /// Determine the supervisor_id for a new member based on job type.
    /// Craft jobs → leader, Review jobs → review integrator (fallback to leader).
    pub fn supervisor_id_for_job_type(&self, job_type: JobType) -> AgentId {
        match job_type {
            JobType::Review => self
                .find_review_integrator()
                .or_else(|| self.find_leader())
                .map(|l| l.id.clone())
                .unwrap_or_else(|| AgentId::new("")),
            JobType::Craft => self
                .find_leader()
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

    fn make_supervisor(id: &str, role: AgentRole) -> AgentState {
        AgentState {
            id: AgentId::new(id),
            role,
            supervisor_id: AgentId::new(""),
            container_id: ContainerId::new(format!("container-{id}")),
            terminal_target: TerminalTarget::new(format!("pane-{id}")),
            status: AgentStatus::Idle,
            session_id: None,
        }
    }

    #[test]
    fn supervisor_id_for_craft_returns_leader() {
        let mut state = PersistentState::new("test".to_string());
        state
            .supervisors
            .push(make_supervisor("leader-1", AgentRole::Leader));
        state
            .supervisors
            .push(make_supervisor("ri-1", AgentRole::ReviewIntegrator));

        assert_eq!(
            state.supervisor_id_for_job_type(JobType::Craft),
            AgentId::new("leader-1")
        );
    }

    #[test]
    fn supervisor_id_for_review_returns_review_integrator() {
        let mut state = PersistentState::new("test".to_string());
        state
            .supervisors
            .push(make_supervisor("leader-1", AgentRole::Leader));
        state
            .supervisors
            .push(make_supervisor("ri-1", AgentRole::ReviewIntegrator));

        assert_eq!(
            state.supervisor_id_for_job_type(JobType::Review),
            AgentId::new("ri-1")
        );
    }

    #[test]
    fn supervisor_id_for_review_falls_back_to_leader() {
        let mut state = PersistentState::new("test".to_string());
        state
            .supervisors
            .push(make_supervisor("leader-1", AgentRole::Leader));

        assert_eq!(
            state.supervisor_id_for_job_type(JobType::Review),
            AgentId::new("leader-1")
        );
    }

    #[test]
    fn find_leader_and_review_integrator() {
        let mut state = PersistentState::new("test".to_string());
        state
            .supervisors
            .push(make_supervisor("leader-1", AgentRole::Leader));
        state
            .supervisors
            .push(make_supervisor("ri-1", AgentRole::ReviewIntegrator));

        assert_eq!(state.find_leader().unwrap().id, AgentId::new("leader-1"));
        assert_eq!(
            state.find_review_integrator().unwrap().id,
            AgentId::new("ri-1")
        );
    }
}
