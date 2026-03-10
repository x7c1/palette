use super::Orchestrator;
use palette_domain::server::ServerEvent;
use std::sync::Arc;
use tokio::sync::mpsc;

impl Orchestrator {
    /// Start the event processing loop.
    pub fn start(self: Arc<Self>, mut rx: mpsc::UnboundedReceiver<ServerEvent>) {
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                self.handle_event(event).await;
            }
        });
    }
}
