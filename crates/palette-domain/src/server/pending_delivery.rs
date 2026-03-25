use crate::terminal::TerminalTarget;
use crate::worker::WorkerId;

/// A pending delivery that needs to be attempted.
#[derive(Debug, Clone)]
pub struct PendingDelivery {
    pub target_id: WorkerId,
    pub terminal_target: TerminalTarget,
}
