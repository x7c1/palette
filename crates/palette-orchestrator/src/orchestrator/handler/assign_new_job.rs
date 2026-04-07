use super::Orchestrator;
use super::PendingActions;
use super::job_instruction::format_job_instruction;
use palette_domain::job::{Job, JobDetail, JobId, JobType};
use palette_domain::worker::WorkerId;
use palette_usecase::container_runtime::{ArtifactsMount, WorkspaceVolume};
use palette_usecase::data_store::InsertWorkerRequest;

impl Orchestrator {
    /// Assign a new job to a freshly spawned member.
    /// Skipped when the workflow is suspending (no new members during suspend).
    pub(crate) fn assign_new_job(&self, job_id: &JobId) -> crate::Result<PendingActions> {
        let mut result = PendingActions::new();

        // Verify the job is assignable (todo + no assignee)
        let assignable_jobs = self.interactor.data_store.find_assignable_jobs()?;
        let job = match assignable_jobs.iter().find(|j| j.id == *job_id) {
            Some(j) => j.clone(),
            None => return Ok(result),
        };

        // Don't spawn new members while a suspend is in progress
        let task_state = self
            .interactor
            .data_store
            .get_task_state(&job.task_id)?
            .ok_or_else(|| crate::Error::TaskNotFound {
                task_id: job.task_id.clone(),
            })?;
        if self.is_workflow_suspending(&task_state.workflow_id)? {
            tracing::warn!(job_id = %job_id, "suspend in progress, deferring job assignment");
            return Ok(result);
        }
        let active = self.interactor.data_store.count_active_workers()?;
        if active >= self.docker_config.max_workers {
            tracing::warn!(
                job_id = %job_id,
                active = active,
                max = self.docker_config.max_workers,
                "max workers reached, job waits"
            );
            return Ok(result);
        }

        let job_type = job.detail.job_type();

        // Mechanized jobs (Orchestrator/Operator) don't spawn worker containers
        if !job_type.needs_worker() {
            self.handle_mechanized_job(&job)?;
            return Ok(result);
        }

        // Determine workspace volume based on job type
        let workspace = self.resolve_workspace(&job)?;

        // Determine artifacts mount for review jobs
        let artifacts_dir = self.resolve_artifacts_mount(&job)?;

        // Spawn a new member with supervisor from the task tree
        let supervisor_id = self.find_supervisor_for_job(&job.task_id)?;
        let seq = self
            .interactor
            .data_store
            .increment_worker_counter(&task_state.workflow_id)?;
        let member_id = WorkerId::next_member(seq);
        let member = self.spawn_member(
            &member_id,
            job_type,
            &job.detail,
            &supervisor_id,
            &job.task_id,
            workspace,
            artifacts_dir,
        )?;
        // Register in DB
        self.interactor
            .data_store
            .insert_worker(&InsertWorkerRequest {
                id: member.id.clone(),
                workflow_id: member.workflow_id.clone(),
                role: member.role,
                status: member.status,
                supervisor_id: member.supervisor_id.clone(),
                container_id: member.container_id.clone(),
                terminal_target: member.terminal_target.clone(),
                session_id: member.session_id.clone(),
                task_id: member.task_id.clone(),
            })?;

        // Assign job
        self.interactor
            .data_store
            .assign_job(job_id, &member_id, job_type)?;
        tracing::info!(
            job_id = %job_id,
            member_id = %member_id,
            "auto-assigned job"
        );

        // Build job instruction message
        let round = if job_type == JobType::Review {
            Some(self.current_review_round(&job)?)
        } else {
            None
        };
        let instruction = format_job_instruction(&job, round, &self.perspectives);
        self.interactor
            .data_store
            .enqueue_message(&member_id, &instruction)?;

        result.deliver_to.push(member_id);

        Ok(result)
    }

    /// Walk up the task tree to find the nearest supervisor for a job's task.
    fn find_supervisor_for_job(
        &self,
        task_id: &palette_domain::task::TaskId,
    ) -> crate::Result<WorkerId> {
        let task_state = self
            .interactor
            .data_store
            .get_task_state(task_id)?
            .ok_or_else(|| crate::Error::TaskNotFound {
                task_id: task_id.clone(),
            })?;
        let task_store = self.interactor.create_task_store(&task_state.workflow_id)?;

        let mut current_id = task_id.clone();
        loop {
            if let Ok(Some(sup)) = self
                .interactor
                .data_store
                .find_supervisor_for_task(&current_id)
            {
                return Ok(sup.id.clone());
            }
            let task =
                task_store
                    .get_task(&current_id)
                    .ok_or_else(|| crate::Error::TaskNotFound {
                        task_id: current_id.clone(),
                    })?;
            match task.parent_id {
                Some(ref pid) => current_id = pid.clone(),
                None => break,
            }
        }
        Err(crate::Error::InvalidTaskState {
            task_id: task_id.clone(),
            detail: "no supervisor found in task ancestry".into(),
        })
    }

    /// Handle a mechanized job (Orchestrator or Operator).
    /// These jobs don't spawn worker containers.
    fn handle_mechanized_job(&self, job: &Job) -> crate::Result<()> {
        match &job.detail {
            JobDetail::Orchestrator { command } => {
                tracing::info!(job_id = %job.id, command = ?command, "executing orchestrator task");
                self.execute_orchestrator_task(job, &self.event_tx);
            }
            JobDetail::Operator => {
                self.interactor.data_store.update_job_status(
                    &job.id,
                    palette_domain::job::JobStatus::Operator(
                        palette_domain::job::MechanizedStatus::InProgress,
                    ),
                )?;
                tracing::info!(job_id = %job.id, "operator task waiting for human input");
            }
            _ => {}
        }
        Ok(())
    }

