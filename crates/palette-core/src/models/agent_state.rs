use super::ContainerId;
use super::TmuxTarget;
use palette_domain::{AgentId, AgentRole, AgentStatus};

#[derive(Debug, Clone)]
pub struct AgentState {
    pub id: AgentId,
    pub role: AgentRole,
    pub leader_id: AgentId,
    pub container_id: ContainerId,
    pub tmux_target: TmuxTarget,
    pub status: AgentStatus,
    pub session_id: Option<String>,
}
