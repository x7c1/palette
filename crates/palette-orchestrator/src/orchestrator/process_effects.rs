use super::Orchestrator;
use palette_docker::WorkspaceVolume;
use palette_domain::agent::AgentId;
use palette_domain::job::{CraftStatus, Job, JobId, JobStatus, JobType, ReviewStatus};
use palette_domain::rule::{RuleEffect, RuleEngine};
use palette_domain::server::{PendingDelivery, PersistentState};
use palette_domain::task::{TaskId, TaskStatus, TaskStore};
use palette_service::TaskStoreImpl;

impl Orchestrator {
    /// Processes rule engine effects: auto-assign jobs, spawn/destroy members.
    /// Returns a list of messages that need to be sent to members via tmux.
    ///
    /// The caller is responsible for saving state after this function returns.
    pub(super) fn process_effects(
        &self,
        effects: &[RuleEffect],
        infra: &mut PersistentState,
    ) -> crate::Result<Vec<PendingDelivery>> {
        let mut deliveries = Vec::new();
        let mut pending: Vec<RuleEffect> = effects.to_vec();

        while let Some(effect) = pending.pop() {
            match &effect {
                RuleEffect::AutoAssign { job_id } => {
                    self.handle_auto_assign(job_id, infra, &mut deliveries)?;
                }
                RuleEffect::DestroyMember { member_id } => {
                    self.handle_destroy_member(member_id, infra);
                }
                RuleEffect::StatusChanged { job_id, new_status } => {
                    let chained = self.handle_status_changed(job_id, *new_status)?;
                    pending.extend(chained);
                }
                _ => {}
            }
        }

        Ok(deliveries)
    }

    fn handle_auto_assign(
        &self,
        job_id: &JobId,
        infra: &mut PersistentState,
        deliveries: &mut Vec<PendingDelivery>,
    ) -> crate::Result<()> {
        let Some(job) = self.db.get_job(job_id)? else {
            return Ok(());
        };

        // Re-review: job already has an assignee (e.g. reviewer from previous round).
        // Deliver a new instruction to the existing member instead of spawning a new one.
        if let Some(ref existing_assignee) = job.assignee
            && let Some(member) = infra.find_member(existing_assignee)
        {
            let instruction = format_job_instruction(&job);
            self.db.enqueue_message(existing_assignee, &instruction)?;
            let in_progress = match job.job_type {
                JobType::Craft => JobStatus::Craft(CraftStatus::InProgress),
                JobType::Review => JobStatus::Review(ReviewStatus::InProgress),
            };
            self.db.update_job_status(job_id, in_progress)?;
            deliveries.push(PendingDelivery {
                target_id: existing_assignee.clone(),
                terminal_target: member.terminal_target.clone(),
            });
            tracing::info!(
                job_id = %job_id,
                member_id = %existing_assignee,
                "re-assigned job to existing member"
            );
            return Ok(());
        }

        // New assignment: verify the job is assignable (todo + no assignee)
        let assignable_jobs = self.db.find_assignable_jobs()?;
        let job = match assignable_jobs.iter().find(|j| j.id == *job_id) {
            Some(j) => j.clone(),
            None => return Ok(()),
        };
        let active = self.db.count_active_members()?;
        if active >= self.docker_config.max_members {
            tracing::info!(
                job_id = %job_id,
                active = active,
                max = self.docker_config.max_members,
                "max members reached, job waits"
            );
            return Ok(());
        }

        // Determine workspace volume based on job type
        let workspace = self.resolve_workspace(&job)?;

        // Spawn a new member with supervisor_id based on job type
        let task_state = self
            .db
            .get_task_state(&job.task_id)?
            .ok_or_else(|| crate::Error::Internal(format!("task not found: {}", job.task_id)))?;
        let seq = self.db.increment_member_counter(&task_state.workflow_id)?;
        let member_id = AgentId::next_member(seq);
        let member = self.spawn_member(&member_id, job.job_type, infra, workspace)?;
        let terminal_target = member.terminal_target.clone();
        infra.members.push(member);

        // Assign job
        self.db.assign_job(job_id, &member_id, job.job_type)?;
        tracing::info!(
            job_id = %job_id,
            member_id = %member_id,
            "auto-assigned job"
        );

        // Build job instruction message
        let instruction = format_job_instruction(&job);
        self.db.enqueue_message(&member_id, &instruction)?;

        deliveries.push(PendingDelivery {
            target_id: member_id,
            terminal_target,
        });

        infra.touch();
        Ok(())
    }

