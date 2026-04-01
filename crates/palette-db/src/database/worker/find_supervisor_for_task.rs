use super::super::{Database, lock};
use super::row::{COLUMNS, into_worker_state, read_worker_row};
use crate::lookup;
use palette_domain::task::TaskId;
use palette_domain::worker::*;
use rusqlite::params;

impl Database {
    /// Find the first supervisor assigned to a specific task.
    pub fn find_supervisor_for_task(&self, task_id: &TaskId) -> crate::Result<Option<WorkerState>> {
        Ok(self.find_supervisors_for_task(task_id)?.into_iter().next())
    }

    /// Find all supervisors assigned to a specific task.
    pub fn find_supervisors_for_task(&self, task_id: &TaskId) -> crate::Result<Vec<WorkerState>> {
        let conn = lock(&self.conn)?;
        let role_ps = lookup::worker_role_id(WorkerRole::PermissionSupervisor);
        let role_ri = lookup::worker_role_id(WorkerRole::ReviewIntegrator);
        let sql =
            format!("SELECT {COLUMNS} FROM workers WHERE task_id = ?1 AND role_id IN (?2, ?3)");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![task_id.as_ref(), role_ps, role_ri], read_worker_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(into_worker_state(row?)?);
        }
        Ok(result)
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
        insert_test_worker(
            &db,
            "supervisor-1",
            WorkerRole::PermissionSupervisor,
            "wf-1",
        );
        insert_test_worker(&db, "member-1", WorkerRole::Member, "wf-1");

        let sup = db
            .find_supervisor_for_task(&TaskId::parse("wf-1:supervisor-1").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(sup.id, WorkerId::parse("supervisor-1").unwrap());

        // Member should not be found
        assert!(
            db.find_supervisor_for_task(&TaskId::parse("wf-1:member-1").unwrap())
                .unwrap()
                .is_none()
        );
    }
}
