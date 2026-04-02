use super::Orchestrator;
use super::PendingActions;
use palette_domain::job::{JobId, JobType};
use palette_domain::task::{TaskId, TaskStatus};
use palette_domain::worker::WorkerRole;
use palette_usecase::task_store::TaskStore;

impl Orchestrator {
    /// Check if a job's task can be completed and cascade.
    /// A task is complete when all children are Completed AND its own job (if any) is Done.
    pub(crate) fn try_complete_task_by_job(&self, job_id: &JobId) -> crate::Result<PendingActions> {
        let Some(job) = self.interactor.data_store.get_job(job_id)? else {
            return Ok(PendingActions::new());
        };
        let task_id = &job.task_id;
        let Some(task_state) = self.interactor.data_store.get_task_state(task_id)? else {
            return Ok(PendingActions::new());
        };

        let task_store = self.interactor.create_task_store(&task_state.workflow_id)?;

        // Check if all children are Completed (if any)
        let children = task_store.get_child_tasks(task_id);
        let all_children_completed = children.iter().all(|c| c.status == TaskStatus::Completed);

        if !all_children_completed && !children.is_empty() {
            return Ok(PendingActions::new());
        }

        // All conditions met: mark task as Completed
        task_store.update_task_status(task_id, TaskStatus::Completed)?;
        tracing::info!(task_id = %task_id, "task completed (job done + all children completed)");

        // Destroy all supervisors for this task (e.g. review-integrate tasks
        // may have both an Approver and a ReviewIntegrator)
        if let Ok(sups) = self
            .interactor
            .data_store
            .find_supervisors_for_task(task_id)
        {
            for sup in sups {
                self.destroy_supervisor(&sup.id);
            }
        }

        let result = self
            .cascade_task_completion(task_id, &task_store)?
            .merge(self.fill_vacant_slots()?);

        Ok(result)
    }

    /// Find assignable jobs waiting for a member slot and assign them.
    fn fill_vacant_slots(&self) -> crate::Result<PendingActions> {
        let assignable = self.interactor.data_store.find_assignable_jobs()?;
        let mut result = PendingActions::new();
        for job in &assignable {
            result = result.merge(self.assign_new_job(&job.id)?);
        }
        Ok(result)
    }

    /// Process cascading effects after a task completes.
    fn cascade_task_completion(
        &self,
        completed_task_id: &TaskId,
        task_store: &TaskStore,
    ) -> crate::Result<PendingActions> {
        use palette_usecase::TaskRuleEngine;

        let task_engine = TaskRuleEngine::new(task_store);
        let completion = task_engine.on_task_completed(completed_task_id);

        let mut result = PendingActions::new();

        // Process newly ready tasks
        for task_id in &completion.newly_ready {
            tracing::info!(task_id = %task_id, status = ?TaskStatus::Ready, "task status cascaded");
            result = result.merge(self.activate_ready_task(task_id, task_store, &task_engine)?);
        }

        // Process parent completion
        if let Some(ref parent_id) = completion.parent_completed {
            result =
                result.merge(self.handle_parent_completion(parent_id, task_store, &task_engine)?);
        }

        Ok(result)
    }

    /// Handle a parent task that may be completing (all children done).
    fn handle_parent_completion(
        &self,
        task_id: &TaskId,
        task_store: &TaskStore,
        task_engine: &palette_usecase::TaskRuleEngine<'_>,
    ) -> crate::Result<PendingActions> {
        let mut result = PendingActions::new();

        // Before marking parent as Completed, check its own Job (if any)
        let own_job_done = self
            .interactor
            .data_store
            .get_job_by_task_id(task_id)?
            .is_none_or(|j| j.status.is_done());

        if !own_job_done {
            // For review-integrate tasks: all child reviewers are done.
            // Spawn the ReviewIntegrator to read review.md files and
            // write integrated-review.md.
            if let Some(task) = task_store.get_task(task_id)
                && task.job_type == Some(JobType::ReviewIntegrate)
            {
                tracing::info!(
                    task_id = %task_id,
                    "all child reviewers completed; spawning ReviewIntegrator"
                );
                match self.handle_spawn_supervisor(task_id, WorkerRole::ReviewIntegrator) {
                    Ok(sup_id) => result.watch_only.push(sup_id),
                    Err(e) => {
                        tracing::error!(error = %e, task_id = %task_id, "failed to spawn ReviewIntegrator");
                    }
                }
            } else {
                tracing::info!(
                    task_id = %task_id,
                    "all children completed but own job not done; deferring task completion"
                );
            }
            return Ok(result);
        }

        task_store.update_task_status(task_id, TaskStatus::Completed)?;
        tracing::info!(task_id = %task_id, status = ?TaskStatus::Completed, "task status cascaded");

        // Destroy all supervisors for this composite task
        if let Ok(sups) = self
            .interactor
            .data_store
            .find_supervisors_for_task(task_id)
        {
            for sup in sups {
                self.destroy_supervisor(&sup.id);
            }
        }

        // Check workflow completion
        if let Some(task) = task_store.get_task(task_id)
            && task.parent_id.is_none()
        {
            use palette_domain::workflow::WorkflowStatus;
            self.interactor
                .data_store
                .update_workflow_status(&task.workflow_id, WorkflowStatus::Completed)?;
            tracing::info!(
                workflow_id = %task.workflow_id,
                "workflow completed"
            );
        }

        // Continue cascading
        let completion = task_engine.on_task_completed(task_id);
        for ready_id in &completion.newly_ready {
            tracing::info!(task_id = %ready_id, status = ?TaskStatus::Ready, "task status cascaded");
            result = result.merge(self.activate_ready_task(ready_id, task_store, task_engine)?);
        }
        if let Some(ref parent_id) = completion.parent_completed {
            result =
                result.merge(self.handle_parent_completion(parent_id, task_store, task_engine)?);
        }

        Ok(result)
    }
}
