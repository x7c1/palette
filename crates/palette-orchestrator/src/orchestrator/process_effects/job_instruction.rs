use palette_domain::job::{Job, JobType};
use palette_usecase::container_runtime::{ArtifactsMount, WorkspaceVolume};

use super::Orchestrator;

/// Container-side mount point for the shared plan directory.
const PLAN_DIR_MOUNT: &str = "/home/agent/plans";

/// Container-side mount point for artifacts.
const ARTIFACTS_MOUNT: &str = "/home/agent/artifacts";

/// Format a job into an instruction message for a member.
///
/// `round` is included for review jobs so the reviewer knows which round directory to use.
pub(super) fn format_job_instruction(job: &Job, round: Option<u32>) -> String {
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
    if let Some(round) = round {
        msg.push_str(&format!(
            "\nRound: {round}\nArtifacts: {ARTIFACTS_MOUNT}/round-{round}/{}/\n",
            job.id
        ));
    }
    msg.push_str("\nPlease begin working on this task.");
    msg
}

impl Orchestrator {
    /// Determine the artifacts mount for a job.
    ///
    /// Review jobs get a read-write mount of the artifacts directory.
    /// Craft jobs get a read-only mount (to read review feedback).
    pub(super) fn resolve_artifacts_mount(
        &self,
        job: &Job,
    ) -> crate::Result<Option<ArtifactsMount>> {
        let (workflow_id, craft_job_id) = match job.job_type {
            JobType::Craft => {
                let Some(task_state) = self.interactor.data_store.get_task_state(&job.task_id)?
                else {
                    return Ok(None);
                };
                (task_state.workflow_id, job.id.clone())
            }
            // Orchestrator/Operator jobs don't have artifacts mounts
            JobType::Orchestrator | JobType::Operator => return Ok(None),
            JobType::Review => {
                let Some(task_state) = self.interactor.data_store.get_task_state(&job.task_id)?
                else {
                    return Ok(None);
                };
                let task_store = self.interactor.create_task_store(&task_state.workflow_id)?;
                let craft_job = match self.find_ancestor_craft_job(&task_store, &job.task_id) {
                    Some(j) => j,
                    None => return Ok(None),
                };
                (task_state.workflow_id, craft_job.id)
            }
        };

        let artifacts_path = self
            .workspace_manager
            .artifacts_path(workflow_id.as_ref(), craft_job_id.as_ref());
        std::fs::create_dir_all(&artifacts_path)
            .map_err(|e| crate::Error::External(Box::new(e)))?;

        // For review jobs, pre-create the round and reviewer subdirectories
        // so the container (which may run as a different user) can write there.
        if job.job_type == JobType::Review {
            let round = self.current_review_round(job)?;
            let reviewer_dir = artifacts_path
                .join(format!("round-{round}"))
                .join(job.id.to_string());
            std::fs::create_dir_all(&reviewer_dir)
                .map_err(|e| crate::Error::External(Box::new(e)))?;
        }

        let abs_path = std::fs::canonicalize(&artifacts_path)
            .map_err(|e| crate::Error::External(Box::new(e)))?;

        Ok(Some(ArtifactsMount {
            host_path: abs_path.to_string_lossy().to_string(),
            read_only: job.job_type == JobType::Craft,
        }))
    }

    /// Get the current review round for a review job.
    pub(super) fn current_review_round(&self, job: &Job) -> crate::Result<u32> {
        let submissions = self.interactor.data_store.get_review_submissions(&job.id)?;
        Ok(submissions.len() as u32 + 1)
    }

    /// Determine the workspace volume for a job.
    ///
    /// Craft jobs get a new workspace via `git clone --shared` from the bare cache.
    /// Review jobs share the parent craft job's workspace as read-only.
    pub(super) fn resolve_workspace(&self, job: &Job) -> crate::Result<Option<WorkspaceVolume>> {
        match job.job_type {
            // Orchestrator/Operator jobs don't have workspaces
            JobType::Orchestrator | JobType::Operator => Ok(None),
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
                // Review may be nested: reviewer → composite review → craft.
                // Walk up the task tree to find the ancestor craft job.
                let task_id = &job.task_id;
                let Some(task_state) = self.interactor.data_store.get_task_state(task_id)? else {
                    return Ok(None);
                };
                let task_store = self.interactor.create_task_store(&task_state.workflow_id)?;
                let craft_job = match self.find_ancestor_craft_job(&task_store, task_id) {
                    Some(j) => j,
                    None => return Ok(None),
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
