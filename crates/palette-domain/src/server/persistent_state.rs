use crate::agent::{AgentId, AgentState, ContainerId};
use crate::task::TaskId;
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

    /// Restore state from a saved file.
    pub fn restore(
        session_name: String,
        supervisors: Vec<AgentState>,
        members: Vec<AgentState>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            session_name,
            supervisors,
            members,
            created_at,
            updated_at,
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

    /// Find the supervisor assigned to a specific composite task.
    pub fn find_supervisor_for_task(&self, task_id: &TaskId) -> Option<&AgentState> {
        self.supervisors.iter().find(|s| s.task_id == *task_id)
    }

    /// Remove a supervisor by ID, returning the removed state.
    pub fn remove_supervisor(&mut self, id: &AgentId) -> Option<AgentState> {
        if let Some(pos) = self.supervisors.iter().position(|s| s.id == *id) {
            Some(self.supervisors.remove(pos))
        } else {
            None
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentRole, AgentStatus};
    use crate::terminal::TerminalTarget;

    fn make_supervisor(id: &str, role: AgentRole) -> AgentState {
        use crate::task::TaskId;
        AgentState {
            id: AgentId::new(id),
            role,
            supervisor_id: AgentId::new(""),
            container_id: ContainerId::new(format!("container-{id}")),
            terminal_target: TerminalTarget::new(format!("pane-{id}")),
            status: AgentStatus::Idle,
            session_id: None,
            task_id: TaskId::new(format!("task-{id}")),
        }
    }

    #[test]
    fn find_supervisor_for_task_returns_matching() {
        let mut state = PersistentState::new("test".to_string());
        state
            .supervisors
            .push(make_supervisor("leader-1", AgentRole::Leader));
        state
            .supervisors
            .push(make_supervisor("ri-1", AgentRole::ReviewIntegrator));

        assert_eq!(
            state
                .find_supervisor_for_task(&TaskId::new("task-leader-1"))
                .unwrap()
                .id,
            AgentId::new("leader-1")
        );
        assert_eq!(
            state
                .find_supervisor_for_task(&TaskId::new("task-ri-1"))
                .unwrap()
                .id,
            AgentId::new("ri-1")
        );
        assert!(
            state
                .find_supervisor_for_task(&TaskId::new("nonexistent"))
                .is_none()
        );
    }

    #[test]
    fn remove_supervisor_returns_removed() {
        let mut state = PersistentState::new("test".to_string());
        state
            .supervisors
            .push(make_supervisor("leader-1", AgentRole::Leader));
        state
            .supervisors
            .push(make_supervisor("ri-1", AgentRole::ReviewIntegrator));

        let removed = state.remove_supervisor(&AgentId::new("leader-1"));
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, AgentId::new("leader-1"));
        assert_eq!(state.supervisors.len(), 1);

        assert!(
            state
                .remove_supervisor(&AgentId::new("nonexistent"))
                .is_none()
        );
    }
}
