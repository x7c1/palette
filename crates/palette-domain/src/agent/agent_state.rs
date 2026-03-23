use super::{AgentId, AgentRole, AgentSessionId, AgentStatus, ContainerId};
use crate::task::TaskId;
use crate::terminal::TerminalTarget;

#[derive(Debug, Clone)]
pub struct AgentState {
    pub id: AgentId,
    pub role: AgentRole,
    pub supervisor_id: AgentId,
    pub container_id: ContainerId,
    pub terminal_target: TerminalTarget,
    pub status: AgentStatus,
    pub session_id: Option<AgentSessionId>,
    pub task_id: TaskId,
}
