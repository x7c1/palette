use super::super::Database;
use palette_domain::agent::*;
use rusqlite::params;

impl Database {
    /// Remove an agent by ID, returning the removed state.
    pub fn remove_agent(&self, id: &AgentId) -> crate::Result<Option<AgentState>> {
        let agent = self.find_agent(id)?;
        if agent.is_some() {
            let conn = lock!(self.conn);
            conn.execute("DELETE FROM agents WHERE id = ?1", params![id.as_ref()])?;
        }
        Ok(agent)
    }
}

#[cfg(test)]
mod tests {
    use super::super::insert_agent::tests::insert_test_agent;
    use crate::database::test_helpers::test_db;
    use palette_domain::agent::*;

    #[test]
    fn remove_agent() {
        let db = test_db();
        insert_test_agent(&db, "member-1", AgentRole::Member, "wf-1");

        let removed = db.remove_agent(&AgentId::new("member-1")).unwrap();
        assert!(removed.is_some());
        assert!(db.find_agent(&AgentId::new("member-1")).unwrap().is_none());
    }
}
