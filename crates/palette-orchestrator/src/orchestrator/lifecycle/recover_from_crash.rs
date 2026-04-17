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
        let inconsistencies = self.check_job_worker_consistency();
        if inconsistencies > 0 {
            tracing::warn!(
                count = inconsistencies,
                "recovery: detected job/worker state inconsistencies"
            );
        }
    }

    /// Detect jobs whose assignee is idle or missing while the job is not done.
    ///
    /// Returns the number of inconsistencies found.
    fn check_job_worker_consistency(&self) -> usize {
        let jobs = match self.interactor.data_store.list_jobs(&JobFilter::default()) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!(error = %e, "recovery: failed to list jobs for consistency check");
                return 0;
            }
        };

        jobs.iter()
            .filter(|job| !job.status.is_done())
            .filter_map(|job| {
                let assignee_id = job.assignee_id.as_ref()?;
                Some((job, assignee_id))
            })
            .filter(|(job, assignee_id)| {
                match self.interactor.data_store.find_worker(assignee_id) {
                    Ok(None) => {
                        tracing::warn!(
                            job_id = %job.id,
                            assignee_id = %assignee_id,
                            job_status = %job.status,
                            "recovery: job assigned to non-existent worker"
                        );
                        true
                    }
                    Ok(Some(w)) if w.status == WorkerStatus::Idle => {
                        tracing::warn!(
                            job_id = %job.id,
                            assignee_id = %assignee_id,
                            job_status = %job.status,
                            worker_status = ?w.status,
                            "recovery: job in progress but assignee is idle"
                        );
                        true
                    }
                    Ok(Some(_)) => false,
                    // DB error means we can't determine consistency, not that
                    // an inconsistency exists. Logged as error separately.
                    Err(e) => {
                        tracing::error!(error = %e, job_id = %job.id, "recovery: failed to find worker");
                        false
                    }
                }
            })
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;
    use palette_domain::job::{CraftStatus, JobStatus, JobType};
    use palette_domain::worker::{WorkerId, WorkerRole, WorkerStatus};
    use palette_usecase::Interactor;
    use std::collections::HashMap;
    use tokio_util::sync::CancellationToken;

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
                github_review_port: Box::new(MockGitHubReview),
            }),
            docker_config: crate::DockerConfig {
                worker_callback_url: String::new(),
                callback_network: crate::CallbackNetwork::Auto,
                approver_image: String::new(),
                member_image: String::new(),
                settings_template: String::new(),
                approver_prompt: String::new(),
                review_integrator_image: String::new(),
                review_integrator_prompt: String::new(),
                crafter_prompt: String::new(),
                reviewer_prompt: String::new(),
                max_workers: 3,
            },
            plan_dir: std::path::PathBuf::new(),
            session_name: String::new(),
            cancel_token: CancellationToken::new(),
            workspace_manager: crate::orchestrator::infra::workspace::WorkspaceManager::new("data"),
            perspectives: crate::ValidatedPerspectives {
                dirs: HashMap::new(),
                perspectives: vec![],
            },
            event_tx: tokio::sync::mpsc::unbounded_channel().0,
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
            .insert(WorkerId::parse("m-1").unwrap(), vec!["do work".to_string()]);

        let container = MockContainerRuntime::with_running(&["container-m-1"]);
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);
        orch.recover_from_crash();

        // Worker should now be Working (message delivered via send_keys)
        let w = orch
            .interactor
            .data_store
            .find_worker(&WorkerId::parse("m-1").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(w.status, WorkerStatus::Working);

        // Message queue should be empty
        assert!(
            !orch
                .interactor
                .data_store
                .has_pending_messages(&WorkerId::parse("m-1").unwrap())
                .unwrap(),
            "message queue should be empty after delivery"
        );
    }

    #[test]
    fn consistency_detects_idle_worker_with_in_progress_job() {
        let worker = make_worker("m-1", WorkerRole::Member, WorkerStatus::Idle);
        let data_store = MockDataStore::with_workers(vec![worker]);

        let mut job = make_job("C-1");
        job.assignee_id = Some(WorkerId::parse("m-1").unwrap());
        job.status = JobStatus::in_progress(JobType::Craft);
        *data_store.jobs.lock().unwrap() = vec![job];

        let container = MockContainerRuntime::with_running(&["container-m-1"]);
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);
        assert_eq!(orch.check_job_worker_consistency(), 1);
    }

    #[test]
    fn consistency_detects_job_assigned_to_missing_worker() {
        let data_store = MockDataStore::new();
        // Worker "m-1" does NOT exist in workers list
        *data_store.workers.lock().unwrap() = vec![make_worker(
            "m-2",
            WorkerRole::Member,
            WorkerStatus::Working,
        )];

        let mut job = make_job("C-1");
        job.assignee_id = Some(WorkerId::parse("m-1").unwrap());
        job.status = JobStatus::in_progress(JobType::Craft);
        *data_store.jobs.lock().unwrap() = vec![job];

        let container = MockContainerRuntime::with_running(&["container-m-2"]);
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);
        assert_eq!(orch.check_job_worker_consistency(), 1);
    }

    #[test]
    fn consistency_ignores_done_jobs() {
        let worker = make_worker("m-1", WorkerRole::Member, WorkerStatus::Idle);
        let data_store = MockDataStore::with_workers(vec![worker]);

        let mut job = make_job("C-1");
        job.assignee_id = Some(WorkerId::parse("m-1").unwrap());
        job.status = JobStatus::Craft(CraftStatus::Done);
        *data_store.jobs.lock().unwrap() = vec![job];

        let container = MockContainerRuntime::with_running(&["container-m-1"]);
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);
        assert_eq!(orch.check_job_worker_consistency(), 0);
    }

    #[test]
    fn consistency_ignores_working_worker() {
        let worker = make_worker("m-1", WorkerRole::Member, WorkerStatus::Working);
        let data_store = MockDataStore::with_workers(vec![worker]);

        let mut job = make_job("C-1");
        job.assignee_id = Some(WorkerId::parse("m-1").unwrap());
        job.status = JobStatus::in_progress(JobType::Craft);
        *data_store.jobs.lock().unwrap() = vec![job];

        let container = MockContainerRuntime::with_running(&["container-m-1"]);
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);
        assert_eq!(orch.check_job_worker_consistency(), 0);
    }
}