    /// Determine the artifacts mount for a job.
    ///
    /// Review jobs get a read-write mount of the artifacts directory.
    /// Craft jobs get a read-only mount (to read review feedback).
    fn resolve_artifacts_mount(&self, job: &Job) -> crate::Result<Option<ArtifactsMount>> {
        let job_type = job.detail.job_type();
        let (workflow_id, anchor_job_id) = match job_type {
            JobType::Craft => {
                let Some(task_state) = self.interactor.data_store.get_task_state(&job.task_id)?
                else {
                    return Ok(None);
                };
                (task_state.workflow_id, job.id.clone())
            }
            JobType::ReviewIntegrate | JobType::Orchestrator | JobType::Operator => {
                return Ok(None);
            }
            JobType::Review => {
                let Some(task_state) = self.interactor.data_store.get_task_state(&job.task_id)?
                else {
                    return Ok(None);
                };
                let task_store = self.interactor.create_task_store(&task_state.workflow_id)?;
                let anchor_job = match self.find_artifact_anchor(&task_store, &job.task_id) {
                    Some(j) => j,
                    None => return Ok(None),
                };
                (task_state.workflow_id, anchor_job.id)
            }
        };

        let artifacts_path = self
            .workspace_manager
            .artifacts_path(workflow_id.as_ref(), anchor_job_id.as_ref());
        std::fs::create_dir_all(&artifacts_path)
            .map_err(|e| crate::Error::External(Box::new(e)))?;

        // For review jobs, pre-create the round and reviewer subdirectories
        // so the container (which may run as a different user) can write there.
        if job_type == JobType::Review {
            let round = self.current_review_round(job)?;
            let reviewer_dir = artifacts_path
                .join(format!("round-{round}"))
                .join(job.id.to_string());
            std::fs::create_dir_all(&reviewer_dir)
                .map_err(|e| crate::Error::External(Box::new(e)))?;
        }

        let abs_path = std::fs::canonicalize(&artifacts_path)
            .map_err(|e| crate::Error::External(Box::new(e)))?;

        Ok(Some(ArtifactsMount {
            host_path: abs_path.to_string_lossy().to_string(),
            read_only: job_type == JobType::Craft,
        }))
    }

    /// Get the current review round for a review job.
    pub(crate) fn current_review_round(&self, job: &Job) -> crate::Result<u32> {
        let submissions = self.interactor.data_store.get_review_submissions(&job.id)?;
        Ok(submissions.len() as u32 + 1)
    }

    /// Determine the workspace volume for a job.
    ///
    /// Craft jobs get a new workspace via `git clone --shared` from the bare cache.
    /// Review jobs share the parent craft job's workspace as read-only.
    fn resolve_workspace(&self, job: &Job) -> crate::Result<Option<WorkspaceVolume>> {
        match &job.detail {
            JobDetail::ReviewIntegrate { .. }
            | JobDetail::Orchestrator { .. }
            | JobDetail::Operator => Ok(None),
            JobDetail::Craft { repository } => {
                let info = self
                    .workspace_manager
                    .create_workspace(job.id.as_ref(), repository)?;
                Ok(Some(WorkspaceVolume {
                    host_path: info.host_path,
                    repo_cache_path: info.repo_cache_path,
                    read_only: false,
                }))
            }
            JobDetail::Review { target, .. } => {
                if let Some(pr) = target.pull_request() {
                    // Standalone PR review: clone the PR repository
                    let repo_name = format!("{}/{}", pr.owner, pr.repo);
                    let repository = palette_domain::job::Repository::parse(&repo_name, "main")
                        .map_err(|e| crate::Error::InvalidTaskState {
                            task_id: job.task_id.clone(),
                            detail: format!("invalid PR repository: {e:?}"),
                        })?;
                    let info = self
                        .workspace_manager
                        .create_workspace(job.id.as_ref(), &repository)?;
                    Ok(Some(WorkspaceVolume {
                        host_path: info.host_path,
                        repo_cache_path: info.repo_cache_path,
                        read_only: true,
                    }))
                } else {
                    // Craft-parented review: share the crafter's workspace read-only
                    let task_id = &job.task_id;
                    let Some(task_state) = self.interactor.data_store.get_task_state(task_id)?
                    else {
                        return Ok(None);
                    };
                    let task_store = self.interactor.create_task_store(&task_state.workflow_id)?;
                    let craft_job = match self.find_ancestor_craft_job(&task_store, task_id) {
                        Some(j) => j,
                        None => return Ok(None),
                    };
                    let JobDetail::Craft { ref repository } = craft_job.detail else {
                        return Ok(None);
                    };
                    let cache_path = self.workspace_manager.repo_cache_path(repository);
                    let ws_path = self.workspace_manager.workspace_path(craft_job.id.as_ref());
                    let cache_abs = std::fs::canonicalize(&cache_path)
                        .map_err(|e| crate::Error::External(Box::new(e)))?;
                    let ws_abs = std::fs::canonicalize(&ws_path)
                        .map_err(|e| crate::Error::External(Box::new(e)))?;
                    Ok(Some(WorkspaceVolume {
                        host_path: ws_abs.to_string_lossy().to_string(),
                        repo_cache_path: cache_abs.to_string_lossy().to_string(),
                        read_only: true,
                    }))
                }
            }
        }
    }
}
