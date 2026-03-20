use super::Database;
use crate::error::Error;
use palette_domain::workflow::{WorkflowId, WorkflowStatus};
use rusqlite::params;

impl Database {
    pub fn update_workflow_status(
        &self,
        id: &WorkflowId,
        status: WorkflowStatus,
    ) -> crate::Result<()> {
        let conn = lock!(self.conn);
        conn.execute(
            "UPDATE workflows SET status = ?1 WHERE id = ?2",
            params![status.as_str(), id.as_ref()],
        )?;
        Ok(())
    }
}
