use super::EffectResult;
use super::Orchestrator;
use palette_core::ReasonKey;
use palette_domain::job::{JobId, JobType};
use palette_domain::task::{TaskId, TaskStatus};
use palette_domain::worker::WorkerRole;
use palette_usecase::task_store::TaskStore;

impl Orchestrator {
    /// When a Job is Done, check if its task can be completed and cascade.
    pub(in crate::orchestrator) fn complete_job(
        &self,
        job_id: &JobId,
        result: &mut EffectResult,
    ) -> crate::Result<()> {
        self.try_complete_task_by_job(job_id, result)
    }

    /// Check if a job's task can be completed.
    /// A task is complete when all children are Completed AND its own job (if any) is Done.
    pub(in crate::orchestrator) fn try_complete_task_by_job(
        &self,
        job_id: &JobId,
        result: &mut EffectResult,
    ) -> crate::Result<()> {
        let Some(job) = self.interactor.data_store.get_job(job_id)? else {
            return Ok(());
        };
        let task_id = &job.task_id;
        let Some(task_state) = self.interactor.data_store.get_task_state(task_id)? else {
            return Ok(());
        };

        let task_store = self.interactor.create_task_store(&task_state.workflow_id)?;

        // Check if all children are Completed (if any)
        let children = task_store.get_child_tasks(task_id);
        let all_children_completed = children.iter().all(|c| c.status == TaskStatus::Completed);

        if !all_children_completed && !children.is_empty() {
            return Ok(());
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

        self.cascade_task_effects(task_id, &task_store, result)?;

        // Fill vacant member slots with waiting jobs
        self.fill_vacant_slots(result)?;

        Ok(())
    }

    /// Find assignable jobs waiting for a member slot and assign them.
    fn fill_vacant_slots(&self, result: &mut EffectResult) -> crate::Result<()> {
        let assignable = self.interactor.data_store.find_assignable_jobs()?;
        for job in &assignable {
            self.assign_new_job(&job.id, &mut result.deliveries)?;
        }
        Ok(())
    }

    /// Process cascading effects after a task completes.
    fn cascade_task_effects(
        &self,
        completed_task_id: &TaskId,
        task_store: &TaskStore,
        result: &mut EffectResult,
    ) -> crate::Result<()> {
        use palette_usecase::TaskRuleEngine;

        let task_engine = TaskRuleEngine::new(task_store);
        let completion = task_engine.on_task_completed(completed_task_id);

        // Process newly ready tasks
        for task_id in &completion.newly_ready {
            tracing::info!(task_id = %task_id, status = ?TaskStatus::Ready, "task status cascaded");
            self.activate_ready_task(task_id, task_store, &task_engine, result)?;
        }

        // Process parent completion
        if let Some(ref parent_id) = completion.parent_completed {
            self.handle_parent_completion(parent_id, task_store, &task_engine, result)?;
        }

        Ok(())
    }

    /// Handle a parent task that may be completing (all children done).
    fn handle_parent_completion(
        &self,
        task_id: &TaskId,
        task_store: &TaskStore,
        task_engine: &palette_usecase::TaskRuleEngine<'_>,
        result: &mut EffectResult,
    ) -> crate::Result<()> {
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
                    Ok(sup_id) => result.spawned_supervisors.push(sup_id),
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
            return Ok(());
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
            self.activate_ready_task(ready_id, task_store, task_engine, result)?;
        }
        if let Some(ref parent_id) = completion.parent_completed {
            self.handle_parent_completion(parent_id, task_store, task_engine, result)?;
        }

        Ok(())
    }

