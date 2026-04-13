use super::error::track_error;
use super::types::AdminCleanupPlan;
use crate::Interactor;
use crate::maintenance::error::AdminMaintenanceError;
use palette_domain::workflow::WorkflowId;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

impl Interactor {
    pub(super) fn gather_cleanup_plan(
        &self,
        workflow_ids: &[WorkflowId],
        data_dir: &Path,
    ) -> Result<AdminCleanupPlan, AdminMaintenanceError> {
        let all_workers = self.data_store.list_all_workers().map_err(track_error)?;
        let all_workflows = self.data_store.list_workflows(None).map_err(track_error)?;

        let mut task_ids = Vec::new();
        let mut job_ids = Vec::new();
        let mut worker_ids = Vec::new();
        let mut file_paths = BTreeSet::new();

        for workflow_id in workflow_ids {
            let tasks = self
                .data_store
                .get_task_statuses(workflow_id)
                .map_err(track_error)?;
            let mut workflow_job_ids = Vec::new();
            for task_id in tasks.keys() {
                task_ids.push(task_id.clone());
                if let Some(job) = self
                    .data_store
                    .get_job_by_task_id(task_id)
                    .map_err(track_error)?
                {
                    let jid = job.id.to_string();
                    workflow_job_ids.push(jid.clone());
                    job_ids.push(jid);
                }
            }

            let mut workflow_worker_ids = Vec::new();
            for worker in all_workers.iter().filter(|w| w.workflow_id == *workflow_id) {
                worker_ids.push(worker.id.clone());
                workflow_worker_ids.push(worker.id.clone());
            }

            file_paths.insert(data_dir.join("artifacts").join(workflow_id.as_ref()));
            for job_id in &workflow_job_ids {
                file_paths.insert(data_dir.join("workspace").join(job_id));
            }
            for worker_id in &workflow_worker_ids {
                file_paths.insert(data_dir.join("transcripts").join(worker_id.as_ref()));
            }
            if let Some(wf) = all_workflows.iter().find(|w| w.id == *workflow_id) {
                file_paths.insert(resolve_path_like(&wf.blueprint_path));
            }
        }

        Ok(AdminCleanupPlan {
            workflow_ids: workflow_ids.to_vec(),
            task_ids,
            job_ids,
            worker_ids,
            file_paths: file_paths.into_iter().collect(),
        })
    }
}

fn resolve_path_like(path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        p
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}
