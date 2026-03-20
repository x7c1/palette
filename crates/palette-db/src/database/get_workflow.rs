use super::{Database, parse_datetime};
use crate::error::Error;
use palette_domain::workflow::{Workflow, WorkflowId, WorkflowStatus};
use rusqlite::params;

impl Database {
    pub fn get_workflow(&self, id: &WorkflowId) -> crate::Result<Option<Workflow>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, blueprint_path, status, started_at FROM workflows WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id.as_ref()], |row| {
            let status_str: String = row.get("status")?;
            Ok(Workflow {
                id: WorkflowId::new(row.get::<_, String>("id")?),
                blueprint_path: row.get("blueprint_path")?,
                status: status_str.parse().unwrap_or(WorkflowStatus::Active),
                started_at: parse_datetime(&row.get::<_, String>("started_at")?),
            })
        })?;
        rows.next().transpose().map_err(Into::into)
    }
}
