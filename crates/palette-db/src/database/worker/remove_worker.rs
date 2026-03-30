use super::super::{Database, lock};
use palette_domain::worker::*;
use rusqlite::params;

impl Database {
    /// Remove a worker by ID, returning the removed state.
    pub fn remove_worker(&self, id: &WorkerId) -> crate::Result<Option<WorkerState>> {
        let worker = self.find_worker(id)?;
        if worker.is_some() {
            let conn = lock(&self.conn)?;
            conn.execute("DELETE FROM workers WHERE id = ?1", params![id.as_ref()])?;
        }
        Ok(worker)
    }
}

#[cfg(test)]
mod tests {
    use super::super::insert_worker::tests::insert_test_worker;
    use crate::database::test_helpers::test_db;
    use palette_domain::worker::*;

    #[test]
    fn remove_worker() {
        let db = test_db();
        insert_test_worker(&db, "member-1", WorkerRole::Member, "wf-1");

        let removed = db
            .remove_worker(&WorkerId::parse("member-1").unwrap())
            .unwrap();
        assert!(removed.is_some());
        assert!(
            db.find_worker(&WorkerId::parse("member-1").unwrap())
                .unwrap()
                .is_none()
        );
    }
}
