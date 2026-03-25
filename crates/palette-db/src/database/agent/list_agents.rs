use super::super::Database;
use super::row::row_to_agent_state;
use crate::lookup;
use palette_domain::agent::*;
use palette_domain::workflow::WorkflowId;
use rusqlite::params;

impl Database {
    /// List all supervisors for a workflow.
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

#[cfg(test)]
mod tests {
    use super::super::insert_agent::tests::insert_test_agent;
    use crate::database::test_helpers::test_db;
    use palette_domain::agent::*;

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
