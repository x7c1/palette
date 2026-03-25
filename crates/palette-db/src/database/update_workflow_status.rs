use super::Database;
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
            "UPDATE workflows SET status_id = ?1 WHERE id = ?2",
            params![crate::lookup::workflow_status_id(status), id.as_ref()],
        )?;
        Ok(())
    }
}
