use crate::{AgentId, TerminalTarget};

/// A pending delivery that needs to be attempted.
#[derive(Debug, Clone)]
pub struct PendingDelivery {
    pub target_id: AgentId,
    pub terminal_target: TerminalTarget,
}
