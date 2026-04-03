use super::Orchestrator;
use palette_domain::terminal::TerminalSessionName;

impl Orchestrator {
    /// Gracefully shut down the orchestrator:
    /// 1. Stop and remove all worker containers
    /// 2. Remove all worker records from DB
    /// 3. Kill the tmux session
    pub fn shutdown(&self) {
        self.cancel_token.cancel();
        tracing::info!("starting graceful shutdown");

        let workers = match self.interactor.data_store.list_all_workers() {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "failed to list workers for shutdown");
                return;
            }
        };

        for worker in &workers {
            tracing::info!(worker_id = %worker.id, "stopping worker container");
            if let Err(e) = self
                .interactor
                .container
                .stop_container(&worker.container_id)
            {
                tracing::warn!(worker_id = %worker.id, error = %e, "failed to stop container during shutdown");
            }
            if let Err(e) = self
                .interactor
                .container
                .remove_container(&worker.container_id)
            {
                tracing::warn!(worker_id = %worker.id, error = %e, "failed to remove container during shutdown");
            }
            if let Err(e) = self.interactor.data_store.remove_worker(&worker.id) {
                tracing::warn!(worker_id = %worker.id, error = %e, "failed to remove worker from DB during shutdown");
            }
        }

        let session_name = TerminalSessionName::new(&self.session_name);
        if let Err(e) = self.interactor.terminal.kill_session(&session_name) {
            tracing::warn!(error = %e, "failed to kill tmux session during shutdown");
        }

        tracing::info!(
            workers_stopped = workers.len(),
            "graceful shutdown complete"
        );
    }
}
