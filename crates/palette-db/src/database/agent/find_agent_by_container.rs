use super::super::Database;
use super::row::row_to_agent_state;
use palette_domain::agent::*;
use rusqlite::params;

impl Database {
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
}

#[cfg(test)]
mod tests {
    use super::super::insert_agent::tests::insert_test_agent;
    use crate::database::test_helpers::test_db;
    use palette_domain::agent::*;

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
}
