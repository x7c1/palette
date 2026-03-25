use super::super::Database;
use palette_domain::workflow::{WorkflowId, WorkflowStatus};
use rusqlite::params;

impl Database {
    pub fn create_workflow(&self, id: &WorkflowId, blueprint_path: &str) -> crate::Result<()> {
        let conn = lock!(self.conn);
        let now = chrono::Utc::now();
        conn.execute(
            "INSERT INTO workflows (id, blueprint_path, status_id, started_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                id.as_ref(),
                blueprint_path,
                crate::lookup::workflow_status_id(WorkflowStatus::Active),
                now.to_rfc3339(),
            ],
        )?;
        Ok(())
    }
}
