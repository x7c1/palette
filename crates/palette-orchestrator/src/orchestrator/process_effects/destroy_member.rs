use super::Orchestrator;
use palette_domain::job::JobType;
use palette_domain::worker::WorkerId;

impl Orchestrator {
    pub(super) fn destroy_member(&self, member_id: &WorkerId) {
        let worker = match self.interactor.data_store.remove_worker(member_id) {
            Ok(Some(w)) => w,
            Ok(None) => return,
            Err(e) => {
                tracing::error!(member_id = %member_id, error = %e, "failed to remove member from DB");
                return;
            }
        };
        tracing::info!(member_id = %member_id, "destroying member container");
        if let Err(e) = self
            .interactor
            .container
            .stop_container(&worker.container_id)
        {
            tracing::warn!(member_id = %member_id, error = %e, "failed to stop member container");
        }
        if let Err(e) = self
            .interactor
            .container
            .remove_container(&worker.container_id)
        {
            tracing::warn!(member_id = %member_id, error = %e, "failed to remove member container");
        }

        // Clean up workspace directory for craft jobs
        if let Ok(Some(job)) = self
            .interactor
            .data_store
            .get_job_by_task_id(&worker.task_id)
            && job.job_type == JobType::Craft
        {
            self.workspace_manager.remove_workspace(job.id.as_ref());
        }
    }
}
