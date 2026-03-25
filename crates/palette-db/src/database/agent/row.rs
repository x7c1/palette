use super::super::id_conversion_error;
use crate::lookup;
use palette_domain::agent::*;
use palette_domain::task::TaskId;
use palette_domain::terminal::TerminalTarget;

pub(super) fn row_to_agent_state(row: &rusqlite::Row) -> rusqlite::Result<AgentState> {
    let role_id: i64 = row.get("role_id")?;
    let status_id: i64 = row.get("status_id")?;
    Ok(AgentState {
        id: AgentId::new(row.get::<_, String>("id")?),
        role: lookup::agent_role_from_id(role_id).map_err(id_conversion_error)?,
        status: lookup::agent_status_from_id(status_id).map_err(id_conversion_error)?,
        supervisor_id: AgentId::new(row.get::<_, String>("supervisor_id")?),
        container_id: ContainerId::new(row.get::<_, String>("container_id")?),
        terminal_target: TerminalTarget::new(row.get::<_, String>("terminal_target")?),
        session_id: row
            .get::<_, Option<String>>("session_id")?
            .map(AgentSessionId::new),
        task_id: TaskId::new(row.get::<_, String>("task_id")?),
    })
}
