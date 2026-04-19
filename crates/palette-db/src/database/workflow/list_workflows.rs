use super::super::{Database, lock};
use super::row::{into_workflow, read_workflow_row};
use palette_domain::workflow::{Workflow, WorkflowStatus};

impl Database {
    pub fn list_workflows(&self, status: Option<WorkflowStatus>) -> crate::Result<Vec<Workflow>> {
        let conn = lock(&self.conn)?;

        let (sql, param_values): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = match status {
            Some(s) => (
                "SELECT id, blueprint_path, status_id, started_at, blueprint_hash, failure_reason FROM workflows WHERE status_id = ?1",
                vec![Box::new(crate::lookup::workflow_status_id(s))],
            ),
            None => (
                "SELECT id, blueprint_path, status_id, started_at, blueprint_hash, failure_reason FROM workflows",
                vec![],
            ),
        };

        let params: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|b| b.as_ref()).collect();
        let mut stmt = conn.prepare(sql)?;
        stmt.query_map(params.as_slice(), read_workflow_row)?
            .map(|row| into_workflow(row?))
            .collect::<crate::Result<Vec<_>>>()
    }
}
