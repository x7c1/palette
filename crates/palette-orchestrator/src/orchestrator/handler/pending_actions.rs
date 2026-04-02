use palette_domain::worker::WorkerId;

/// Follow-up actions that the orchestrator event loop should perform
/// after an event handler completes.
///
/// Returned by handler methods; dispatched by `dispatch_pending_actions`.
pub(crate) struct PendingActions {
    /// Workers to watch for readiness and deliver queued messages to.
    pub deliver_to: Vec<WorkerId>,
    /// Workers to watch for readiness only (no immediate messages).
    pub watch_only: Vec<WorkerId>,
}

impl PendingActions {
    pub fn new() -> Self {
        Self {
            deliver_to: Vec::new(),
            watch_only: Vec::new(),
        }
    }

    /// Combine two results into one.
    pub fn merge(mut self, other: PendingActions) -> Self {
        self.deliver_to.extend(other.deliver_to);
        self.watch_only.extend(other.watch_only);
        self
    }
}
