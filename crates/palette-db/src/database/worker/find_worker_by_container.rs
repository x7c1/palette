use super::super::Database;
use super::row::row_to_worker_state;
use palette_domain::worker::*;
use rusqlite::params;

impl Database {
    /// Find a worker by container ID.
    pub fn find_worker_by_container(
        &self,
        container_id: &ContainerId,
    ) -> crate::Result<Option<WorkerState>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, role_id, status_id, supervisor_id, container_id, terminal_target, session_id, task_id
             FROM workers WHERE container_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![container_id.as_ref()], row_to_worker_state)?;
        rows.next().transpose().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::super::insert_worker::tests::insert_test_worker;
    use crate::database::test_helpers::test_db;
    use palette_domain::worker::*;

    #[test]
    fn find_worker_by_container() {
        let db = test_db();
        insert_test_worker(&db, "member-1", WorkerRole::Member, "wf-1");

        let worker = db
            .find_worker_by_container(&ContainerId::new("container-member-1"))
            .unwrap()
            .unwrap();
        assert_eq!(worker.id, WorkerId::new("member-1"));
    }
}
