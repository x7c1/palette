use super::Database;
use crate::error::Error;
use palette_domain::workflow::{Workflow, WorkflowId, WorkflowStatus};
use rusqlite::params;

impl Database {
    pub fn create_workflow(
        &self,
        id: &WorkflowId,
        blueprint_path: &str,
    ) -> crate::Result<Workflow> {
        let conn = lock!(self.conn);
        let now = chrono::Utc::now();
        conn.execute(
            "INSERT INTO workflows (id, blueprint_path, status, started_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                id.as_ref(),
                blueprint_path,
                WorkflowStatus::Active.as_str(),
                now.to_rfc3339(),
            ],
        )?;
        Ok(Workflow {
            id: id.clone(),
            blueprint_path: blueprint_path.to_string(),
            status: WorkflowStatus::Active,
            started_at: now,
        })
    }
}
