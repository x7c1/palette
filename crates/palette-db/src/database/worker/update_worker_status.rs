use super::super::Database;
use crate::lookup;
use palette_domain::worker::*;
use rusqlite::params;

impl Database {
    /// Update a worker's status.
    pub fn update_worker_status(&self, id: &WorkerId, status: WorkerStatus) -> crate::Result<()> {
        let conn = lock!(self.conn);
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE workers SET status_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![lookup::worker_status_id(status), now, id.as_ref()],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::insert_worker::tests::insert_test_worker;
    use crate::database::test_helpers::test_db;
    use palette_domain::worker::*;

    #[test]
    fn update_worker_status() {
        let db = test_db();
        insert_test_worker(&db, "member-1", WorkerRole::Member, "wf-1");

        db.update_worker_status(&WorkerId::new("member-1"), WorkerStatus::Idle)
            .unwrap();
        let worker = db.find_worker(&WorkerId::new("member-1")).unwrap().unwrap();
        assert_eq!(worker.status, WorkerStatus::Idle);
    }
}