    /// Handle a task that just became Ready.
    /// Leaf tasks get a Job created; composite tasks with no job resolve their children.
    pub(in crate::orchestrator) fn activate_ready_task(
        &self,
        task_id: &TaskId,
        task_store: &TaskStore,
        task_engine: &palette_usecase::TaskRuleEngine<'_>,
        result: &mut EffectResult,
    ) -> crate::Result<()> {
        let children = task_store.get_child_tasks(task_id);

        if children.is_empty() {
            // Leaf task: create a job if it has a job_type
            if let Some(mut task) = task_store.get_task(task_id)
                && task.job_type.is_some()
            {
                // For review tasks, inherit plan_path from parent craft task
                if task.job_type == Some(JobType::Review)
                    && task.plan_path.is_none()
                    && let Some(ref parent_id) = task.parent_id
                    && let Some(parent) = task_store.get_task(parent_id)
                {
                    task.plan_path = parent.plan_path.clone();
                }
                self.create_and_assign_job(&task, result)?;
            }
        } else {
            if let Some(task) = task_store.get_task(task_id)
                && let Some(job_type) = task.job_type
            {
                task_store.update_task_status(task_id, TaskStatus::InProgress)?;
                match job_type {
                    // Craft composites: create job + member, do NOT resolve children
                    // (activated later on InReview).
                    JobType::Craft => {
                        self.create_and_assign_job(&task, result)?;
                        return Ok(());
                    }
                    // ReviewIntegrate composites: create job (verdict anchor) but do NOT
                    // assign a member. The ReviewIntegrator is spawned after all child
                    // reviewers complete.
                    JobType::ReviewIntegrate => {
                        self.create_job_without_assign(&task)?;
                    }
                    _ => {
                        self.create_and_assign_job(&task, result)?;
                    }
                }
            } else {
                // Pure composite task (no job_type): spawn Approver
                match self.handle_spawn_supervisor(task_id, WorkerRole::Approver) {
                    Ok(sup_id) => result.spawned_supervisors.push(sup_id),
                    Err(e) => {
                        tracing::error!(error = %e, task_id = %task_id, "failed to spawn supervisor");
                    }
                }
                task_store.update_task_status(task_id, TaskStatus::InProgress)?;
            }

            // Resolve which children can become Ready
            let child_ids: Vec<TaskId> = children.iter().map(|c| c.id.clone()).collect();
            let ready_ids = task_engine.resolve_ready_tasks(&child_ids);
            for ready_id in &ready_ids {
                tracing::info!(task_id = %ready_id, status = ?TaskStatus::Ready, "task status cascaded");
                self.activate_ready_task(ready_id, task_store, task_engine, result)?;
            }
        }

        Ok(())
    }

    /// Create a Job for a task and assign it (spawn member).
    fn create_and_assign_job(
        &self,
        task: &palette_domain::task::Task,
        result: &mut EffectResult,
    ) -> crate::Result<()> {
        let req = task
            .to_create_job_request()
            .map_err(|e| crate::Error::InvalidTaskState {
                task_id: task.id.clone(),
                detail: e.reason_key(),
            })?;
        let job = self.interactor.data_store.create_job(&req)?;

        tracing::info!(
            job_id = %job.id,
            task_id = %task.id,
            job_type = ?job.job_type,
            "created job for ready task"
        );

        self.assign_new_job(&job.id, &mut result.deliveries)?;
        Ok(())
    }

    /// Create a Job for a task without assigning a member.
    fn create_job_without_assign(&self, task: &palette_domain::task::Task) -> crate::Result<()> {
        let req = task
            .to_create_job_request()
            .map_err(|e| crate::Error::InvalidTaskState {
                task_id: task.id.clone(),
                detail: e.reason_key(),
            })?;
        let job = self.interactor.data_store.create_job(&req)?;

        tracing::info!(
            job_id = %job.id,
            task_id = %task.id,
            job_type = ?job.job_type,
            "created job for ready task (no member assignment)"
        );

        Ok(())
    }

    /// Walk up the task tree to find the nearest supervisor for a job's task.
    pub(in crate::orchestrator) fn find_supervisor_for_job(
        &self,
        task_id: &TaskId,
    ) -> crate::Result<palette_domain::worker::WorkerId> {
        let task_state = self
            .interactor
            .data_store
            .get_task_state(task_id)?
            .ok_or_else(|| crate::Error::TaskNotFound {
                task_id: task_id.clone(),
            })?;
        let task_store = self.interactor.create_task_store(&task_state.workflow_id)?;

        let mut current_id = task_id.clone();
        loop {
            if let Ok(Some(sup)) = self
                .interactor
                .data_store
                .find_supervisor_for_task(&current_id)
            {
                return Ok(sup.id.clone());
            }
            let task =
                task_store
                    .get_task(&current_id)
                    .ok_or_else(|| crate::Error::TaskNotFound {
                        task_id: current_id.clone(),
                    })?;
            match task.parent_id {
                Some(ref pid) => current_id = pid.clone(),
                None => break,
            }
        }
        Err(crate::Error::InvalidTaskState {
            task_id: task_id.clone(),
            detail: "no supervisor found in task ancestry".into(),
        })
    }
}
