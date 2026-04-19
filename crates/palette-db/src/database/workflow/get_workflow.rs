use super::super::{Database, lock};
use super::row::{into_workflow, read_workflow_row};
use palette_domain::workflow::{Workflow, WorkflowId};
use rusqlite::params;

impl Database {
    pub fn get_workflow(&self, id: &WorkflowId) -> crate::Result<Option<Workflow>> {
        let conn = lock(&self.conn)?;
        let mut stmt = conn.prepare(
            "SELECT id, blueprint_path, status_id, started_at, blueprint_hash, failure_reason FROM workflows WHERE id = ?1",
        )?;
        stmt.query_map(params![id.as_ref()], read_workflow_row)?
            .next()
            .transpose()?
            .map(into_workflow)
            .transpose()
    }
}
