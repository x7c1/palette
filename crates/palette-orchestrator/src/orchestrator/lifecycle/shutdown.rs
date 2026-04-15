use super::Orchestrator;
use palette_domain::job::JobStatus;
use palette_domain::task::TaskStatus;
use palette_domain::terminal::TerminalSessionName;
use palette_domain::workflow::{WorkflowId, WorkflowStatus};

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

        self.terminate_active_workflows();

        let workers = match self.interactor.data_store.list_all_workers() {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "failed to list workers for shutdown");
                return;
            }
        };

        let worker_ids: Vec<_> = workers.iter().map(|w| w.id.clone()).collect();
        if let Err(e) = self
            .interactor
            .data_store
            .delete_messages_by_targets(&worker_ids)
        {
            tracing::warn!(error = %e, "failed to delete message queues during shutdown");
        }

        for worker in &workers {
            self.destroy_worker(worker);
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

    fn terminate_active_workflows(&self) {
        let workflows = match self.interactor.data_store.list_workflows(None) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "failed to list workflows for shutdown");
                return;
            }
        };
        for wf in workflows {
            if !matches!(
                wf.status,
                WorkflowStatus::Active | WorkflowStatus::Suspending
            ) {
                continue;
            }
            self.terminate_jobs_and_tasks(&wf.id);
            if let Err(e) = self
                .interactor
                .data_store
                .update_workflow_status(&wf.id, WorkflowStatus::Terminated)
            {
                tracing::error!(
                    workflow_id = %wf.id, error = %e,
                    "failed to terminate workflow during shutdown"
                );
            } else {
                tracing::info!(workflow_id = %wf.id, "workflow terminated");
            }
        }
    }

    fn terminate_jobs_and_tasks(&self, workflow_id: &WorkflowId) {
        let tasks = match self.interactor.data_store.get_task_statuses(workflow_id) {
            Ok(t) => t,
            Err(e) => {
                tracing::error!(
                    workflow_id = %workflow_id, error = %e,
                    "failed to list tasks for shutdown"
                );
                return;
            }
        };

        for (task_id, status) in &tasks {
            // Terminate jobs under this task
            if let Ok(Some(job)) = self.interactor.data_store.get_job_by_task_id(task_id)
                && !job.status.is_done()
                && job.status.as_str() != "terminated"
            {
                let terminated = JobStatus::terminated(job.detail.job_type());
                if let Err(e) = self
                    .interactor
                    .data_store
                    .update_job_status(&job.id, terminated)
                {
                    tracing::error!(
                        job_id = %job.id, error = %e,
                        "failed to terminate job during shutdown"
                    );
                }
            }

            // Terminate the task itself
            if !matches!(status, TaskStatus::Completed | TaskStatus::Terminated)
                && let Err(e) = self
                    .interactor
                    .data_store
                    .update_task_status(task_id, TaskStatus::Terminated)
            {
                tracing::error!(
                    task_id = %task_id, error = %e,
                    "failed to terminate task during shutdown"
                );
            }
        }
    }

    fn destroy_worker(&self, worker: &palette_domain::worker::WorkerState) {
        tracing::info!(worker_id = %worker.id, "stopping worker container");
        if let Err(e) = self
            .interactor
            .container
            .stop_container(&worker.container_id)
        {
            tracing::warn!(worker_id = %worker.id, error = %e, "failed to stop container");
        }
        if let Err(e) = self
            .interactor
            .container
            .remove_container(&worker.container_id)
        {
            tracing::warn!(worker_id = %worker.id, error = %e, "failed to remove container");
        }
        if let Err(e) = self.interactor.data_store.remove_worker(&worker.id) {
            tracing::warn!(worker_id = %worker.id, error = %e, "failed to remove worker from DB");
        }
    }
}
