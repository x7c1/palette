use super::Orchestrator;
use super::PendingActions;
use palette_core::ReasonKey;
use palette_domain::job::JobType;
use palette_domain::task::{TaskId, TaskStatus};
use palette_domain::worker::WorkerRole;
use palette_usecase::task_store::TaskStore;

impl Orchestrator {
    /// Handle a task that just became Ready.
    /// Leaf tasks get a Job created; composite tasks with no job resolve their children.
    pub(in crate::orchestrator) fn activate_ready_task(
        &self,
        task_id: &TaskId,
        task_store: &TaskStore,
        task_engine: &palette_usecase::TaskRuleEngine<'_>,
    ) -> crate::Result<PendingActions> {
        let mut result = PendingActions::new();
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
                result = result.merge(self.create_and_assign_job(&task)?);
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
                        return self.create_and_assign_job(&task);
                    }
                    // ReviewIntegrate composites: create job (verdict anchor) but do NOT
                    // assign a member. The ReviewIntegrator is spawned after all child
                    // reviewers complete.
                    JobType::ReviewIntegrate => {
                        self.create_job_without_assign(&task)?;
                    }
                    _ => {
                        result = result.merge(self.create_and_assign_job(&task)?);
                    }
                }
            } else {
                // Pure composite task (no job_type): spawn Approver
                match self.handle_spawn_supervisor(task_id, WorkerRole::Approver) {
                    Ok(sup_id) => result.watch_only.push(sup_id),
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
                result =
                    result.merge(self.activate_ready_task(ready_id, task_store, task_engine)?);
            }
        }

        Ok(result)
    }

    /// Create a Job for a task and assign it (spawn member).
    fn create_and_assign_job(
        &self,
        task: &palette_domain::task::Task,
    ) -> crate::Result<PendingActions> {
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

        self.assign_new_job(&job.id)
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
}
