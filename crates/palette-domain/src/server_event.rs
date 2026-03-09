use crate::agent_id::AgentId;
use crate::rule_effect::RuleEffect;

/// Events emitted by the server for asynchronous processing by the orchestrator.
#[derive(Debug)]
pub enum ServerEvent {
    /// Rule engine produced effects that need orchestrator processing
    /// (auto-assign, destroy member, etc.).
    ProcessEffects { effects: Vec<RuleEffect> },
    /// Deliver queued messages to a specific target.
    DeliverMessages { target_id: AgentId },
    /// Deliver queued messages to all idle targets.
    NotifyDeliveryLoop,
}
