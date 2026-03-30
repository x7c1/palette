use super::super::{Database, lock, parse_datetime};
use palette_domain::workflow::{Workflow, WorkflowId};
use rusqlite::params;

impl Database {
    pub fn get_workflow(&self, id: &WorkflowId) -> crate::Result<Option<Workflow>> {
        let conn = lock(&self.conn)?;
        let mut stmt = conn.prepare(
            "SELECT id, blueprint_path, status_id, started_at, blueprint_hash FROM workflows WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id.as_ref()], |row| {
            let status_id: i64 = row.get("status_id")?;
            let status = crate::lookup::workflow_status_from_id(status_id)
                .map_err(super::super::id_conversion_error)?;
            Ok(Workflow {
                id: WorkflowId::parse(row.get::<_, String>("id")?)
                    .map_err(|e| super::super::id_conversion_error(e.reason_key()))?,
                blueprint_path: row.get("blueprint_path")?,
                status,
                started_at: parse_datetime(&row.get::<_, String>("started_at")?),
                blueprint_hash: row.get("blueprint_hash")?,
            })
        })?;
        rows.next().transpose().map_err(Into::into)
    }
}
