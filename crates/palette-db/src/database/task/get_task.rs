use super::super::{Database, id_conversion_error, lock};
use crate::models::TaskRow;
use palette_domain::task::{TaskId, TaskState, TaskStatus};
use palette_domain::workflow::WorkflowId;
use rusqlite::params;
use std::collections::HashMap;

fn row_to_task_row(row: &rusqlite::Row) -> rusqlite::Result<TaskRow> {
    let status_id: i64 = row.get("status_id")?;
    let status = crate::lookup::task_status_from_id(status_id).map_err(id_conversion_error)?;
    Ok(TaskRow {
        id: TaskId::parse(row.get::<_, String>("id")?)
            .map_err(|e| id_conversion_error(e.reason_key().into()))?,
        workflow_id: WorkflowId::parse(row.get::<_, String>("workflow_id")?)
            .map_err(|e| id_conversion_error(e.reason_key().into()))?,
        status,
    })
}

impl Database {
    pub fn get_task_state(&self, id: &TaskId) -> crate::Result<Option<TaskState>> {
        let conn = lock(&self.conn)?;
        let mut stmt =
            conn.prepare("SELECT id, workflow_id, status_id FROM tasks WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![id.as_ref()], row_to_task_row)?;
        let row = rows.next().transpose()?;
        Ok(row.map(|r| TaskState {
            id: r.id,
            workflow_id: r.workflow_id,
            status: r.status,
        }))
    }

    /// Get all task statuses for a workflow, keyed by TaskId.
    pub fn get_task_statuses(
        &self,
        workflow_id: &WorkflowId,
    ) -> crate::Result<HashMap<TaskId, TaskStatus>> {
        let conn = lock(&self.conn)?;
        let mut stmt =
            conn.prepare("SELECT id, workflow_id, status_id FROM tasks WHERE workflow_id = ?1")?;
        let rows = stmt.query_map(params![workflow_id.as_ref()], row_to_task_row)?;
        let mut map = HashMap::new();
        for row in rows {
            let row = row?;
            map.insert(row.id, row.status);
        }
        Ok(map)
    }
}
