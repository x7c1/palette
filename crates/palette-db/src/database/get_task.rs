use super::{Database, parse_column};
use crate::error::Error;
use crate::models::TaskRow;
use palette_domain::task::TaskId;
use palette_domain::workflow::WorkflowId;
use rusqlite::params;

fn row_to_task_row(row: &rusqlite::Row) -> rusqlite::Result<TaskRow> {
    Ok(TaskRow {
        id: TaskId::new(row.get::<_, String>("id")?),
        workflow_id: WorkflowId::new(row.get::<_, String>("workflow_id")?),
        parent_id: row.get::<_, Option<String>>("parent_id")?.map(TaskId::new),
        title: row.get("title")?,
        plan_path: row.get("plan_path")?,
        job_type: row
            .get::<_, Option<String>>("job_type")?
            .and_then(|s| s.parse().ok()),
        status: parse_column(row, "status")?,
    })
}

impl Database {
    pub fn get_task_row(&self, id: &TaskId) -> crate::Result<Option<TaskRow>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, workflow_id, parent_id, title, plan_path, job_type, status FROM tasks WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id.as_ref()], row_to_task_row)?;
        rows.next().transpose().map_err(Into::into)
    }

    pub fn get_child_task_rows(&self, parent_id: &TaskId) -> crate::Result<Vec<TaskRow>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, workflow_id, parent_id, title, plan_path, job_type, status FROM tasks WHERE parent_id = ?1",
        )?;
        let rows = stmt.query_map(params![parent_id.as_ref()], row_to_task_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}
