use super::super::{Database, lock};
use crate::models::TaskRow;
use palette_domain::task::{TaskId, TaskState, TaskStatus};
use palette_domain::workflow::WorkflowId;
use rusqlite::params;
use std::collections::HashMap;

fn read_task_row(row: &rusqlite::Row) -> rusqlite::Result<TaskRow> {
    Ok(TaskRow {
        id: row.get("id")?,
        workflow_id: row.get("workflow_id")?,
        status_id: row.get("status_id")?,
    })
}

fn into_task_state(row: TaskRow) -> crate::Result<TaskState> {
    let status = crate::lookup::task_status_from_id(row.status_id)
        .map_err(|e| crate::Error::Internal(Box::new(e)))?;
    let id = TaskId::parse(row.id).map_err(|e| crate::Error::Internal(Box::new(e.reason_key())))?;
    let workflow_id = WorkflowId::parse(row.workflow_id)
        .map_err(|e| crate::Error::Internal(Box::new(e.reason_key())))?;

    Ok(TaskState {
        id,
        workflow_id,
        status,
    })
}

impl Database {
    pub fn get_task_state(&self, id: &TaskId) -> crate::Result<Option<TaskState>> {
        let conn = lock(&self.conn)?;
        let mut stmt =
            conn.prepare("SELECT id, workflow_id, status_id FROM tasks WHERE id = ?1")?;
        stmt.query_map(params![id.as_ref()], read_task_row)?
            .next()
            .transpose()?
            .map(into_task_state)
            .transpose()
    }

    /// Get all task statuses for a workflow, keyed by TaskId.
    pub fn get_task_statuses(
        &self,
        workflow_id: &WorkflowId,
    ) -> crate::Result<HashMap<TaskId, TaskStatus>> {
        let conn = lock(&self.conn)?;
        let mut stmt =
            conn.prepare("SELECT id, workflow_id, status_id FROM tasks WHERE workflow_id = ?1")?;
        stmt.query_map(params![workflow_id.as_ref()], read_task_row)?
            .map(|row| {
                let state = into_task_state(row?)?;
                Ok((state.id, state.status))
            })
            .collect::<crate::Result<HashMap<_, _>>>()
    }
}
