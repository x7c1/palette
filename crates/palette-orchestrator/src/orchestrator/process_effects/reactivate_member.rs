use super::Orchestrator;
use super::job_instruction::format_job_instruction;
use palette_domain::job::{JobId, JobStatus};
use palette_domain::server::PendingDelivery;
use palette_domain::worker::WorkerId;

impl Orchestrator {
    /// Reactivate an existing member for re-review (same container, new instruction).
    pub(super) fn reactivate_member(
        &self,
        job_id: &JobId,
        member_id: &WorkerId,
        deliveries: &mut Vec<PendingDelivery>,
    ) -> crate::Result<()> {
        let Some(job) = self.interactor.data_store.get_job(job_id)? else {
            return Ok(());
        };
        let Some(member) = self.interactor.data_store.find_worker(member_id)? else {
            return Ok(());
        };

        let instruction = format_job_instruction(&job);
        self.interactor
            .data_store
            .enqueue_message(member_id, &instruction)?;
        let in_progress = JobStatus::in_progress(job.job_type);
        self.interactor
            .data_store
            .update_job_status(job_id, in_progress)?;
        deliveries.push(PendingDelivery {
            target_id: member_id.clone(),
            terminal_target: member.terminal_target.clone(),
        });
        tracing::info!(
            job_id = %job_id,
            member_id = %member_id,
            "reactivated member for re-review"
        );
        Ok(())
    }
}
