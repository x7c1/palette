use super::Orchestrator;
use palette_domain::job::{JobId, JobType};
use palette_domain::rule::{RuleEffect, RuleEngine};
use palette_domain::task::{TaskId, TaskStatus, TaskStore};
use palette_domain::worker::WorkerRole;
use palette_usecase::task_store::TaskStoreImpl;

impl Orchestrator {
    /// When a Job is Done, check if its task can be completed and cascade.
    pub(super) fn complete_job(&self, job_id: &JobId) -> crate::Result<Vec<RuleEffect>> {
        self.try_complete_task_by_job(job_id)
    }

    /// Check if a job's task can be completed.
    /// A task is complete when all children are Completed AND its own job (if any) is Done.
    pub(super) fn try_complete_task_by_job(
        &self,
        job_id: &JobId,
    ) -> crate::Result<Vec<RuleEffect>> {
        let Some(job) = self.interactor.data_store.get_job(job_id)? else {
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

        // Check if all children are Completed (if any)
        let children = task_store.get_child_tasks(task_id)?;
        let all_children_completed = children.iter().all(|c| c.status == TaskStatus::Completed);

        if !all_children_completed && !children.is_empty() {
            return Ok(vec![]);
        }

        // All conditions met: mark task as Completed
        task_store.update_task_status(task_id, TaskStatus::Completed)?;
        tracing::info!(task_id = %task_id, "task completed (job done + all children completed)");

        let mut effects = Vec::new();

        // Destroy supervisor for this task if it had one
        if let Ok(Some(sup)) = self.interactor.data_store.find_supervisor_for_task(task_id) {
            effects.push(RuleEffect::DestroySupervisor {
                supervisor_id: sup.id.clone(),
            });
        }

        let cascade = self.cascade_task_effects(task_id, &task_store)?;
        effects.extend(cascade);

        // Fill vacant member slots with waiting jobs
        effects.extend(self.fill_vacant_slots()?);

        Ok(effects)
    }

    /// Find assignable jobs waiting for a member slot and emit AssignNewJob effects.
    fn fill_vacant_slots(&self) -> crate::Result<Vec<RuleEffect>> {
        let assignable = self.interactor.data_store.find_assignable_jobs()?;
        Ok(assignable
            .into_iter()
            .map(|j| RuleEffect::AssignNewJob { job_id: j.id })
            .collect())
    }

    /// Process cascading effects after a task completes.
    fn cascade_task_effects(
        &self,
        completed_task_id: &TaskId,
        task_store: &TaskStoreImpl,
    ) -> crate::Result<Vec<RuleEffect>> {
        use palette_domain::rule::{TaskEffect, TaskRuleEngine};

        let task_engine = TaskRuleEngine::new(task_store);
        let mut pending = task_engine.on_task_completed(completed_task_id)?;
        let mut job_effects = Vec::new();

        while !pending.is_empty() {
            let mut next = Vec::new();
            for effect in &pending {
                let TaskEffect::TaskStatusChanged {
                    task_id,
                    new_status,
                } = effect
                else {
                    continue;
                };

                match new_status {
                    TaskStatus::Ready => {
                        task_store.update_task_status(task_id, *new_status)?;
                        tracing::info!(task_id = %task_id, status = ?new_status, "task status cascaded");
                        let (follow_up, new_job_effects) =
                            self.activate_ready_task(task_id, task_store, &task_engine)?;
                        next.extend(follow_up);
                        job_effects.extend(new_job_effects);
                    }
                    TaskStatus::Completed => {
                        // Before marking parent as Completed, check its own Job (if any)
                        let own_job_done = self
                            .interactor
                            .data_store
                            .get_job_by_task_id(task_id)?
                            .is_none_or(|j| j.status.is_done());

                        if !own_job_done {
                            tracing::info!(
                                task_id = %task_id,
                                "all children completed but own job not done; deferring task completion"
                            );
                            continue;
                        }

                        task_store.update_task_status(task_id, *new_status)?;
                        tracing::info!(task_id = %task_id, status = ?new_status, "task status cascaded");

                        // Destroy dynamic supervisor if this composite task had one
                        if let Ok(Some(sup)) =
                            self.interactor.data_store.find_supervisor_for_task(task_id)
                        {
                            job_effects.push(RuleEffect::DestroySupervisor {
                                supervisor_id: sup.id.clone(),
                            });
                        }

                        // Check workflow completion
                        if let Some(task) = task_store.get_task(task_id)?
                            && task.parent_id.is_none()
                        {
                            use palette_domain::workflow::WorkflowStatus;
                            self.interactor.data_store.update_workflow_status(
                                &task.workflow_id,
                                WorkflowStatus::Completed,
                            )?;
                            tracing::info!(
                                workflow_id = %task.workflow_id,
                                "workflow completed"
                            );
                        }
                        let effects = task_engine.on_task_completed(task_id)?;
                        next.extend(effects);
                    }
                    _ => {}
                }
            }
            pending = next;
        }

        Ok(job_effects)
    }

    /// Handle a task that just became Ready.
    /// Leaf tasks get a Job created; composite tasks with no job resolve their children.
    pub(super) fn activate_ready_task(
        &self,
        task_id: &TaskId,
        task_store: &TaskStoreImpl,
        task_engine: &palette_domain::rule::TaskRuleEngine<&TaskStoreImpl>,
    ) -> crate::Result<(Vec<palette_domain::rule::TaskEffect>, Vec<RuleEffect>)> {
        let children = task_store.get_child_tasks(task_id)?;

        if children.is_empty() {
            // Leaf task: create a job if it has a job_type
            let job_effects = if let Some(mut task) = task_store.get_task(task_id)?
                && task.job_type.is_some()
            {
                // For review tasks, inherit plan_path from parent craft task
                if task.job_type == Some(JobType::Review)
                    && task.plan_path.is_none()
                    && let Some(ref parent_id) = task.parent_id
                    && let Some(parent) = task_store.get_task(parent_id)?
                {
                    task.plan_path = parent.plan_path.clone();
                }
                self.create_job_for_ready_task(&task)?
            } else {
                vec![]
            };
            Ok((vec![], job_effects))
        } else {
            let mut job_effects = Vec::new();

            // If composite task has a job_type, create the job.
            // Craft composites: do NOT resolve children (activated on InReview).
            // Review composites (review-integrate): resolve children immediately.
            if let Some(task) = task_store.get_task(task_id)?
                && task.job_type.is_some()
            {
                task_store.update_task_status(task_id, TaskStatus::InProgress)?;
                let effects = self.create_job_for_ready_task(&task)?;
                job_effects.extend(effects);

                if task.job_type == Some(JobType::Craft) {
                    return Ok((vec![], job_effects));
                }
                // Review composite: fall through to resolve children
            }

            // Pure composite task (no job_type): spawn supervisor before InProgress
            job_effects.push(RuleEffect::SpawnSupervisor {
                task_id: task_id.clone(),
                role: WorkerRole::Leader,
            });
            task_store.update_task_status(task_id, TaskStatus::InProgress)?;

            // Resolve which children can become Ready
            let child_ids: Vec<TaskId> = children.iter().map(|c| c.id.clone()).collect();
            let task_effects = task_engine.resolve_ready_tasks(&child_ids)?;
            Ok((task_effects, job_effects))
        }
    }

    /// Create a Job for a task that just became Ready.
    fn create_job_for_ready_task(
        &self,
        task: &palette_domain::task::Task,
    ) -> crate::Result<Vec<RuleEffect>> {
        let job_type = task.job_type.expect("task must have job_type");
        let job =
            self.interactor
                .data_store
                .create_job(&palette_domain::job::CreateJobRequest {
                    id: Some(JobId::generate(job_type)),
                    task_id: task.id.clone(),
                    job_type,
                    title: task.key.to_string(),
                    plan_path: task.plan_path.clone().unwrap_or_default(),
                    assignee_id: None,
                    priority: task.priority,
                    repository: task.repository.clone(),
                })?;

        let rules = RuleEngine::new(self.interactor.data_store.as_ref(), 0);
        let effects = rules.on_job_created(&job.id)?;

        tracing::info!(
            job_id = %job.id,
            task_id = %task.id,
            job_type = ?job_type,
            "created job for ready task (cascade)"
        );
        Ok(effects)
    }

    /// Walk up the task tree to find the nearest supervisor for a job's task.
    pub(super) fn find_supervisor_for_job(
        &self,
        task_id: &TaskId,
    ) -> crate::Result<palette_domain::worker::WorkerId> {
        let task_state = self
            .interactor
            .data_store
            .get_task_state(task_id)?
            .ok_or_else(|| crate::Error::Internal(format!("task not found: {task_id}")))?;
        let task_store = self
            .interactor
            .create_task_store(&task_state.workflow_id)
            .map_err(|e| crate::Error::Internal(e.to_string()))?;

        let mut current_id = task_id.clone();
        loop {
            if let Ok(Some(sup)) = self
                .interactor
                .data_store
                .find_supervisor_for_task(&current_id)
            {
                return Ok(sup.id.clone());
            }
            let task = task_store
                .get_task(&current_id)?
                .ok_or_else(|| crate::Error::Internal(format!("task not found: {current_id}")))?;
            match task.parent_id {
                Some(ref pid) => current_id = pid.clone(),
                None => break,
            }
        }
        Err(crate::Error::Internal(format!(
            "no supervisor found for task {task_id}"
        )))
    }
}
