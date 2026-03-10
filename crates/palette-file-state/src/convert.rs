use crate::Error;
use crate::record::{AgentRecord, StateFile};
use palette_domain::agent::{
    AgentId, AgentRole, AgentSessionId, AgentState, AgentStatus, ContainerId,
};
use palette_domain::server::PersistentState;
use palette_domain::terminal::TerminalTarget;

pub fn to_state_file(state: &PersistentState) -> StateFile {
    StateFile {
        session_name: state.session_name.clone(),
        leaders: state.leaders.iter().map(to_agent_record).collect(),
        members: state.members.iter().map(to_agent_record).collect(),
        created_at: state.created_at.to_rfc3339(),
        updated_at: state.updated_at.to_rfc3339(),
    }
}

pub fn from_state_file(file: StateFile) -> Result<PersistentState, Error> {
    let leaders = file
        .leaders
        .into_iter()
        .map(from_agent_record)
        .collect::<Result<Vec<_>, _>>()?;
    let members = file
        .members
        .into_iter()
        .map(from_agent_record)
        .collect::<Result<Vec<_>, _>>()?;

    let created_at = parse_datetime(&file.created_at)?;
    let updated_at = parse_datetime(&file.updated_at)?;

    Ok(PersistentState {
        session_name: file.session_name,
        leaders,
        members,
        created_at,
        updated_at,
    })
}

fn to_agent_record(agent: &AgentState) -> AgentRecord {
    AgentRecord {
        id: agent.id.as_ref().to_string(),
        role: agent.role.as_str().to_string(),
        leader_id: agent.leader_id.as_ref().to_string(),
        container_id: agent.container_id.as_ref().to_string(),
        terminal_target: agent.terminal_target.as_ref().to_string(),
        status: status_to_str(agent.status),
        session_id: agent.session_id.as_ref().map(|s| s.to_string()),
    }
}

fn from_agent_record(record: AgentRecord) -> Result<AgentState, Error> {
    Ok(AgentState {
        id: AgentId::new(record.id),
        role: parse_role(&record.role)?,
        leader_id: AgentId::new(record.leader_id),
        container_id: ContainerId::new(record.container_id),
        terminal_target: TerminalTarget::new(record.terminal_target),
        status: parse_status(&record.status)?,
        session_id: record.session_id.map(AgentSessionId::new),
    })
}

fn parse_role(s: &str) -> Result<AgentRole, Error> {
    match s {
        "leader" => Ok(AgentRole::Leader),
        "member" => Ok(AgentRole::Member),
        other => Err(Error::InvalidData(format!("unknown role: {other}"))),
    }
}

fn status_to_str(status: AgentStatus) -> String {
    match status {
        AgentStatus::Booting => "booting",
        AgentStatus::Working => "working",
        AgentStatus::Idle => "idle",
        AgentStatus::WaitingPermission => "waiting_permission",
        AgentStatus::Crashed => "crashed",
    }
    .to_string()
}

fn parse_status(s: &str) -> Result<AgentStatus, Error> {
    match s {
        "booting" => Ok(AgentStatus::Booting),
        "working" => Ok(AgentStatus::Working),
        "idle" => Ok(AgentStatus::Idle),
        "waiting_permission" => Ok(AgentStatus::WaitingPermission),
        "crashed" => Ok(AgentStatus::Crashed),
        other => Err(Error::InvalidData(format!("unknown status: {other}"))),
    }
}

fn parse_datetime(s: &str) -> Result<chrono::DateTime<chrono::Utc>, Error> {
    s.parse()
        .map_err(|_| Error::InvalidData(format!("invalid datetime: {s}")))
}
