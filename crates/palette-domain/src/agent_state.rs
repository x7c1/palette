use crate::{AgentId, AgentRole, AgentSessionId, AgentStatus, ContainerId, TerminalTarget};

#[derive(Debug, Clone)]
pub struct AgentState {
    pub id: AgentId,
    pub role: AgentRole,
    pub leader_id: AgentId,
    pub container_id: ContainerId,
    pub terminal_target: TerminalTarget,
    pub status: AgentStatus,
    pub session_id: Option<AgentSessionId>,
}
