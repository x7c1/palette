use crate::task::TaskId;
use crate::worker::{ContainerId, WorkerId, WorkerState};
use chrono::{DateTime, Utc};

pub struct PersistentState {
    pub session_name: String,
    pub supervisors: Vec<WorkerState>,
    pub members: Vec<WorkerState>,
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
        supervisors: Vec<WorkerState>,
        members: Vec<WorkerState>,
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

    pub fn find_member(&self, id: &WorkerId) -> Option<&WorkerState> {
        self.members.iter().find(|m| m.id == *id)
    }

    pub fn find_member_mut(&mut self, id: &WorkerId) -> Option<&mut WorkerState> {
        self.members.iter_mut().find(|m| m.id == *id)
    }

    pub fn find_supervisor(&self, id: &WorkerId) -> Option<&WorkerState> {
        self.supervisors.iter().find(|m| m.id == *id)
    }

    pub fn find_supervisor_mut(&mut self, id: &WorkerId) -> Option<&mut WorkerState> {
        self.supervisors.iter_mut().find(|m| m.id == *id)
    }

    /// Find any worker (supervisor or member) by container_id.
    pub fn find_by_container(&self, container_id: &ContainerId) -> Option<&WorkerState> {
        self.supervisors
            .iter()
            .chain(self.members.iter())
            .find(|m| m.container_id == *container_id)
    }

    /// Remove a member by ID, returning the removed state.
    pub fn remove_member(&mut self, id: &WorkerId) -> Option<WorkerState> {
        if let Some(pos) = self.members.iter().position(|m| m.id == *id) {
            Some(self.members.remove(pos))
        } else {
            None
        }
    }

    /// Find the supervisor assigned to a specific composite task.
    pub fn find_supervisor_for_task(&self, task_id: &TaskId) -> Option<&WorkerState> {
        self.supervisors.iter().find(|s| s.task_id == *task_id)
    }

    /// Remove a supervisor by ID, returning the removed state.
    pub fn remove_supervisor(&mut self, id: &WorkerId) -> Option<WorkerState> {
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
    use crate::terminal::TerminalTarget;
    use crate::worker::{WorkerRole, WorkerStatus};

    fn make_supervisor(id: &str, role: WorkerRole) -> WorkerState {
        use crate::task::TaskId;
        WorkerState {
            id: WorkerId::parse(id).unwrap(),
            workflow_id: crate::workflow::WorkflowId::parse("wf-test").unwrap(),
            role,
            supervisor_id: None,
            container_id: ContainerId::new(format!("container-{id}")),
            terminal_target: TerminalTarget::new(format!("pane-{id}")),
            status: WorkerStatus::Idle,
            session_id: None,
            task_id: TaskId::parse(format!("wf-test:{id}")).unwrap(),
        }
    }

    #[test]
    fn find_supervisor_for_task_returns_matching() {
        let mut state = PersistentState::new("test".to_string());
        state
            .supervisors
            .push(make_supervisor("leader-1", WorkerRole::Leader));
        state
            .supervisors
            .push(make_supervisor("ri-1", WorkerRole::ReviewIntegrator));

        assert_eq!(
            state
                .find_supervisor_for_task(&TaskId::parse("wf-test:leader-1").unwrap())
                .unwrap()
                .id,
            WorkerId::parse("leader-1").unwrap()
        );
        assert_eq!(
            state
                .find_supervisor_for_task(&TaskId::parse("wf-test:ri-1").unwrap())
                .unwrap()
                .id,
            WorkerId::parse("ri-1").unwrap()
        );
        assert!(
            state
                .find_supervisor_for_task(&TaskId::parse("wf-test:nonexistent").unwrap())
                .is_none()
        );
    }

    #[test]
    fn remove_supervisor_returns_removed() {
        let mut state = PersistentState::new("test".to_string());
        state
            .supervisors
            .push(make_supervisor("leader-1", WorkerRole::Leader));
        state
            .supervisors
            .push(make_supervisor("ri-1", WorkerRole::ReviewIntegrator));

        let removed = state.remove_supervisor(&WorkerId::parse("leader-1").unwrap());
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, WorkerId::parse("leader-1").unwrap());
        assert_eq!(state.supervisors.len(), 1);

        assert!(
            state
                .remove_supervisor(&WorkerId::parse("nonexistent").unwrap())
                .is_none()
        );
    }
}
