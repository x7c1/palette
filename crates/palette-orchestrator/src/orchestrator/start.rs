use super::Orchestrator;
use palette_domain::server::ServerEvent;
use std::sync::Arc;
use tokio::sync::mpsc;

impl Orchestrator {
    /// Start the event processing loop.
    ///
    /// The loop exits when `shutdown_rx` fires or the event channel closes.
    /// Graceful shutdown (container cleanup, tmux kill) runs only when the
    /// shutdown signal is received, not when the channel simply closes.
    pub fn start(
        self: Arc<Self>,
        mut rx: mpsc::UnboundedReceiver<ServerEvent>,
        mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        tokio::spawn(async move {
            let should_shutdown = loop {
                tokio::select! {
                    event = rx.recv() => {
                        match event {
                            Some(e) => self.handle_event(e).await,
                            None => break false,
                        }
                    }
                    _ = &mut shutdown_rx => {
                        tracing::info!("shutdown signal received, stopping event loop");
                        break true;
                    }
                }
            };
            if should_shutdown {
                self.shutdown();
            }
        });
    }
}
