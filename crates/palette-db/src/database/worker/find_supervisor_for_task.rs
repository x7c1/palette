use super::super::{Database, lock};
use super::row::{COLUMNS, row_to_worker_state};
use crate::lookup;
use palette_domain::task::TaskId;
use palette_domain::worker::*;
use rusqlite::params;

impl Database {
    /// Find the supervisor assigned to a specific task.
    pub fn find_supervisor_for_task(&self, task_id: &TaskId) -> crate::Result<Option<WorkerState>> {
        let conn = lock(&self.conn)?;
        let role_leader = lookup::worker_role_id(WorkerRole::Leader);
        let role_ri = lookup::worker_role_id(WorkerRole::ReviewIntegrator);
        let sql =
            format!("SELECT {COLUMNS} FROM workers WHERE task_id = ?1 AND role_id IN (?2, ?3)");
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query_map(
            params![task_id.as_ref(), role_leader, role_ri],
            row_to_worker_state,
        )?;
        rows.next().transpose().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::super::insert_worker::tests::insert_test_worker;
    use crate::database::test_helpers::test_db;
    use palette_domain::task::TaskId;
    use palette_domain::worker::*;

    #[test]
    fn find_supervisor_for_task() {
        let db = test_db();
        insert_test_worker(&db, "leader-1", WorkerRole::Leader, "wf-1");
        insert_test_worker(&db, "member-1", WorkerRole::Member, "wf-1");

        let sup = db
            .find_supervisor_for_task(&TaskId::parse("wf-1:leader-1").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(sup.id, WorkerId::new("leader-1"));

        // Member should not be found
        assert!(
            db.find_supervisor_for_task(&TaskId::parse("wf-1:member-1").unwrap())
                .unwrap()
                .is_none()
        );
    }
}
