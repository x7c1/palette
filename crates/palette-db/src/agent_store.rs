use crate::database::{Database, id_conversion_error};
use crate::lookup;
use palette_domain::agent::*;
use palette_domain::task::TaskId;
use palette_domain::terminal::TerminalTarget;
use palette_domain::workflow::WorkflowId;
use rusqlite::params;

/// Request to insert a new agent.
pub struct InsertAgentRequest {
    pub id: AgentId,
    pub workflow_id: WorkflowId,
    pub role: AgentRole,
    pub status: AgentStatus,
    pub supervisor_id: AgentId,
    pub container_id: ContainerId,
    pub terminal_target: TerminalTarget,
    pub session_id: Option<AgentSessionId>,
    pub task_id: TaskId,
}

impl Database {
    /// Insert a new agent record.
    pub fn insert_agent(&self, req: &InsertAgentRequest) -> crate::Result<()> {
        let conn = lock!(self.conn);
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO agents (id, workflow_id, role_id, status_id, supervisor_id, container_id, terminal_target, session_id, task_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                req.id.as_ref(),
                req.workflow_id.as_ref(),
                lookup::agent_role_id(req.role),
                lookup::agent_status_id(req.status),
                req.supervisor_id.as_ref(),
                req.container_id.as_ref(),
                req.terminal_target.as_ref(),
                req.session_id.as_ref().map(|s| s.as_ref()),
                req.task_id.as_ref(),
                now,
                now,
            ],
        )?;
        Ok(())
    }

    /// Find an agent by ID.
    pub fn find_agent(&self, id: &AgentId) -> crate::Result<Option<AgentState>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, role_id, status_id, supervisor_id, container_id, terminal_target, session_id, task_id
             FROM agents WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id.as_ref()], row_to_agent_state)?;
        rows.next().transpose().map_err(Into::into)
    }

    /// Find an agent by container ID.
    pub fn find_agent_by_container(
        &self,
        container_id: &ContainerId,
    ) -> crate::Result<Option<AgentState>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, role_id, status_id, supervisor_id, container_id, terminal_target, session_id, task_id
             FROM agents WHERE container_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![container_id.as_ref()], row_to_agent_state)?;
        rows.next().transpose().map_err(Into::into)
    }

    /// Find the supervisor assigned to a specific task.
    pub fn find_supervisor_for_task(&self, task_id: &TaskId) -> crate::Result<Option<AgentState>> {
        let conn = lock!(self.conn);
        let role_leader = lookup::agent_role_id(AgentRole::Leader);
        let role_ri = lookup::agent_role_id(AgentRole::ReviewIntegrator);
        let mut stmt = conn.prepare(
            "SELECT id, role_id, status_id, supervisor_id, container_id, terminal_target, session_id, task_id
             FROM agents WHERE task_id = ?1 AND role_id IN (?2, ?3)",
        )?;
        let mut rows = stmt.query_map(
            params![task_id.as_ref(), role_leader, role_ri],
            row_to_agent_state,
        )?;
        rows.next().transpose().map_err(Into::into)
    }

    /// Update an agent's status.
    pub fn update_agent_status(&self, id: &AgentId, status: AgentStatus) -> crate::Result<()> {
        let conn = lock!(self.conn);
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE agents SET status_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![lookup::agent_status_id(status), now, id.as_ref()],
        )?;
        Ok(())
    }

    /// Remove an agent by ID, returning the removed state.
    pub fn remove_agent(&self, id: &AgentId) -> crate::Result<Option<AgentState>> {
        let agent = self.find_agent(id)?;
        if agent.is_some() {
            let conn = lock!(self.conn);
            conn.execute("DELETE FROM agents WHERE id = ?1", params![id.as_ref()])?;
        }
        Ok(agent)
    }

    /// List all agents with a given role for a workflow.
    pub fn list_supervisors(&self, workflow_id: &WorkflowId) -> crate::Result<Vec<AgentState>> {
        let conn = lock!(self.conn);
        let role_leader = lookup::agent_role_id(AgentRole::Leader);
        let role_ri = lookup::agent_role_id(AgentRole::ReviewIntegrator);
        let mut stmt = conn.prepare(
            "SELECT id, role_id, status_id, supervisor_id, container_id, terminal_target, session_id, task_id
             FROM agents WHERE workflow_id = ?1 AND role_id IN (?2, ?3)",
        )?;
        let rows = stmt.query_map(
            params![workflow_id.as_ref(), role_leader, role_ri],
            row_to_agent_state,
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// List all members for a workflow.
    pub fn list_members(&self, workflow_id: &WorkflowId) -> crate::Result<Vec<AgentState>> {
        let conn = lock!(self.conn);
        let role_member = lookup::agent_role_id(AgentRole::Member);
        let mut stmt = conn.prepare(
            "SELECT id, role_id, status_id, supervisor_id, container_id, terminal_target, session_id, task_id
             FROM agents WHERE workflow_id = ?1 AND role_id = ?2",
        )?;
        let rows = stmt.query_map(
            params![workflow_id.as_ref(), role_member],
            row_to_agent_state,
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// List all agents (all workflows).
    pub fn list_all_agents(&self) -> crate::Result<Vec<AgentState>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, role_id, status_id, supervisor_id, container_id, terminal_target, session_id, task_id
             FROM agents",
        )?;
        let rows = stmt.query_map([], row_to_agent_state)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// List all agents in Booting status (all workflows).
    pub fn list_booting_agents(&self) -> crate::Result<Vec<AgentState>> {
        let conn = lock!(self.conn);
        let booting_id = lookup::agent_status_id(AgentStatus::Booting);
        let mut stmt = conn.prepare(
            "SELECT id, role_id, status_id, supervisor_id, container_id, terminal_target, session_id, task_id
             FROM agents WHERE status_id = ?1",
        )?;
        let rows = stmt.query_map(params![booting_id], row_to_agent_state)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// List all agents that are idle or waiting for permission (all workflows).
    pub fn list_idle_or_waiting_agents(&self) -> crate::Result<Vec<AgentState>> {
        let conn = lock!(self.conn);
        let idle_id = lookup::agent_status_id(AgentStatus::Idle);
        let waiting_id = lookup::agent_status_id(AgentStatus::WaitingPermission);
        let mut stmt = conn.prepare(
            "SELECT id, role_id, status_id, supervisor_id, container_id, terminal_target, session_id, task_id
             FROM agents WHERE status_id IN (?1, ?2)",
        )?;
        let rows = stmt.query_map(params![idle_id, waiting_id], row_to_agent_state)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

fn row_to_agent_state(row: &rusqlite::Row) -> rusqlite::Result<AgentState> {
    let role_id: i64 = row.get("role_id")?;
    let status_id: i64 = row.get("status_id")?;
    Ok(AgentState {
        id: AgentId::new(row.get::<_, String>("id")?),
        role: lookup::agent_role_from_id(role_id).map_err(id_conversion_error)?,
        status: lookup::agent_status_from_id(status_id).map_err(id_conversion_error)?,
        supervisor_id: AgentId::new(row.get::<_, String>("supervisor_id")?),
        container_id: ContainerId::new(row.get::<_, String>("container_id")?),
        terminal_target: TerminalTarget::new(row.get::<_, String>("terminal_target")?),
        session_id: row
            .get::<_, Option<String>>("session_id")?
            .map(AgentSessionId::new),
        task_id: TaskId::new(row.get::<_, String>("task_id")?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_helpers::test_db;

    fn insert_test_agent(db: &Database, id: &str, role: AgentRole, workflow_id: &str) {
        let wf_id = WorkflowId::new(workflow_id);
        let _ = db.create_workflow(&wf_id, "test/blueprint.yaml");
        db.insert_agent(&InsertAgentRequest {
            id: AgentId::new(id),
            workflow_id: wf_id,
            role,
            status: AgentStatus::Booting,
            supervisor_id: AgentId::new(""),
            container_id: ContainerId::new(format!("container-{id}")),
            terminal_target: TerminalTarget::new(format!("pane-{id}")),
            session_id: None,
            task_id: TaskId::new(format!("task-{id}")),
        })
        .unwrap();
    }

    #[test]
    fn insert_and_find_agent() {
        let db = test_db();
        insert_test_agent(&db, "leader-1", AgentRole::Leader, "wf-1");

        let agent = db.find_agent(&AgentId::new("leader-1")).unwrap().unwrap();
        assert_eq!(agent.id, AgentId::new("leader-1"));
        assert_eq!(agent.role, AgentRole::Leader);
        assert_eq!(agent.status, AgentStatus::Booting);
    }

    #[test]
    fn find_agent_by_container() {
        let db = test_db();
        insert_test_agent(&db, "member-1", AgentRole::Member, "wf-1");

        let agent = db
            .find_agent_by_container(&ContainerId::new("container-member-1"))
            .unwrap()
            .unwrap();
        assert_eq!(agent.id, AgentId::new("member-1"));
    }

    #[test]
    fn find_supervisor_for_task() {
        let db = test_db();
        insert_test_agent(&db, "leader-1", AgentRole::Leader, "wf-1");
        insert_test_agent(&db, "member-1", AgentRole::Member, "wf-1");

        let sup = db
            .find_supervisor_for_task(&TaskId::new("task-leader-1"))
            .unwrap()
            .unwrap();
        assert_eq!(sup.id, AgentId::new("leader-1"));

        // Member should not be found
        assert!(
            db.find_supervisor_for_task(&TaskId::new("task-member-1"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn update_agent_status() {
        let db = test_db();
        insert_test_agent(&db, "member-1", AgentRole::Member, "wf-1");

        db.update_agent_status(&AgentId::new("member-1"), AgentStatus::Idle)
            .unwrap();
        let agent = db.find_agent(&AgentId::new("member-1")).unwrap().unwrap();
        assert_eq!(agent.status, AgentStatus::Idle);
    }

    #[test]
    fn remove_agent() {
        let db = test_db();
        insert_test_agent(&db, "member-1", AgentRole::Member, "wf-1");

        let removed = db.remove_agent(&AgentId::new("member-1")).unwrap();
        assert!(removed.is_some());
        assert!(db.find_agent(&AgentId::new("member-1")).unwrap().is_none());
    }

    #[test]
    fn list_booting_agents() {
        let db = test_db();
        insert_test_agent(&db, "leader-1", AgentRole::Leader, "wf-1");
        insert_test_agent(&db, "member-1", AgentRole::Member, "wf-1");
        db.update_agent_status(&AgentId::new("member-1"), AgentStatus::Idle)
            .unwrap();

        let booting = db.list_booting_agents().unwrap();
        assert_eq!(booting.len(), 1);
        assert_eq!(booting[0].id, AgentId::new("leader-1"));
    }
}
