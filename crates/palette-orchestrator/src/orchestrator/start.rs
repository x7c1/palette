use super::Orchestrator;
use palette_domain::ServerEvent;
use std::sync::Arc;
use tokio::sync::mpsc;

impl Orchestrator {
    /// Start the event processing loop.
    pub fn start(self: Arc<Self>, mut rx: mpsc::UnboundedReceiver<ServerEvent>) {
        let this = self;
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                Self::handle_event(&this, event).await;
            }
        });
    }
}
