use super::Orchestrator;
use super::PendingActions;
use super::job_instruction::format_job_instruction;
use palette_domain::job::{CraftTransition, JobId, ReviewTransition};
use palette_domain::worker::WorkerId;

impl Orchestrator {
    /// Reactivate an idle member with a new instruction (same container, preserving context).
    pub(crate) fn reactivate_member(
        &self,
        job_id: &JobId,
        member_id: &WorkerId,
    ) -> crate::Result<PendingActions> {
        let mut result = PendingActions::new();

        let Some(job) = self.interactor.data_store.get_job(job_id)? else {
            return Ok(result);
        };
        let Some(_member) = self.interactor.data_store.find_worker(member_id)? else {
            return Ok(result);
        };

        let round = if job.job_type == palette_domain::job::JobType::Review {
            Some(self.current_review_round(&job)?)
        } else {
            None
        };
        let instruction = format_job_instruction(&job, round);
        self.interactor
            .data_store
            .enqueue_message(member_id, &instruction)?;
        // ReactivateMember is used for both craft (ChangesRequested → re-work)
        // and review (re-review cycle). The transition meaning differs by job type.
        let reactivated_status = match job.job_type {
            palette_domain::job::JobType::Craft => CraftTransition::RequestChanges.to_job_status(),
            palette_domain::job::JobType::Review => ReviewTransition::Restart.to_job_status(),
            // ReviewIntegrate/Orchestrator/Operator jobs don't have members to reactivate
            palette_domain::job::JobType::ReviewIntegrate
            | palette_domain::job::JobType::Orchestrator
            | palette_domain::job::JobType::Operator => {
                return Ok(result);
            }
        };
        self.interactor
            .data_store
            .update_job_status(job_id, reactivated_status)?;
        result.deliver_to.push(member_id.clone());
        tracing::info!(
            job_id = %job_id,
            member_id = %member_id,
            "reactivated member"
        );
        Ok(result)
    }
}
