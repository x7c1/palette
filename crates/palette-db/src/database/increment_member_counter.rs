use super::Database;
use crate::error::Error;
use palette_domain::workflow::WorkflowId;
use rusqlite::params;

impl Database {
    /// Atomically increment the workflow's member counter and return the previous value.
    /// The returned value can be used as a sequence number for the new member.
    pub fn increment_member_counter(&self, workflow_id: &WorkflowId) -> crate::Result<usize> {
        let conn = lock!(self.conn);
        let prev: i64 = conn.query_row(
            "UPDATE workflows SET member_counter = member_counter + 1 WHERE id = ?1
             RETURNING member_counter - 1",
            params![workflow_id.as_ref()],
            |row| row.get(0),
        )?;
        Ok(prev as usize)
    }
}
