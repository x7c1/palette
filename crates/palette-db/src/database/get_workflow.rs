use super::{Database, parse_datetime};
use crate::error::Error;
use palette_domain::workflow::{Workflow, WorkflowId};
use rusqlite::params;

impl Database {
    pub fn get_workflow(&self, id: &WorkflowId) -> crate::Result<Option<Workflow>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, blueprint_path, status_id, started_at FROM workflows WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id.as_ref()], |row| {
            let status_id: i64 = row.get("status_id")?;
            let status = crate::lookup::workflow_status_from_id(status_id)
                .map_err(super::id_conversion_error)?;
            Ok(Workflow {
                id: WorkflowId::new(row.get::<_, String>("id")?),
                blueprint_path: row.get("blueprint_path")?,
                status,
                started_at: parse_datetime(&row.get::<_, String>("started_at")?),
            })
        })?;
        rows.next().transpose().map_err(Into::into)
    }
}
