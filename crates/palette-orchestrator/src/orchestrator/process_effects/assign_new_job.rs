use super::Orchestrator;
use super::job_instruction::format_job_instruction;
use palette_domain::agent::AgentId;
use palette_domain::job::JobId;
use palette_domain::server::{PendingDelivery, PersistentState};

impl Orchestrator {
    /// Assign a new job to a freshly spawned member.
    pub(super) fn assign_new_job(
        &self,
        job_id: &JobId,
        infra: &mut PersistentState,
        deliveries: &mut Vec<PendingDelivery>,
    ) -> crate::Result<()> {
        // Verify the job is assignable (todo + no assignee)
        let assignable_jobs = self.db.find_assignable_jobs()?;
        let job = match assignable_jobs.iter().find(|j| j.id == *job_id) {
            Some(j) => j.clone(),
            None => return Ok(()),
        };
        let active = self.db.count_active_members()?;
        if active >= self.docker_config.max_members {
            tracing::info!(
                job_id = %job_id,
                active = active,
                max = self.docker_config.max_members,
                "max members reached, job waits"
            );
            return Ok(());
        }

        // Determine workspace volume based on job type
        let workspace = self.resolve_workspace(&job)?;

        // Spawn a new member with supervisor from the task tree
        let task_state = self
            .db
            .get_task_state(&job.task_id)?
            .ok_or_else(|| crate::Error::Internal(format!("task not found: {}", job.task_id)))?;
        let supervisor_id = self.find_supervisor_for_job(&job.task_id, infra)?;
        let seq = self.db.increment_worker_counter(&task_state.workflow_id)?;
        let member_id = AgentId::next_member(seq);
        let member =
            self.spawn_member(&member_id, job.job_type, &supervisor_id, infra, workspace)?;
        let terminal_target = member.terminal_target.clone();
        infra.members.push(member);

        // Assign job
        self.db.assign_job(job_id, &member_id, job.job_type)?;
        tracing::info!(
            job_id = %job_id,
            member_id = %member_id,
            "auto-assigned job"
        );

        // Build job instruction message
        let instruction = format_job_instruction(&job);
        self.db.enqueue_message(&member_id, &instruction)?;

        deliveries.push(PendingDelivery {
            target_id: member_id,
            terminal_target,
        });

        infra.touch();
        Ok(())
    }
}
