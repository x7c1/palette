use super::Orchestrator;
use palette_domain::job::JobFilter;
use palette_domain::worker::WorkerStatus;
use std::sync::Arc;

impl Orchestrator {
    /// Recover from a previous Orchestrator crash.
    ///
    /// Called during startup when Worker records exist in the DB from a prior
    /// run. Performs health checks, delivers queued messages to idle workers,
    /// and detects Job/Worker state inconsistencies.
    pub fn recover_from_crash(self: &Arc<Self>) {
        let workers = match self.interactor.data_store.list_all_workers() {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "recovery: failed to list workers");
                return;
            }
        };

        if workers.is_empty() {
            return;
        }

        tracing::info!(
            worker_count = workers.len(),
            "recovery: found existing workers, running recovery sequence"
        );

        // Step 1: Check all workers for crashes and trigger recovery
        self.run_health_check();

        // Step 2: Deliver queued messages to idle workers
        self.deliver_to_all_idle();

        // Step 3: Detect Job/Worker state inconsistencies
        self.check_job_worker_consistency();
    }

    /// Detect jobs whose assignee is idle but the job is still in progress.
    fn check_job_worker_consistency(&self) {
        let jobs = match self.interactor.data_store.list_jobs(&JobFilter::default()) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!(error = %e, "recovery: failed to list jobs for consistency check");
                return;
            }
        };

        for job in &jobs {
            if job.status.is_done() {
                continue;
            }

            let assignee_id = match &job.assignee_id {
                Some(id) => id,
                None => continue,
            };

            let worker = match self.interactor.data_store.find_worker(assignee_id) {
                Ok(Some(w)) => w,
                Ok(None) => {
                    tracing::warn!(
                        job_id = %job.id,
                        assignee_id = %assignee_id,
                        job_status = %job.status,
                        "recovery: job assigned to non-existent worker"
                    );
                    continue;
                }
                Err(e) => {
                    tracing::error!(error = %e, job_id = %job.id, "recovery: failed to find worker");
                    continue;
                }
            };

            if worker.status == WorkerStatus::Idle {
                tracing::warn!(
                    job_id = %job.id,
                    assignee_id = %assignee_id,
                    job_status = %job.status,
                    worker_status = ?worker.status,
                    "recovery: job in progress but assignee is idle"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;
    use palette_domain::job::{CraftStatus, JobStatus, JobType};
    use palette_domain::worker::{WorkerId, WorkerRole, WorkerStatus};
    use palette_usecase::Interactor;

    fn make_orchestrator(
        data_store: MockDataStore,
        container: MockContainerRuntime,
        terminal: MockTerminalSession,
    ) -> Arc<Orchestrator> {
        Arc::new(Orchestrator {
            interactor: Arc::new(Interactor {
                container: Box::new(container),
                terminal: Box::new(terminal),
                data_store: Box::new(data_store),
                blueprint: Box::new(MockBlueprintReader),
            }),
            docker_config: crate::DockerConfig {
                palette_url: String::new(),
                leader_image: String::new(),
                member_image: String::new(),
                settings_template: String::new(),
                leader_prompt: String::new(),
                review_integrator_image: String::new(),
                review_integrator_prompt: String::new(),
                crafter_prompt: String::new(),
                reviewer_prompt: String::new(),
                max_workers: 3,
            },
            plan_dir: String::new(),
            session_name: String::new(),
            cancel_token: tokio_util::sync::CancellationToken::new(),
        })
    }

    #[test]
    fn recovery_skipped_when_no_workers() {
        let data_store = MockDataStore::new();
        let container = MockContainerRuntime::new();
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);

        // Should not panic — just returns early
        orch.recover_from_crash();
    }

    #[test]
    fn recovery_delivers_queued_messages_to_idle_worker() {
        let worker = make_worker("m-1", WorkerRole::Member, WorkerStatus::Idle);
        let data_store = MockDataStore::with_workers(vec![worker]);
        data_store
            .messages
            .lock()
            .unwrap()
            .insert(WorkerId::new("m-1"), vec!["do work".to_string()]);

        let container = MockContainerRuntime::with_running(&["container-m-1"]);
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);
        orch.recover_from_crash();

        // Worker should now be Working (message delivered via send_keys)
        let w = orch
            .interactor
            .data_store
            .find_worker(&WorkerId::new("m-1"))
            .unwrap()
            .unwrap();
        assert_eq!(w.status, WorkerStatus::Working);

        // Message queue should be empty
        assert!(
            !orch
                .interactor
                .data_store
                .has_pending_messages(&WorkerId::new("m-1"))
                .unwrap(),
            "message queue should be empty after delivery"
        );
    }

    #[test]
    fn recovery_detects_idle_worker_with_in_progress_job() {
        let worker = make_worker("m-1", WorkerRole::Member, WorkerStatus::Idle);
        let data_store = MockDataStore::with_workers(vec![worker]);

        let mut job = make_job("C-1");
        job.assignee_id = Some(WorkerId::new("m-1"));
        job.status = JobStatus::in_progress(JobType::Craft);
        *data_store.jobs.lock().unwrap() = vec![job];

        let container = MockContainerRuntime::with_running(&["container-m-1"]);
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);

        // Should not panic — logs a warning about the inconsistency
        orch.recover_from_crash();
    }

    #[test]
    fn recovery_detects_job_assigned_to_missing_worker() {
        let data_store = MockDataStore::new();
        // Worker "m-1" does NOT exist in workers list
        *data_store.workers.lock().unwrap() = vec![make_worker(
            "m-2",
            WorkerRole::Member,
            WorkerStatus::Working,
        )];

        let mut job = make_job("C-1");
        job.assignee_id = Some(WorkerId::new("m-1"));
        job.status = JobStatus::in_progress(JobType::Craft);
        *data_store.jobs.lock().unwrap() = vec![job];

        let container = MockContainerRuntime::with_running(&["container-m-2"]);
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);

        // Should not panic — logs a warning about non-existent worker
        orch.recover_from_crash();
    }

    #[test]
    fn recovery_ignores_done_jobs() {
        let worker = make_worker("m-1", WorkerRole::Member, WorkerStatus::Idle);
        let data_store = MockDataStore::with_workers(vec![worker]);

        let mut job = make_job("C-1");
        job.assignee_id = Some(WorkerId::new("m-1"));
        job.status = JobStatus::Craft(CraftStatus::Done);
        *data_store.jobs.lock().unwrap() = vec![job];

        let container = MockContainerRuntime::with_running(&["container-m-1"]);
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);

        // Should not panic — done jobs are skipped
        orch.recover_from_crash();
    }
}
