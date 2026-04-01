use super::Orchestrator;
use palette_core::ReasonKey;
use palette_domain::job::{JobId, JobType};
use palette_domain::rule::RuleEffect;
use palette_domain::task::{TaskId, TaskStatus};
use palette_domain::worker::WorkerRole;
use palette_usecase::RuleEngine;
use palette_usecase::task_store::TaskStore;

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

        let task_store = self.interactor.create_task_store(&task_state.workflow_id)?;

        // Check if all children are Completed (if any)
        let children = task_store.get_child_tasks(task_id);
        let all_children_completed = children.iter().all(|c| c.status == TaskStatus::Completed);

        if !all_children_completed && !children.is_empty() {
            return Ok(vec![]);
        }

        // All conditions met: mark task as Completed
        task_store.update_task_status(task_id, TaskStatus::Completed)?;
        tracing::info!(task_id = %task_id, "task completed (job done + all children completed)");

        let mut effects = Vec::new();

        // Destroy all supervisors for this task (e.g. review-integrate tasks
        // may have both an Approver and a ReviewIntegrator)
        if let Ok(sups) = self
            .interactor
            .data_store
            .find_supervisors_for_task(task_id)
        {
            for sup in sups {
                effects.push(RuleEffect::DestroySupervisor {
                    supervisor_id: sup.id.clone(),
                });
            }
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
        task_store: &TaskStore,
    ) -> crate::Result<Vec<RuleEffect>> {
        use palette_domain::rule::TaskEffect;
        use palette_usecase::TaskRuleEngine;

        let task_engine = TaskRuleEngine::new(task_store);
        let mut pending = task_engine.on_task_completed(completed_task_id);
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
                                job_effects.push(RuleEffect::SpawnSupervisor {
                                    task_id: task_id.clone(),
                                    role: WorkerRole::ReviewIntegrator,
                                });
                            } else {
                                tracing::info!(
                                    task_id = %task_id,
                                    "all children completed but own job not done; deferring task completion"
                                );
                            }
                            continue;
                        }

                        task_store.update_task_status(task_id, *new_status)?;
                        tracing::info!(task_id = %task_id, status = ?new_status, "task status cascaded");

                        // Destroy all supervisors for this composite task
                        if let Ok(sups) = self
                            .interactor
                            .data_store
                            .find_supervisors_for_task(task_id)
                        {
                            for sup in sups {
                                job_effects.push(RuleEffect::DestroySupervisor {
                                    supervisor_id: sup.id.clone(),
                                });
                            }
                        }

                        // Check workflow completion
                        if let Some(task) = task_store.get_task(task_id)
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
                        let effects = task_engine.on_task_completed(task_id);
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
        task_store: &TaskStore,
        task_engine: &palette_usecase::TaskRuleEngine<'_>,
    ) -> crate::Result<(Vec<palette_domain::rule::TaskEffect>, Vec<RuleEffect>)> {
        let children = task_store.get_child_tasks(task_id);

        if children.is_empty() {
            // Leaf task: create a job if it has a job_type
            let job_effects = if let Some(mut task) = task_store.get_task(task_id)
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
                self.create_job_for_ready_task(&task)?
            } else {
                vec![]
            };
            Ok((vec![], job_effects))
        } else {
            let mut job_effects = Vec::new();

            if let Some(task) = task_store.get_task(task_id)
                && let Some(job_type) = task.job_type
            {
                task_store.update_task_status(task_id, TaskStatus::InProgress)?;
                let effects = self.create_job_for_ready_task(&task)?;

                match job_type {
                    // Craft composites: create job + member, do NOT resolve children
                    // (activated later on InReview).
                    JobType::Craft => {
                        job_effects.extend(effects);
                        return Ok((vec![], job_effects));
                    }
                    // ReviewIntegrate composites: create job (verdict anchor) but do NOT
                    // assign a member. The ReviewIntegrator is spawned after all child
                    // reviewers complete.
                    JobType::ReviewIntegrate => {
                        let filtered: Vec<_> = effects
                            .into_iter()
                            .filter(|e| !matches!(e, RuleEffect::AssignNewJob { .. }))
                            .collect();
                        job_effects.extend(filtered);
                    }
                    _ => {
                        job_effects.extend(effects);
                    }
                }
            } else {
                // Pure composite task (no job_type): spawn Approver
                job_effects.push(RuleEffect::SpawnSupervisor {
                    task_id: task_id.clone(),
                    role: WorkerRole::Approver,
                });
                task_store.update_task_status(task_id, TaskStatus::InProgress)?;
            }

            // Resolve which children can become Ready
            let child_ids: Vec<TaskId> = children.iter().map(|c| c.id.clone()).collect();
            let task_effects = task_engine.resolve_ready_tasks(&child_ids);
            Ok((task_effects, job_effects))
        }
    }

    /// Create a Job for a task that just became Ready.
    fn create_job_for_ready_task(
        &self,
        task: &palette_domain::task::Task,
    ) -> crate::Result<Vec<RuleEffect>> {
        let req = task
            .to_create_job_request()
            .map_err(|e| crate::Error::InvalidTaskState {
                task_id: task.id.clone(),
                detail: e.reason_key(),
            })?;
        let job = self.interactor.data_store.create_job(&req)?;
        let effects =
            RuleEngine::new(self.interactor.data_store.as_ref(), 0).on_job_created(&job.id)?;

        tracing::info!(
            job_id = %job.id,
            task_id = %task.id,
            job_type = ?job.job_type,
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
