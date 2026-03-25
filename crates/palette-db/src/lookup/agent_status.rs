use palette_domain::agent::AgentStatus;

pub fn agent_status_id(status: AgentStatus) -> i64 {
    match status {
        AgentStatus::Booting => 1,
        AgentStatus::Working => 2,
        AgentStatus::Idle => 3,
        AgentStatus::WaitingPermission => 4,
        AgentStatus::Crashed => 5,
    }
}

pub fn agent_status_from_id(id: i64) -> Result<AgentStatus, String> {
    match id {
        1 => Ok(AgentStatus::Booting),
        2 => Ok(AgentStatus::Working),
        3 => Ok(AgentStatus::Idle),
        4 => Ok(AgentStatus::WaitingPermission),
        5 => Ok(AgentStatus::Crashed),
        _ => Err(format!("unknown agent_status id: {id}")),
    }
}
