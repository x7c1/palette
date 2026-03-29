use super::super::id_conversion_error;
use crate::lookup;
use palette_domain::task::TaskId;
use palette_domain::terminal::TerminalTarget;
use palette_domain::worker::*;
use palette_domain::workflow::WorkflowId;

/// Column list for SELECT queries that produce a WorkerState via `row_to_worker_state`.
pub(super) const COLUMNS: &str = "id, workflow_id, role_id, status_id, supervisor_id, container_id, terminal_target, session_id, task_id";

pub(super) fn row_to_worker_state(row: &rusqlite::Row) -> rusqlite::Result<WorkerState> {
    let role_id: i64 = row.get("role_id")?;
    let status_id: i64 = row.get("status_id")?;
    Ok(WorkerState {
        id: WorkerId::new(row.get::<_, String>("id")?),
        workflow_id: WorkflowId::new(row.get::<_, String>("workflow_id")?),
        role: lookup::worker_role_from_id(role_id).map_err(id_conversion_error)?,
        status: lookup::worker_status_from_id(status_id).map_err(id_conversion_error)?,
        supervisor_id: row
            .get::<_, Option<String>>("supervisor_id")?
            .map(WorkerId::new),
        container_id: ContainerId::new(row.get::<_, String>("container_id")?),
        terminal_target: TerminalTarget::new(row.get::<_, String>("terminal_target")?),
        session_id: row
            .get::<_, Option<String>>("session_id")?
            .map(WorkerSessionId::new),
        task_id: TaskId::new(row.get::<_, String>("task_id")?),
    })
}
