use super::EffectResult;
use super::Orchestrator;
use palette_domain::job::{JobStatus, JobType, ReviewStatus, ReviewTransition};
use palette_domain::task::{TaskId, TaskStatus};
use palette_domain::worker::WorkerRole;

impl Orchestrator {
    /// When a Craft Job reaches InReview, activate its child review tasks.
    /// The craft task stays InProgress; review tasks become Ready.
    pub(in crate::orchestrator) fn activate_child_review_tasks(
        &self,
        craft_job_id: &palette_domain::job::JobId,
    ) -> crate::Result<EffectResult> {
        let mut result = EffectResult::new();

        let Some(job) = self.interactor.data_store.get_job(craft_job_id)? else {
            return Ok(result);
        };
        let task_id = &job.task_id;
        let Some(task_state) = self.interactor.data_store.get_task_state(task_id)? else {
            return Ok(result);
        };

        let task_store = self.interactor.create_task_store(&task_state.workflow_id)?;

        let children = task_store.get_child_tasks(task_id);
        if children.is_empty() {
            return Ok(result);
        }

        // Ensure the craft task is InProgress (it should be, since its job is active)
        if let Some(task) = task_store.get_task(task_id)
            && task.status != TaskStatus::InProgress
        {
            task_store.update_task_status(task_id, TaskStatus::InProgress)?;
        }

        // First pass: activate Pending children (initial review cycle)
        use palette_usecase::TaskRuleEngine;
        let task_engine = TaskRuleEngine::new(&task_store);
        let child_ids: Vec<TaskId> = children.iter().map(|c| c.id.clone()).collect();
        let ready_ids = task_engine.resolve_ready_tasks(&child_ids);

        for ready_id in &ready_ids {
            tracing::info!(task_id = %ready_id, status = ?TaskStatus::Ready, "review task activated");

            // If this is a review-integrate composite, spawn an
            // Approver to handle reviewer permission prompts.
            // The ReviewIntegrator is spawned later when all reviewers complete.
            if let Some(child_task) = task_store.get_task(ready_id)
                && child_task.job_type == Some(JobType::ReviewIntegrate)
            {
                tracing::info!(task_id = %ready_id, "spawning Approver for review-integrate composite");
                match self.handle_spawn_supervisor(ready_id, WorkerRole::Approver) {
                    Ok(sup_id) => result.spawned_supervisors.push(sup_id),
                    Err(e) => {
                        tracing::error!(error = %e, task_id = %ready_id, "failed to spawn Approver");
                    }
                }
            }

            result = result.merge(self.activate_ready_task(ready_id, &task_store, &task_engine)?);
        }

        // Second pass: reactivate ChangesRequested review jobs (re-review cycle).
        for child in &children {
            let Some(review_job) = self.interactor.data_store.get_job_by_task_id(&child.id)? else {
                continue;
            };
            if !matches!(
                review_job.status,
                JobStatus::Review(ReviewStatus::ChangesRequested)
            ) {
                tracing::debug!(
                    job_id = %review_job.id,
                    status = ?review_job.status,
                    "skipping review job reactivation (not ChangesRequested)"
                );
                continue;
            }
            self.interactor
                .data_store
                .update_job_status(&review_job.id, ReviewTransition::Restart.to_job_status())?;
            tracing::info!(
                job_id = %review_job.id,
                task_id = %child.id,
                "reactivated ChangesRequested review job for re-review"
            );
            if let Some(ref assignee) = review_job.assignee_id {
                result = result.merge(self.reactivate_member(&review_job.id, assignee)?);
            }
        }

        Ok(result)
    }
}
