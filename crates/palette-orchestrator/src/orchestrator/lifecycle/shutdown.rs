use super::Orchestrator;
use palette_domain::terminal::TerminalSessionName;
use palette_domain::workflow::WorkflowStatus;

impl Orchestrator {
    /// Gracefully shut down the orchestrator:
    /// 1. Terminate active workflows
    /// 2. Delete message queues for all workers
    /// 3. Stop and remove all worker containers
    /// 4. Remove all worker records from DB
    /// 5. Kill the tmux session
    pub fn shutdown(&self) {
        self.cancel_token.cancel();
        tracing::info!("starting graceful shutdown");

        // Terminate active workflows
        match self.interactor.data_store.list_workflows(None) {
            Ok(workflows) => {
                for wf in workflows {
                    if matches!(
                        wf.status,
                        WorkflowStatus::Active | WorkflowStatus::Suspending
                    ) {
                        if let Err(e) = self
                            .interactor
                            .data_store
                            .update_workflow_status(&wf.id, WorkflowStatus::Terminated)
                        {
                            tracing::warn!(
                                workflow_id = %wf.id,
                                error = %e,
                                "failed to terminate workflow during shutdown"
                            );
                        } else {
                            tracing::info!(workflow_id = %wf.id, "workflow terminated");
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to list workflows for shutdown");
            }
        }

        let workers = match self.interactor.data_store.list_all_workers() {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "failed to list workers for shutdown");
                return;
            }
        };

        // Delete message queues
        let worker_ids: Vec<_> = workers.iter().map(|w| w.id.clone()).collect();
        if let Err(e) = self
            .interactor
            .data_store
            .delete_messages_by_targets(&worker_ids)
        {
            tracing::warn!(error = %e, "failed to delete message queues during shutdown");
        }

        // Stop and remove worker containers
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
