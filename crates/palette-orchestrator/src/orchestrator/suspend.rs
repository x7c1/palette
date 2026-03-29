use super::Orchestrator;
use palette_domain::worker::WorkerStatus;
use palette_domain::workflow::WorkflowStatus;
use std::collections::HashSet;

impl Orchestrator {
    /// Suspend all active workers: stop containers (without removing them),
    /// update worker status to Suspended, and update workflow status.
    pub fn suspend(&self) {
        let workers = match self.interactor.data_store.list_all_workers() {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "suspend: failed to list workers");
                return;
            }
        };

        let suspendable: Vec<_> = workers
            .iter()
            .filter(|w| {
                matches!(
                    w.status,
                    WorkerStatus::Booting
                        | WorkerStatus::Working
                        | WorkerStatus::Idle
                        | WorkerStatus::WaitingPermission
                )
            })
            .collect();

        if suspendable.is_empty() {
            tracing::info!("suspend: no workers to suspend");
            return;
        }

        let workflow_ids: HashSet<_> = suspendable.iter().map(|w| w.workflow_id.clone()).collect();
        let mut suspended_count = 0;

        for worker in &suspendable {
            tracing::info!(worker_id = %worker.id, status = ?worker.status, "suspending worker");

            // Stop the container but do not remove it (will be reused on resume)
            if let Err(e) = self
                .interactor
                .container
                .stop_container(&worker.container_id)
            {
                tracing::warn!(
                    worker_id = %worker.id,
                    error = %e,
                    "failed to stop container during suspend"
                );
                continue;
            }

            // Update worker status to Suspended (session_id is already in DB)
            if let Err(e) = self
                .interactor
                .data_store
                .update_worker_status(&worker.id, WorkerStatus::Suspended)
            {
                tracing::warn!(
                    worker_id = %worker.id,
                    error = %e,
                    "failed to update worker status to Suspended"
                );
                continue;
            }

            suspended_count += 1;
        }

        // Update workflow status to Suspended
        for workflow_id in &workflow_ids {
            if let Err(e) = self
                .interactor
                .data_store
                .update_workflow_status(workflow_id, WorkflowStatus::Suspended)
            {
                tracing::warn!(
                    workflow_id = %workflow_id,
                    error = %e,
                    "failed to update workflow status to Suspended"
                );
            }
        }

        tracing::info!(suspended_count, "suspend complete");
    }
}
