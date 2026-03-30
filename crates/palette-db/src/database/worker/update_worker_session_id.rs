use super::super::{Database, lock};
use palette_domain::worker::{WorkerId, WorkerSessionId};
use rusqlite::params;

impl Database {
    /// Update the session_id for a worker (set when Claude Code reports it via hooks).
    pub fn update_worker_session_id(
        &self,
        id: &WorkerId,
        session_id: &WorkerSessionId,
    ) -> crate::Result<()> {
        let conn = lock(&self.conn)?;
        conn.execute(
            "UPDATE workers SET session_id = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![session_id.as_ref(), id.as_ref()],
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
    fn update_session_id() {
        let db = test_db();
        insert_test_worker(&db, "member-1", WorkerRole::Member, "wf-1");

        let session_id = WorkerSessionId::new("session-abc");
        db.update_worker_session_id(&WorkerId::parse("member-1").unwrap(), &session_id)
            .unwrap();

        let worker = db
            .find_worker(&WorkerId::parse("member-1").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(worker.session_id, Some(session_id));
    }
}
