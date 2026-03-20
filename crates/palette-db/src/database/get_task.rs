use super::Database;
use crate::error::Error;
use palette_domain::task::{TaskId, TaskRow, TaskStatus};
use palette_domain::workflow::WorkflowId;
use rusqlite::params;

fn row_to_task_row(row: &rusqlite::Row) -> rusqlite::Result<TaskRow> {
    let status_str: String = row.get(5)?;
    let status: TaskStatus = status_str.parse().map_err(|e: String| {
        rusqlite::Error::FromSqlConversionFailure(
            5,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        )
    })?;

    Ok(TaskRow {
        id: TaskId::new(row.get::<_, String>(0)?),
        workflow_id: WorkflowId::new(row.get::<_, String>(1)?),
        parent_id: row.get::<_, Option<String>>(2)?.map(TaskId::new),
        title: row.get(3)?,
        plan_path: row.get(4)?,
        status,
    })
}

impl Database {
    pub fn get_task_row(&self, id: &TaskId) -> crate::Result<Option<TaskRow>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, workflow_id, parent_id, title, plan_path, status FROM tasks WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id.as_ref()], row_to_task_row)?;
        match rows.next() {
            Some(Ok(task)) => Ok(Some(task)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    pub fn get_child_task_rows(&self, parent_id: &TaskId) -> crate::Result<Vec<TaskRow>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, workflow_id, parent_id, title, plan_path, status FROM tasks WHERE parent_id = ?1",
        )?;
        let rows = stmt.query_map(params![parent_id.as_ref()], row_to_task_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}
