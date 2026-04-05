use super::Orchestrator;
use palette_domain::job::JobDetail;
use palette_domain::worker::WorkerId;

impl Orchestrator {
    pub(crate) fn destroy_member(&self, member_id: &WorkerId) {
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
            && matches!(job.detail, JobDetail::Craft { .. })
        {
            self.workspace_manager.remove_workspace(job.id.as_ref());
        }
    }

    pub(crate) fn destroy_supervisor(&self, supervisor_id: &WorkerId) {
        let worker = match self.interactor.data_store.remove_worker(supervisor_id) {
            Ok(Some(w)) => w,
            Ok(None) => return,
            Err(e) => {
                tracing::error!(supervisor_id = %supervisor_id, error = %e, "failed to remove supervisor from DB");
                return;
            }
        };
        tracing::info!(supervisor_id = %supervisor_id, task_id = %worker.task_id, "destroying supervisor");
        if let Err(e) = self
            .interactor
            .container
            .stop_container(&worker.container_id)
        {
            tracing::warn!(supervisor_id = %supervisor_id, error = %e, "failed to stop supervisor container");
        }
        if let Err(e) = self
            .interactor
            .container
            .remove_container(&worker.container_id)
        {
            tracing::warn!(supervisor_id = %supervisor_id, error = %e, "failed to remove supervisor container");
        }
    }
}
