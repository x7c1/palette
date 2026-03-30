use crate::models::WorkerRow;
use palette_domain::ReasonKey;
use palette_domain::task::TaskId;
use palette_domain::terminal::TerminalTarget;
use palette_domain::worker::*;
use palette_domain::workflow::WorkflowId;

/// Column list for SELECT queries that produce a WorkerRow.
pub(super) const COLUMNS: &str = "id, workflow_id, role_id, status_id, supervisor_id, container_id, terminal_target, session_id, task_id";

/// Extract a raw DB row into a WorkerRow (DB-native types only).
pub(super) fn read_worker_row(row: &rusqlite::Row) -> rusqlite::Result<WorkerRow> {
    Ok(WorkerRow {
        id: row.get("id")?,
        workflow_id: row.get("workflow_id")?,
        role_id: row.get("role_id")?,
        status_id: row.get("status_id")?,
        supervisor_id: row.get("supervisor_id")?,
        container_id: row.get("container_id")?,
        terminal_target: row.get("terminal_target")?,
        session_id: row.get("session_id")?,
        task_id: row.get("task_id")?,
    })
}

/// Convert a WorkerRow to a domain WorkerState.
pub(super) fn into_worker_state(row: WorkerRow) -> crate::Result<WorkerState> {
    let role = crate::lookup::worker_role_from_id(row.role_id)
        .map_err(|e| crate::Error::Internal(Box::new(e)))?;
    let status = crate::lookup::worker_status_from_id(row.status_id)
        .map_err(|e| crate::Error::Internal(Box::new(e)))?;
    let workflow_id = WorkflowId::parse(row.workflow_id)
        .map_err(|e| crate::Error::Internal(Box::new(e.reason_key())))?;
    let task_id =
        TaskId::parse(row.task_id).map_err(|e| crate::Error::Internal(Box::new(e.reason_key())))?;

    Ok(WorkerState {
        id: WorkerId::new(row.id),
        workflow_id,
        role,
        status,
        supervisor_id: row.supervisor_id.map(WorkerId::new),
        container_id: ContainerId::new(row.container_id),
        terminal_target: TerminalTarget::new(row.terminal_target),
        session_id: row.session_id.map(WorkerSessionId::new),
        task_id,
    })
}
