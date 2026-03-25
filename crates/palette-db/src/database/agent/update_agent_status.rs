use super::super::Database;
use crate::lookup;
use palette_domain::agent::*;
use rusqlite::params;

impl Database {
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
}

#[cfg(test)]
mod tests {
    use super::super::insert_agent::tests::insert_test_agent;
    use crate::database::test_helpers::test_db;
    use palette_domain::agent::*;

    #[test]
    fn update_agent_status() {
        let db = test_db();
        insert_test_agent(&db, "member-1", AgentRole::Member, "wf-1");

        db.update_agent_status(&AgentId::new("member-1"), AgentStatus::Idle)
            .unwrap();
        let agent = db.find_agent(&AgentId::new("member-1")).unwrap().unwrap();
        assert_eq!(agent.status, AgentStatus::Idle);
    }
}
