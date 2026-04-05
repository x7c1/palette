use super::Orchestrator;
use super::PendingActions;
use palette_core::ReasonKey;
use palette_domain::job::{JobDetail, JobType};
use palette_domain::task::{TaskId, TaskStatus};
use palette_domain::worker::WorkerRole;
use palette_usecase::task_store::TaskStore;

impl Orchestrator {
    /// Handle a task that just became Ready.
    /// Leaf tasks get a Job created; composite tasks with no job resolve their children.
    pub(crate) fn activate_ready_task(
        &self,
        task_id: &TaskId,
        task_store: &TaskStore,
        task_engine: &palette_usecase::TaskRuleEngine<'_>,
    ) -> crate::Result<PendingActions> {
        let children = task_store.get_child_tasks(task_id);

        if children.is_empty() {
            return self.activate_leaf_task(task_id, task_store);
        }

        let (result, resolve_children) = self.activate_composite_task(task_id, task_store)?;
        if !resolve_children {
            return Ok(result);
        }

        // Resolve which children can become Ready
        let child_ids: Vec<TaskId> = children.iter().map(|c| c.id.clone()).collect();
        let ready_ids = task_engine.resolve_ready_tasks(&child_ids);
        let mut result = result;
        for ready_id in &ready_ids {
            tracing::info!(task_id = %ready_id, status = ?TaskStatus::Ready, "task status cascaded");
            result = result.merge(self.activate_ready_task(ready_id, task_store, task_engine)?);
        }

        Ok(result)
    }

    /// Activate a leaf task (no children): create a job if it has a job_detail.
    fn activate_leaf_task(
        &self,
        task_id: &TaskId,
        task_store: &TaskStore,
    ) -> crate::Result<PendingActions> {
        let Some(mut task) = task_store.get_task(task_id) else {
            return Ok(PendingActions::new());
        };
        if task.job_detail.is_none() {
            return Ok(PendingActions::new());
        }

        // For review tasks, inherit plan_path from parent craft task
        if matches!(task.job_detail, Some(JobDetail::Review))
            && task.plan_path.is_none()
            && let Some(ref parent_id) = task.parent_id
            && let Some(parent) = task_store.get_task(parent_id)
        {
            task.plan_path = parent.plan_path.clone();
        }

        self.create_and_assign_job(&task)
    }

    /// Activate a composite task (has children): create job and/or spawn supervisor.
    ///
    /// Returns `(actions, resolve_children)`. When `resolve_children` is false
    /// (e.g. Craft composites), the caller must skip child resolution — children
    /// are activated later when the craft reaches InReview.
    fn activate_composite_task(
        &self,
        task_id: &TaskId,
        task_store: &TaskStore,
    ) -> crate::Result<(PendingActions, bool)> {
        let Some(task) = task_store.get_task(task_id) else {
            return Ok((PendingActions::new(), false));
        };

        let Some(ref job_detail) = task.job_detail else {
            // Pure composite task (no job_detail): spawn Approver
            let mut result = PendingActions::new();
            match self.handle_spawn_supervisor(task_id, WorkerRole::Approver) {
                Ok(sup_id) => result.watch_only.push(sup_id),
                Err(e) => {
                    tracing::error!(error = %e, task_id = %task_id, "failed to spawn supervisor");
                }
            }
            task_store.update_task_status(task_id, TaskStatus::InProgress)?;
            return Ok((result, true));
        };

        task_store.update_task_status(task_id, TaskStatus::InProgress)?;

        match job_detail.job_type() {
            // Craft composites: create job + member, do NOT resolve children.
            JobType::Craft => Ok((self.create_and_assign_job(&task)?, false)),
            // ReviewIntegrate composites: create job (verdict anchor) but do NOT
            // assign a member. The ReviewIntegrator is spawned after all child
            // reviewers complete.
            JobType::ReviewIntegrate => {
                self.create_job_without_assign(&task)?;
                Ok((PendingActions::new(), true))
            }
            _ => Ok((self.create_and_assign_job(&task)?, true)),
        }
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
            job_type = ?job.detail.job_type(),
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
            job_type = ?job.detail.job_type(),
            "created job for ready task (no member assignment)"
        );

        Ok(())
    }
}
