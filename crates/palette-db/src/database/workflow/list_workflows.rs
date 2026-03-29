use super::super::{Database, lock, parse_datetime};
use palette_domain::workflow::{Workflow, WorkflowId, WorkflowStatus};

impl Database {
    pub fn list_workflows(&self, status: Option<WorkflowStatus>) -> crate::Result<Vec<Workflow>> {
        let conn = lock(&self.conn)?;

        let (sql, param_values): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = match status {
            Some(s) => (
                "SELECT id, blueprint_path, status_id, started_at, blueprint_hash FROM workflows WHERE status_id = ?1",
                vec![Box::new(crate::lookup::workflow_status_id(s))],
            ),
            None => (
                "SELECT id, blueprint_path, status_id, started_at, blueprint_hash FROM workflows",
                vec![],
            ),
        };

        let params: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|b| b.as_ref()).collect();
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params.as_slice(), |row| {
            let status_id: i64 = row.get("status_id")?;
            let status = crate::lookup::workflow_status_from_id(status_id)
                .map_err(super::super::id_conversion_error)?;
            Ok(Workflow {
                id: WorkflowId::new(row.get::<_, String>("id")?),
                blueprint_path: row.get("blueprint_path")?,
                status,
                started_at: parse_datetime(&row.get::<_, String>("started_at")?),
                blueprint_hash: row.get("blueprint_hash")?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}
