use super::Orchestrator;
use palette_domain::job::{JobId, JobStatus, JobType, ReviewStatus};
use palette_domain::rule::RuleEffect;
use palette_domain::task::{TaskId, TaskStatus, TaskStore};
use palette_domain::worker::WorkerRole;

impl Orchestrator {
    /// When a Craft Job reaches InReview, activate its child review tasks.
    /// The craft task stays InProgress; review tasks become Ready.
    pub(super) fn activate_child_review_tasks(
        &self,
        craft_job_id: &JobId,
    ) -> crate::Result<Vec<RuleEffect>> {
        let Some(job) = self.interactor.data_store.get_job(craft_job_id)? else {
            return Ok(vec![]);
        };
        let task_id = &job.task_id;
        let Some(task_state) = self.interactor.data_store.get_task_state(task_id)? else {
            return Ok(vec![]);
        };

        let task_store = self
            .interactor
            .create_task_store(&task_state.workflow_id)
            .map_err(|e| crate::Error::Internal(e.to_string()))?;

        let children = task_store.get_child_tasks(task_id)?;
        if children.is_empty() {
            return Ok(vec![]);
        }

        // Ensure the craft task is InProgress (it should be, since its job is active)
        if let Some(task) = task_store.get_task(task_id)?
            && task.status != TaskStatus::InProgress
        {
            task_store.update_task_status(task_id, TaskStatus::InProgress)?;
        }

        let mut job_effects = Vec::new();

        // First pass: activate Pending children (initial review cycle)
        use palette_domain::rule::TaskRuleEngine;
        let task_engine = TaskRuleEngine::new(&task_store);
        let child_ids: Vec<TaskId> = children.iter().map(|c| c.id.clone()).collect();
        let task_effects = task_engine.resolve_ready_tasks(&child_ids)?;

        let mut pending = task_effects;
        while !pending.is_empty() {
            let mut next = Vec::new();
            for effect in &pending {
                let palette_domain::rule::TaskEffect::TaskStatusChanged {
                    task_id,
                    new_status,
                } = effect
                else {
                    continue;
                };

                task_store.update_task_status(task_id, *new_status)?;
                tracing::info!(task_id = %task_id, status = ?new_status, "review task activated");

                if *new_status == TaskStatus::Ready {
                    // If this is a review-integrate composite (has children + job_type: review),
                    // spawn a ReviewIntegrator supervisor before activation
                    let grandchildren = task_store.get_child_tasks(task_id)?;
                    if !grandchildren.is_empty()
                        && let Some(child_task) = task_store.get_task(task_id)?
                        && child_task.job_type == Some(JobType::Review)
                    {
                        job_effects.push(RuleEffect::SpawnSupervisor {
                            task_id: task_id.clone(),
                            role: WorkerRole::ReviewIntegrator,
                        });
                    }

                    let (follow_up, effects) =
                        self.activate_ready_task(task_id, &task_store, &task_engine)?;
                    next.extend(follow_up);
                    job_effects.extend(effects);
                }
            }
            pending = next;
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
                .update_job_status(&review_job.id, JobStatus::Review(ReviewStatus::InProgress))?;
            tracing::info!(
                job_id = %review_job.id,
                task_id = %child.id,
                "reactivated ChangesRequested review job for re-review"
            );
            if let Some(ref assignee) = review_job.assignee_id {
                job_effects.push(RuleEffect::ReactivateMember {
                    job_id: review_job.id.clone(),
                    member_id: assignee.clone(),
                });
            }
        }

        Ok(job_effects)
    }
}
