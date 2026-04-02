use super::Orchestrator;
use palette_domain::task::TaskId;
use palette_domain::worker::{ContainerId, WorkerId, WorkerRole, WorkerStatus};
use palette_usecase::container_runtime::{ArtifactsMount, ContainerMounts};
use palette_usecase::data_store::InsertWorkerRequest;

impl Orchestrator {
    /// Spawn a dynamic supervisor for a composite task.
    /// Creates a tmux window and Docker container, then registers in DB.
    /// If Docker fails, the supervisor is still registered with an empty container_id.
    pub(in crate::orchestrator) fn handle_spawn_supervisor(
        &self,
        task_id: &TaskId,
        role: WorkerRole,
    ) -> crate::Result<WorkerId> {
        let task_state = self
            .interactor
            .data_store
            .get_task_state(task_id)?
            .ok_or_else(|| crate::Error::TaskNotFound {
                task_id: task_id.clone(),
            })?;
        let seq = self
            .interactor
            .data_store
            .increment_worker_counter(&task_state.workflow_id)?;
        let sup_id = WorkerId::next_supervisor(seq, role);

        // Create a tmux window for this supervisor
        let sup_name = sup_id.as_ref();
        let terminal_target = self.interactor.terminal.create_target(sup_name)?;

        // Select Docker image and prompt based on role
        let (image, prompt_path) = match role {
            WorkerRole::Approver => (
                &self.docker_config.approver_image,
                &self.docker_config.approver_prompt,
            ),
            WorkerRole::ReviewIntegrator => (
                &self.docker_config.review_integrator_image,
                &self.docker_config.review_integrator_prompt,
            ),
            WorkerRole::Member => {
                return Err(crate::Error::InvalidTaskState {
                    task_id: task_id.clone(),
                    detail: "cannot spawn a supervisor with Member role".into(),
                });
            }
        };

        // Review Integrators need artifacts access
        let artifacts_dir = if role == WorkerRole::ReviewIntegrator {
            self.resolve_supervisor_artifacts(task_id)?
        } else {
            None
        };

        let container_id = self.spawn_supervisor_container(
            sup_name,
            image,
            prompt_path,
            &terminal_target,
            role,
            artifacts_dir,
        )?;

        // Register in DB
        self.interactor
            .data_store
            .insert_worker(&InsertWorkerRequest {
                id: sup_id.clone(),
                workflow_id: task_state.workflow_id,
                role,
                status: WorkerStatus::Booting,
                supervisor_id: None,
                container_id,
                terminal_target,
                // Claude Code session does not exist yet; it will be created once the process boots.
                session_id: None,
                task_id: task_id.clone(),
            })?;

        tracing::info!(
            supervisor_id = %sup_id,
            task_id = %task_id,
            role = %role,
            "spawned dynamic supervisor"
        );

        // ReviewIntegrator needs job instruction so it knows which job to submit
        // and where to find review.md files.
        if role == WorkerRole::ReviewIntegrator {
            self.enqueue_ri_instruction(task_id, &sup_id)?;
        }

        Ok(sup_id)
    }

    /// Enqueue a job instruction message for a ReviewIntegrator.
    /// The RI needs the review-integrate job ID and round number to submit its verdict.
    /// Also transitions the job to InProgress so crash recovery can detect
    /// that the RI has active work and nudge it to continue.
    fn enqueue_ri_instruction(&self, task_id: &TaskId, ri_id: &WorkerId) -> crate::Result<()> {
        let Some(job) = self.interactor.data_store.get_job_by_task_id(task_id)? else {
            tracing::warn!(task_id = %task_id, "no job found for review-integrate task");
            return Ok(());
        };
        let round = self.current_review_round(&job)?;
        let instruction = crate::orchestrator::handler::job_instruction::format_job_instruction(
            &job,
            Some(round),
        );

        // Assign the job to the RI (sets assignee_id and transitions to InProgress).
        // The job was created earlier without an assignee because the RI
        // didn't exist yet at job creation time.
        self.interactor
            .data_store
            .assign_job(&job.id, ri_id, job.job_type)?;

        self.interactor
            .data_store
            .enqueue_message(ri_id, &instruction)?;
        tracing::info!(
            ri_id = %ri_id,
            job_id = %job.id,
            round = round,
            "enqueued job instruction for ReviewIntegrator"
        );
        Ok(())
    }

    /// Resolve the artifacts mount for a ReviewIntegrator supervisor.
    /// The supervisor's task_id is the review composite task,
    /// whose parent is the craft task.
    fn resolve_supervisor_artifacts(
        &self,
        task_id: &TaskId,
    ) -> crate::Result<Option<ArtifactsMount>> {
        let Some(task_state) = self.interactor.data_store.get_task_state(task_id)? else {
            return Ok(None);
        };
        let task_store = self.interactor.create_task_store(&task_state.workflow_id)?;
        let Some(task) = task_store.get_task(task_id) else {
            return Ok(None);
        };
        let Some(ref parent_id) = task.parent_id else {
            return Ok(None);
        };
        let Some(craft_job) = self.interactor.data_store.get_job_by_task_id(parent_id)? else {
            return Ok(None);
        };

        let artifacts_path = self
            .workspace_manager
            .artifacts_path(task_state.workflow_id.as_ref(), craft_job.id.as_ref());
        std::fs::create_dir_all(&artifacts_path)
            .map_err(|e| crate::Error::External(Box::new(e)))?;
        let abs_path = std::fs::canonicalize(&artifacts_path)
            .map_err(|e| crate::Error::External(Box::new(e)))?;

        Ok(Some(ArtifactsMount {
            host_path: abs_path.to_string_lossy().to_string(),
            read_only: false,
        }))
    }

    fn spawn_supervisor_container(
        &self,
        name: &str,
        image: &str,
        prompt_path: &str,
        terminal_target: &palette_domain::terminal::TerminalTarget,
        role: WorkerRole,
        artifacts_dir: Option<ArtifactsMount>,
    ) -> crate::Result<ContainerId> {
        let container_id = self.interactor.container.create_container(
            name,
            image,
            role,
            &self.session_name,
            ContainerMounts {
                artifacts_dir,
                ..Default::default()
            },
        )?;
        self.interactor.container.start_container(&container_id)?;
        self.interactor.container.write_settings(
            &container_id,
            std::path::Path::new(&self.docker_config.settings_template),
            name,
        )?;
        self.interactor.container.copy_file_to_container(
            &container_id,
            std::path::Path::new(prompt_path),
            "/home/agent/prompt.md",
        )?;
        self.interactor.container.copy_dir_to_container(
            &container_id,
            std::path::Path::new("claude-code-plugin"),
            "/home/agent/claude-code-plugin",
        )?;
        self.interactor.container.copy_file_to_container(
            &container_id,
            std::path::Path::new("config/hooks/guard-cd-chain.sh"),
            "/home/agent/.claude/hooks/guard-cd-chain.sh",
        )?;

        let cmd = self.interactor.container.claude_exec_command(
            &container_id,
            "/home/agent/prompt.md",
            role,
            None,
        );
        self.interactor.terminal.send_keys(terminal_target, &cmd)?;

        Ok(container_id)
    }
}
