use super::super::Database;
use super::row::row_to_agent_state;
use palette_domain::agent::*;
use rusqlite::params;

impl Database {
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
}