    fn handle_destroy_member(&self, member_id: &AgentId, infra: &mut PersistentState) {
        if let Some(member) = infra.remove_member(member_id) {
            tracing::info!(member_id = %member_id, "destroying member container");
            let _ = self.docker.stop_container(&member.container_id);
            let _ = self.docker.remove_container(&member.container_id);
            infra.touch();
        }
    }

    /// Determine the workspace volume for a job.
    /// Craft jobs get a new volume; review jobs share the parent craft job's volume (read-only).
    fn resolve_workspace(&self, job: &Job) -> crate::Result<Option<WorkspaceVolume>> {
        match job.job_type {
            JobType::Craft => Ok(Some(WorkspaceVolume {
                name: format!("palette-workspace-{}", job.id),
                read_only: false,
            })),
            JobType::Review => {
                // Review is a child of craft, so find the parent task's craft job
                let task_id = &job.task_id;
                let Some(task_state) = self.db.get_task_state(task_id)? else {
                    return Ok(None);
                };
                let task_store = TaskStoreImpl::from_db(&self.db, &task_state.workflow_id)
                    .map_err(|e| crate::Error::Internal(e.to_string()))?;
                let Some(task) = task_store.get_task(task_id)? else {
                    return Ok(None);
                };
                let Some(ref parent_id) = task.parent_id else {
                    return Ok(None);
                };
                let Some(craft_job) = self.db.get_job_by_task_id(parent_id)? else {
                    return Ok(None);
                };
                Ok(Some(WorkspaceVolume {
                    name: format!("palette-workspace-{}", craft_job.id),
                    read_only: true,
                }))
            }
        }
    }

    fn handle_status_changed(
        &self,
        job_id: &JobId,
        new_status: JobStatus,
    ) -> crate::Result<Vec<RuleEffect>> {
        let rules = RuleEngine::new(&*self.db, 0);
        let chained = rules.on_status_change(job_id, new_status)?;
        for e in &chained {
            tracing::info!(?e, "chained rule engine effect");
        }

        let mut all_effects = chained;

        // When a craft job reaches InReview, activate child review tasks.
        // On re-review (after changes_requested), reactivate ChangesRequested review jobs.
        if matches!(new_status, JobStatus::Craft(CraftStatus::InReview)) {
            let effects = self.activate_child_review_tasks(job_id)?;
            all_effects.extend(effects);
        }

        // When a review job gets ChangesRequested, move parent craft job back to InProgress
        if matches!(
            new_status,
            JobStatus::Review(ReviewStatus::ChangesRequested)
        ) {
            let effects = self.revert_parent_craft_to_in_progress(job_id)?;
            all_effects.extend(effects);
        }

        // When a review job is Done, check if the parent craft job should also become Done
        if matches!(new_status, JobStatus::Review(ReviewStatus::Done)) {
            let effects = self.try_complete_parent_craft_job(job_id)?;
            all_effects.extend(effects);
        }

        // When a job is Done, try to complete its task (checking all conditions)
        if new_status.is_done() {
            let effects = self.try_complete_task(job_id)?;
            all_effects.extend(effects);
        }

        Ok(all_effects)
    }

