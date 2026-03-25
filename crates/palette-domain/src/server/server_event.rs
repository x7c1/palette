use crate::rule::RuleEffect;
use crate::worker::WorkerId;

/// Events emitted by the server for asynchronous processing by the orchestrator.
#[derive(Debug)]
pub enum ServerEvent {
    /// Rule engine produced effects that need orchestrator processing
    /// (auto-assign, destroy member, etc.).
    ProcessEffects { effects: Vec<RuleEffect> },
    /// Deliver queued messages to a specific target.
    DeliverMessages { target_id: WorkerId },
    /// Deliver queued messages to all idle targets.
    NotifyDeliveryLoop,
}
