use super::Orchestrator;
use palette_domain::terminal::TerminalSessionName;

impl Orchestrator {
    /// Gracefully shut down the orchestrator:
    /// 1. Stop and remove all worker containers
    /// 2. Remove all worker records from DB
    /// 3. Kill the tmux session
    pub fn shutdown(&self) {
        tracing::info!("starting graceful shutdown");

        let workers = match self.db.list_all_workers() {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "failed to list workers for shutdown");
                return;
            }
        };

        for worker in &workers {
            tracing::info!(worker_id = %worker.id, "stopping worker container");
            if let Err(e) = self.docker.stop_container(&worker.container_id) {
                tracing::warn!(worker_id = %worker.id, error = %e, "failed to stop container during shutdown");
            }
            if let Err(e) = self.docker.remove_container(&worker.container_id) {
                tracing::warn!(worker_id = %worker.id, error = %e, "failed to remove container during shutdown");
            }
            if let Err(e) = self.db.remove_worker(&worker.id) {
                tracing::warn!(worker_id = %worker.id, error = %e, "failed to remove worker from DB during shutdown");
            }
        }

        let session_name = TerminalSessionName::new(&self.session_name);
        if let Err(e) = self.tmux.kill_session(&session_name) {
            tracing::warn!(error = %e, "failed to kill tmux session during shutdown");
        }

        tracing::info!(
            workers_stopped = workers.len(),
            "graceful shutdown complete"
        );
    }
}
