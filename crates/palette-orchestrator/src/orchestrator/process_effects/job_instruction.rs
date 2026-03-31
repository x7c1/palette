use palette_domain::job::{Job, JobType};
use palette_usecase::container_runtime::WorkspaceVolume;

use super::Orchestrator;

/// Container-side mount point for the shared plan directory.
const PLAN_DIR_MOUNT: &str = "/home/agent/plans";

/// Format a job into an instruction message for a member.
pub(super) fn format_job_instruction(job: &Job) -> String {
    let mut msg = format!(
        "## Task: {}\n\nID: {}\nPlan: {}/{}\n",
        job.title, job.id, PLAN_DIR_MOUNT, job.plan_path
    );
    if let Some(ref repo) = job.repository {
        msg.push_str(&format!(
            "\nRepository: {} (branch: {})\n",
            repo.name, repo.branch
        ));
    }
    msg.push_str("\nPlease begin working on this task.");
    msg
}

impl Orchestrator {
    /// Determine the workspace volume for a job.
    ///
    /// Craft jobs get a new workspace via `git clone --shared` from the bare cache.
    /// Review jobs share the parent craft job's workspace as read-only.
    pub(super) fn resolve_workspace(&self, job: &Job) -> crate::Result<Option<WorkspaceVolume>> {
        match job.job_type {
            JobType::Craft => {
                let Some(ref repo) = job.repository else {
                    return Ok(None);
                };
                let info = self
                    .workspace_manager
                    .create_workspace(job.id.as_ref(), repo)?;
                Ok(Some(WorkspaceVolume {
                    host_path: info.host_path,
                    repo_cache_path: info.repo_cache_path,
                    read_only: false,
                }))
            }
            JobType::Review => {
                // Review is a child of craft, so find the parent task's craft job
                let task_id = &job.task_id;
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
                let Some(craft_job) = self.interactor.data_store.get_job_by_task_id(parent_id)?
                else {
                    return Ok(None);
                };
                let Some(ref repo) = craft_job.repository else {
                    return Ok(None);
                };
                let cache_path = self.workspace_manager.repo_cache_path(repo);
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
