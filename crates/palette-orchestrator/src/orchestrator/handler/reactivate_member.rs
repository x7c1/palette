use super::Orchestrator;
use super::PendingActions;
use super::job_instruction::format_job_instruction;
use palette_domain::job::{CraftTransition, JobDetail, JobId, ReviewTarget, ReviewTransition};
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

        let task_state = self
            .interactor
            .data_store
            .get_task_state(&job.task_id)?
            .ok_or_else(|| crate::Error::TaskNotFound {
                task_id: job.task_id.clone(),
            })?;
        // For reactivation, the container's workspace mount is already fixed
        // by the original spawn. Recompute the workspace host path so that
        // plan location resolution stays consistent with the first
        // assignment.
        let workspace_path = self.existing_workspace_path_for_job(&job)?;
        let plan_loc =
            self.resolve_plan_location(&task_state.workflow_id, workspace_path.as_deref())?;

        let round = if matches!(job.detail, JobDetail::Review { .. }) {
            Some(self.current_review_round(&job)?)
        } else {
            None
        };
        let instruction = format_job_instruction(&job, round, &self.perspectives, &plan_loc);
        self.interactor
            .data_store
            .enqueue_message(member_id, &instruction)?;
        // ReactivateMember is used for both craft (ChangesRequested → re-work)
        // and review (re-review cycle). The transition meaning differs by job type.
        let reactivated_status = match &job.detail {
            JobDetail::Craft { .. } => CraftTransition::RequestChanges.to_job_status(),
            JobDetail::Review { .. } => ReviewTransition::Restart.to_job_status(),
            // ReviewIntegrate/Orchestrator/Operator jobs don't have members to reactivate
            JobDetail::ReviewIntegrate { .. }
            | JobDetail::Orchestrator { .. }
            | JobDetail::Operator => {
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

    /// Absolute host path of the workspace that a running job is already
    /// attached to, used for plan-location resolution during reactivation.
    ///
    /// Returns `None` for jobs that never had a workspace (mechanized jobs,
    /// ReviewIntegrate) or when the workspace directory is no longer on disk.
    fn existing_workspace_path_for_job(
        &self,
        job: &palette_domain::job::Job,
    ) -> crate::Result<Option<std::path::PathBuf>> {
        let ws_source_id = match &job.detail {
            JobDetail::Craft { .. } => job.id.clone(),
            JobDetail::Review { target, .. } => match target {
                ReviewTarget::PullRequest(_) => job.id.clone(),
                ReviewTarget::CraftOutput => {
                    let Some(task_state) =
                        self.interactor.data_store.get_task_state(&job.task_id)?
                    else {
                        return Ok(None);
                    };
                    let task_store = self.interactor.create_task_store(&task_state.workflow_id)?;
                    let Some(craft_job) = self.find_ancestor_craft_job(&task_store, &job.task_id)
                    else {
                        return Ok(None);
                    };
                    craft_job.id
                }
            },
            JobDetail::ReviewIntegrate { .. }
            | JobDetail::Orchestrator { .. }
            | JobDetail::Operator => {
                return Ok(None);
            }
        };
        let ws_path = self.workspace_manager.workspace_path(ws_source_id.as_ref());
        if !ws_path.exists() {
            return Ok(None);
        }
        let abs =
            std::fs::canonicalize(&ws_path).map_err(|e| crate::Error::External(Box::new(e)))?;
        Ok(Some(abs))
    }
}
