use palette_domain::job::{Job, JobType};
use palette_domain::task::TaskStore;
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
    /// Craft jobs get a new volume; review jobs share the parent craft job's volume (read-only).
    pub(super) fn resolve_workspace(&self, job: &Job) -> crate::Result<Option<WorkspaceVolume>> {
        match job.job_type {
            JobType::Craft => Ok(Some(WorkspaceVolume {
                name: format!("palette-workspace-{}", job.id),
                read_only: false,
            })),
            JobType::Review => {
                // Review is a child of craft, so find the parent task's craft job
                let task_id = &job.task_id;
                let Some(task_state) = self.interactor.data_store.get_task_state(task_id)? else {
                    return Ok(None);
                };
                let task_store = self
                    .interactor
                    .create_task_store(&task_state.workflow_id)
                    .map_err(|e| crate::Error::Internal(e.to_string()))?;
                let Some(task) = task_store.get_task(task_id)? else {
                    return Ok(None);
                };
                let Some(ref parent_id) = task.parent_id else {
                    return Ok(None);
                };
                let Some(craft_job) = self.interactor.data_store.get_job_by_task_id(parent_id)?
                else {
                    return Ok(None);
                };
                Ok(Some(WorkspaceVolume {
                    name: format!("palette-workspace-{}", craft_job.id),
                    read_only: true,
                }))
            }
        }
    }
}
