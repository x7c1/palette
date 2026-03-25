use super::Database;
use palette_domain::workflow::WorkflowId;
use rusqlite::params;

impl Database {
    /// Atomically increment the workflow's worker counter and return the previous value.
    /// The returned value can be used as a sequence number for the new worker.
    pub fn increment_worker_counter(&self, workflow_id: &WorkflowId) -> crate::Result<usize> {
        let conn = lock!(self.conn);
        let prev: i64 = conn.query_row(
            "UPDATE workflows SET worker_counter = worker_counter + 1 WHERE id = ?1
             RETURNING worker_counter - 1",
            params![workflow_id.as_ref()],
            |row| row.get(0),
        )?;
        Ok(prev as usize)
    }
}
