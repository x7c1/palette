use super::error::track_error;
use super::types::AdminDeletedCounts;
use crate::Interactor;
use crate::maintenance::error::AdminMaintenanceError;
use palette_domain::workflow::WorkflowId;

impl Interactor {
    pub fn admin_execute_cleanup(
        &self,
        workflow_ids: &[WorkflowId],
    ) -> Result<AdminDeletedCounts, AdminMaintenanceError> {
        let mut deleted = AdminDeletedCounts::default();

        for workflow_id in workflow_ids {
            let task_ids = self
                .data_store
                .get_task_statuses(workflow_id)
                .map_err(track_error)?
                .into_keys()
                .collect::<Vec<_>>();
            let worker_ids = self
                .data_store
                .list_all_workers()
                .map_err(track_error)?
                .into_iter()
                .filter(|w| w.workflow_id == *workflow_id)
                .map(|w| w.id)
                .collect::<Vec<_>>();

            deleted.message_queue += self
                .data_store
                .delete_messages_by_targets(&worker_ids)
                .map_err(track_error)?;
            let (deleted_comments, deleted_submissions) = self
                .data_store
                .delete_review_data_by_workflow(workflow_id)
                .map_err(track_error)?;
            deleted.review_comments += deleted_comments;
            deleted.review_submissions += deleted_submissions;

            for task_id in &task_ids {
                if self
                    .data_store
                    .get_job_by_task_id(task_id)
                    .map_err(track_error)?
                    .is_some()
                {
                    deleted.jobs += 1;
                }
                self.data_store
                    .delete_jobs_by_task_id(task_id)
                    .map_err(track_error)?;
            }

            for worker_id in &worker_ids {
                if self
                    .data_store
                    .remove_worker(worker_id)
                    .map_err(track_error)?
                    .is_some()
                {
                    deleted.workers += 1;
                }
            }

            for task_id in &task_ids {
                self.data_store.delete_task(task_id).map_err(track_error)?;
                deleted.tasks += 1;
            }

            deleted.workflows += self
                .data_store
                .delete_workflow(workflow_id)
                .map_err(track_error)?;
        }

        Ok(deleted)
    }
}
