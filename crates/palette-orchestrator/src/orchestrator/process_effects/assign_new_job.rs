use super::Orchestrator;
use super::PendingActions;
use super::job_instruction::format_job_instruction;
use palette_domain::job::JobId;
use palette_domain::worker::WorkerId;
use palette_usecase::data_store::InsertWorkerRequest;

impl Orchestrator {
    /// Assign a new job to a freshly spawned member.
    /// Skipped when the workflow is suspending (no new members during suspend).
    pub(in crate::orchestrator) fn assign_new_job(
        &self,
        job_id: &JobId,
    ) -> crate::Result<PendingActions> {
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

        // Mechanized jobs (Orchestrator/Operator) don't spawn worker containers
        if !job.job_type.needs_worker() {
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
            job.job_type,
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
            .assign_job(job_id, &member_id, job.job_type)?;
        tracing::info!(
            job_id = %job_id,
            member_id = %member_id,
            "auto-assigned job"
        );

        // Build job instruction message
        let round = if job.job_type == palette_domain::job::JobType::Review {
            Some(self.current_review_round(&job)?)
        } else {
            None
        };
        let instruction = format_job_instruction(&job, round);
        self.interactor
            .data_store
            .enqueue_message(&member_id, &instruction)?;

        result.deliver_to.push(member_id);

        Ok(result)
    }

    /// Handle a mechanized job (Orchestrator or Operator).
    /// These jobs don't spawn worker containers.
    fn handle_mechanized_job(&self, job: &palette_domain::job::Job) -> crate::Result<()> {
        match job.job_type {
            palette_domain::job::JobType::Orchestrator => {
                tracing::info!(job_id = %job.id, command = ?job.command, "executing orchestrator task");
                self.execute_orchestrator_task(job, &self.event_tx);
            }
            palette_domain::job::JobType::Operator => {
                // Operator jobs wait for human input via the API.
                // Mark as in-progress (waiting) — the API will complete it.
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
}
