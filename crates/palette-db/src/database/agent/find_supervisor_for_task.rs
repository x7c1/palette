use super::super::Database;
use super::row::row_to_agent_state;
use crate::lookup;
use palette_domain::agent::*;
use palette_domain::task::TaskId;
use rusqlite::params;

impl Database {
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
}

#[cfg(test)]
mod tests {
    use super::super::insert_agent::tests::insert_test_agent;
    use crate::database::test_helpers::test_db;
    use palette_domain::agent::*;
    use palette_domain::task::TaskId;

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
}
