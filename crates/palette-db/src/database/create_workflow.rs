use super::Database;
use crate::error::Error;
use palette_domain::workflow::{WorkflowId, WorkflowStatus};
use rusqlite::params;

impl Database {
    pub fn create_workflow(&self, id: &WorkflowId, blueprint_yaml: &str) -> crate::Result<()> {
        let conn = lock!(self.conn);
        let now = chrono::Utc::now();
        conn.execute(
            "INSERT INTO workflows (id, blueprint_yaml, status, started_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                id.as_ref(),
                blueprint_yaml,
                WorkflowStatus::Active.as_str(),
                now.to_rfc3339(),
            ],
        )?;
        Ok(())
    }
}