    /// When a Craft Job reaches InReview, activate its child review tasks.
    /// The craft task stays InProgress; review tasks become Ready.
    fn activate_child_review_tasks(&self, craft_job_id: &JobId) -> crate::Result<Vec<RuleEffect>> {
        let Some(job) = self.db.get_job(craft_job_id)? else {
            return Ok(vec![]);
        };
        let task_id = &job.task_id;
        let Some(task_state) = self.db.get_task_state(task_id)? else {
            return Ok(vec![]);
        };

        let task_store = TaskStoreImpl::from_db(&self.db, &task_state.workflow_id)
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
                    let (follow_up, effects) =
                        self.activate_ready_task(task_id, &task_store, &task_engine)?;
                    next.extend(follow_up);
                    job_effects.extend(effects);
                }
            }
            pending = next;
        }

        // Second pass: reactivate ChangesRequested review jobs (re-review cycle).
        // Only reactivate if the review job is in ChangesRequested state.
        // If it's already InProgress (reviewer still working) or Todo, skip it.
        for child in &children {
            let Some(review_job) = self.db.get_job_by_task_id(&child.id)? else {
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
            self.db
                .update_job_status(&review_job.id, JobStatus::Review(ReviewStatus::InProgress))?;
            tracing::info!(
                job_id = %review_job.id,
                task_id = %child.id,
                "reactivated ChangesRequested review job for re-review"
            );
            job_effects.push(RuleEffect::AutoAssign {
                job_id: review_job.id.clone(),
            });
        }

        Ok(job_effects)
    }

    /// When a review job gets ChangesRequested, move the parent craft job
    /// from InReview back to InProgress so the crafter can address feedback.
    fn revert_parent_craft_to_in_progress(
        &self,
        review_job_id: &JobId,
    ) -> crate::Result<Vec<RuleEffect>> {
        let Some(review_job) = self.db.get_job(review_job_id)? else {
            return Ok(vec![]);
        };
        let review_task_id = &review_job.task_id;
        let Some(task_state) = self.db.get_task_state(review_task_id)? else {
            return Ok(vec![]);
        };

        let task_store = TaskStoreImpl::from_db(&self.db, &task_state.workflow_id)
            .map_err(|e| crate::Error::Internal(e.to_string()))?;

        let Some(review_task) = task_store.get_task(review_task_id)? else {
            return Ok(vec![]);
        };

        // Review task must have a parent (the craft task)
        let Some(ref parent_id) = review_task.parent_id else {
            return Ok(vec![]);
        };

        // Parent must have a craft job in InReview
        let Some(craft_job) = self.db.get_job_by_task_id(parent_id)? else {
            return Ok(vec![]);
        };
        if craft_job.status != JobStatus::Craft(CraftStatus::InReview) {
            return Ok(vec![]);
        }

        // Move craft job back to InProgress
        self.db
            .update_job_status(&craft_job.id, JobStatus::Craft(CraftStatus::InProgress))?;
        tracing::info!(
            craft_job_id = %craft_job.id,
            review_job_id = %review_job_id,
            "craft job reverted to InProgress due to changes_requested"
        );

        // Enqueue review feedback to the crafter so they know what to fix
        if let Some(ref assignee) = craft_job.assignee {
            let submissions = self.db.get_review_submissions(review_job_id)?;
            let feedback = submissions
                .last()
                .and_then(|s| s.summary.clone())
                .unwrap_or_else(|| "Changes requested (no summary provided)".to_string());
            let msg = format!(
                "## Review Feedback (changes requested)\n\nReview job {} has requested changes:\n\n{}\n\nPlease address the feedback and complete the task.",
                review_job_id, feedback
            );
            self.db.enqueue_message(assignee, &msg)?;
        }

        // Emit AutoAssign so the crafter gets re-activated
        Ok(vec![RuleEffect::AutoAssign {
            job_id: craft_job.id,
        }])
    }

    /// When a Job is Done, check if its task can be completed.
    /// A task is complete when all children are Completed AND its own job (if any) is Done.
    fn try_complete_task(&self, job_id: &JobId) -> crate::Result<Vec<RuleEffect>> {
        let Some(job) = self.db.get_job(job_id)? else {
            return Ok(vec![]);
        };
        let task_id = &job.task_id;
        let Some(task_state) = self.db.get_task_state(task_id)? else {
            return Ok(vec![]);
        };

        let task_store = TaskStoreImpl::from_db(&self.db, &task_state.workflow_id)
            .map_err(|e| crate::Error::Internal(e.to_string()))?;

        // Check if all children are Completed (if any)
        let children = task_store.get_child_tasks(task_id)?;
        let all_children_completed = children.iter().all(|c| c.status == TaskStatus::Completed);

        if !all_children_completed && !children.is_empty() {
            // Children not done yet; task stays InProgress
            return Ok(vec![]);
        }

        // All conditions met: mark task as Completed
        task_store.update_task_status(task_id, TaskStatus::Completed)?;
        tracing::info!(task_id = %task_id, "task completed (job done + all children completed)");

        self.cascade_task_effects(task_id, &task_store)
    }

    /// When a review job becomes Done, check if all sibling review tasks under
    /// the parent craft task are also done. If so, transition the parent craft job
    /// from InReview to Done.
    fn try_complete_parent_craft_job(
        &self,
        review_job_id: &JobId,
    ) -> crate::Result<Vec<RuleEffect>> {
        let Some(review_job) = self.db.get_job(review_job_id)? else {
            return Ok(vec![]);
        };
        let review_task_id = &review_job.task_id;
        let Some(task_state) = self.db.get_task_state(review_task_id)? else {
            return Ok(vec![]);
        };

        let task_store = TaskStoreImpl::from_db(&self.db, &task_state.workflow_id)
            .map_err(|e| crate::Error::Internal(e.to_string()))?;

        let Some(review_task) = task_store.get_task(review_task_id)? else {
            return Ok(vec![]);
        };

        // Review task must have a parent (the craft task)
        let Some(ref parent_id) = review_task.parent_id else {
            return Ok(vec![]);
        };

        // Parent must have a craft job in InReview
        let Some(craft_job) = self.db.get_job_by_task_id(parent_id)? else {
            return Ok(vec![]);
        };
        if craft_job.status != JobStatus::Craft(CraftStatus::InReview) {
            return Ok(vec![]);
        }

        // Check if ALL review children of the parent have their jobs Done
        let siblings = task_store.get_child_tasks(parent_id)?;
        let all_reviews_done = siblings.iter().all(|child| {
            if child.job_type != Some(JobType::Review) {
                return true;
            }
            self.db
                .get_job_by_task_id(&child.id)
                .ok()
                .flatten()
                .is_some_and(|j| j.status.is_done())
        });

        if !all_reviews_done {
            return Ok(vec![]);
        }

        // All review children are done — transition craft job to Done
        self.db
            .update_job_status(&craft_job.id, JobStatus::Craft(CraftStatus::Done))?;
        tracing::info!(
            craft_job_id = %craft_job.id,
            "craft job completed (all child reviews done)"
        );

        Ok(vec![RuleEffect::StatusChanged {
            job_id: craft_job.id,
            new_status: JobStatus::Craft(CraftStatus::Done),
        }])
    }

    /// Process cascading effects after a task completes.
    /// Status changes are written to DB immediately via TaskStoreImpl.
    fn cascade_task_effects(
        &self,
        completed_task_id: &TaskId,
        task_store: &TaskStoreImpl<'_>,
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
                            .db
                            .get_job_by_task_id(task_id)?
                            .is_none_or(|j| j.status.is_done());

                        if !own_job_done {
                            // Parent has a job that isn't done yet; skip completion
                            tracing::info!(
                                task_id = %task_id,
                                "all children completed but own job not done; deferring task completion"
                            );
                            continue;
                        }

                        task_store.update_task_status(task_id, *new_status)?;
                        tracing::info!(task_id = %task_id, status = ?new_status, "task status cascaded");

                        // Check workflow completion
                        if let Some(task) = task_store.get_task(task_id)?
                            && task.parent_id.is_none()
                        {
                            use palette_domain::workflow::WorkflowStatus;
                            self.db.update_workflow_status(
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
    /// Composite tasks with a job (e.g., craft tasks) also create their job.
    fn activate_ready_task(
        &self,
        task_id: &TaskId,
        task_store: &TaskStoreImpl<'_>,
        task_engine: &palette_domain::rule::TaskRuleEngine<&TaskStoreImpl<'_>>,
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
            // Composite task: transition to InProgress
            task_store.update_task_status(task_id, TaskStatus::InProgress)?;

            let mut job_effects = Vec::new();

            // If composite task has a job_type (e.g., craft), create the job
            // but do NOT resolve children — they are activated later
            // (e.g., review children activate when craft job reaches InReview)
            if let Some(task) = task_store.get_task(task_id)?
                && task.job_type.is_some()
            {
                let effects = self.create_job_for_ready_task(&task)?;
                job_effects.extend(effects);
                return Ok((vec![], job_effects));
            }

            // Pure composite task (no job_type): resolve which children can become Ready
            let child_ids: Vec<TaskId> = children.iter().map(|c| c.id.clone()).collect();
            let task_effects = task_engine.resolve_ready_tasks(&child_ids)?;
            Ok((task_effects, job_effects))
        }
    }

    /// Create a Job for a task that just became Ready.
    /// Returns RuleEffects (e.g. AutoAssign) that should be processed by the caller.
    fn create_job_for_ready_task(
        &self,
        task: &palette_domain::task::Task,
    ) -> crate::Result<Vec<RuleEffect>> {
        let job_type = task.job_type.expect("task must have job_type");
        let job = self.db.create_job(&palette_domain::job::CreateJobRequest {
            id: Some(JobId::generate(job_type)),
            task_id: task.id.clone(),
            job_type,
            title: task
                .id
                .as_ref()
                .rsplit('/')
                .next()
                .unwrap_or("task")
                .to_string(),
            plan_path: task.plan_path.clone().unwrap_or_default(),
            description: task.description.clone(),
            assignee: None,
            priority: task.priority,
            repository: task.repository.clone(),
        })?;

        // Job is already created as Todo; trigger auto-assign
        let todo_status = JobStatus::todo(job_type);
        let rules = RuleEngine::new(&*self.db, 0);
        let effects = rules.on_status_change(&job.id, todo_status)?;

        tracing::info!(
            job_id = %job.id,
            task_id = %task.id,
            job_type = ?job_type,
            "created job for ready task (cascade)"
        );
        Ok(effects)
    }
}

/// Container-side mount point for the shared plan directory.
const PLAN_DIR_MOUNT: &str = "/home/agent/plans";

/// Format a job into an instruction message for a member.
fn format_job_instruction(job: &Job) -> String {
    let mut msg = format!(
        "## Task: {}\n\nID: {}\nPlan: {}/{}\n",
        job.title, job.id, PLAN_DIR_MOUNT, job.plan_path
    );
    if let Some(ref desc) = job.description {
        msg.push_str(&format!("\n{desc}\n"));
    }
    if let Some(ref repo) = job.repository {
        msg.push_str(&format!(
            "\nRepository: {} (branch: {})\n",
            repo.name, repo.branch
        ));
    }
    msg.push_str("\nPlease begin working on this task.");
    msg
}

#[cfg(test)]
mod tests {
    use palette_db::{CreateTaskRequest, Database};
    use palette_domain::agent::*;
    use palette_domain::review::*;
    use palette_domain::rule::*;
    use palette_domain::task::TaskId;
    use palette_domain::workflow::WorkflowId;

    use palette_domain::job::*;

    fn setup_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn jid(s: &str) -> JobId {
        JobId::new(s)
    }

    fn setup_task(db: &Database, task_id: &str) -> TaskId {
        let wf_id = WorkflowId::new(format!("wf-{task_id}"));
        let t_id = TaskId::new(task_id);
        let _ = db.create_workflow(&wf_id, "test/blueprint.yaml");
        let _ = db.create_task(&CreateTaskRequest {
            id: t_id.clone(),
            workflow_id: wf_id,
        });
        t_id
    }

    #[test]
    fn review_todo_triggers_auto_assign() {
        let db = setup_db();
        let task_id = setup_task(&db, "task-R-001");
        db.create_job(&CreateJobRequest {
            task_id,
            id: Some(jid("R-001")),
            job_type: JobType::Review,
            title: "Review".to_string(),
            plan_path: "test/R-001".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
        })
        .unwrap();

        let todo = JobStatus::Review(ReviewStatus::Todo);
        let engine = RuleEngine::new(&db, 5);
        let effects = engine.on_status_change(&jid("R-001"), todo).unwrap();

        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            RuleEffect::AutoAssign {
                job_id: jid("R-001"),
            }
        );
    }

    #[test]
    fn approved_review_produces_done_and_destroy() {
        let db = setup_db();
        let task_id = setup_task(&db, "task-R-001");
        db.create_job(&CreateJobRequest {
            task_id,
            id: Some(jid("R-001")),
            job_type: JobType::Review,
            title: "Review".to_string(),
            plan_path: "test/R-001".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
        })
        .unwrap();

        db.assign_job(&jid("R-001"), &AgentId::new("member-b"), JobType::Review)
            .unwrap();

        let sub = db
            .submit_review(
                &jid("R-001"),
                &SubmitReviewRequest {
                    verdict: Verdict::Approved,
                    summary: Some("LGTM".to_string()),
                    comments: vec![],
                },
            )
            .unwrap();

        let engine = RuleEngine::new(&db, 5);
        let effects = engine.on_review_submitted(&jid("R-001"), &sub).unwrap();

        assert_eq!(effects.len(), 2);
        assert_eq!(
            effects[0],
            RuleEffect::StatusChanged {
                job_id: jid("R-001"),
                new_status: JobStatus::Review(ReviewStatus::Done),
            }
        );
        assert_eq!(
            effects[1],
            RuleEffect::DestroyMember {
                member_id: AgentId::new("member-b"),
            }
        );

        let review = db.get_job(&jid("R-001")).unwrap().unwrap();
        assert_eq!(review.status, JobStatus::Review(ReviewStatus::Done));
    }

    #[test]
    fn changes_requested_sets_review_status() {
        let db = setup_db();
        let task_id = setup_task(&db, "task-R-001");
        db.create_job(&CreateJobRequest {
            task_id,
            id: Some(jid("R-001")),
            job_type: JobType::Review,
            title: "Review".to_string(),
            plan_path: "test/R-001".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
        })
        .unwrap();

        db.assign_job(&jid("R-001"), &AgentId::new("member-b"), JobType::Review)
            .unwrap();

        let sub = db
            .submit_review(
                &jid("R-001"),
                &SubmitReviewRequest {
                    verdict: Verdict::ChangesRequested,
                    summary: None,
                    comments: vec![],
                },
            )
            .unwrap();

        let engine = RuleEngine::new(&db, 5);
        let effects = engine.on_review_submitted(&jid("R-001"), &sub).unwrap();

        // ChangesRequested now emits a StatusChanged effect
        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            RuleEffect::StatusChanged {
                job_id: jid("R-001"),
                new_status: JobStatus::Review(ReviewStatus::ChangesRequested),
            }
        );

        let review = db.get_job(&jid("R-001")).unwrap().unwrap();
        assert_eq!(
            review.status,
            JobStatus::Review(ReviewStatus::ChangesRequested)
        );
        assert_eq!(review.assignee, Some(AgentId::new("member-b")));
    }
}
