use super::TmuxTarget;
use palette_db::AgentId;

/// A pending delivery that needs to be attempted.
#[derive(Debug, Clone)]
pub struct PendingDelivery {
    pub target_id: AgentId,
    pub tmux_target: TmuxTarget,
}
