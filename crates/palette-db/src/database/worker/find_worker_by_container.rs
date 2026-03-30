use super::super::{Database, lock};
use super::row::{COLUMNS, into_worker_state, read_worker_row};
use palette_domain::worker::*;
use rusqlite::params;

impl Database {
    /// Find a worker by container ID.
    pub fn find_worker_by_container(
        &self,
        container_id: &ContainerId,
    ) -> crate::Result<Option<WorkerState>> {
        let conn = lock(&self.conn)?;
        let sql = format!("SELECT {COLUMNS} FROM workers WHERE container_id = ?1");
        let mut stmt = conn.prepare(&sql)?;
        stmt.query_map(params![container_id.as_ref()], read_worker_row)?
            .next()
            .transpose()?
            .map(into_worker_state)
            .transpose()
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
