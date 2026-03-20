use super::Orchestrator;
use palette_docker::WorkspaceVolume;
use palette_domain::agent::AgentId;
use palette_domain::job::{Job, JobId, JobStatus, JobType};
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
            self.db.update_job_status(job_id, JobStatus::InProgress)?;
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

        // New assignment: verify the job is assignable (ready + all deps done, no assignee)
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
        let member_id = infra.next_member_id();
        let member = self.spawn_member(&member_id, job.job_type, infra, workspace)?;
        let terminal_target = member.terminal_target.clone();
        infra.members.push(member);

        // Assign job
        self.db.assign_job(job_id, &member_id)?;
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
    /// Craft jobs get a new volume; review jobs share the sibling craft job's volume (read-only).
    fn resolve_workspace(&self, job: &Job) -> crate::Result<Option<WorkspaceVolume>> {
        match job.job_type {
            JobType::Craft => Ok(Some(WorkspaceVolume {
                name: format!("palette-workspace-{}", job.id),
                read_only: false,
            })),
            JobType::Review => {
                let Some(ref task_id) = job.task_id else {
                    return Ok(None);
                };
                let Some(task_state) = self.db.get_task_state(task_id)? else {
                    return Ok(None);
                };
                let task_store = TaskStoreImpl::from_db(&self.db, &task_state.workflow_id)
                    .map_err(|e| crate::Error::Internal(e.to_string()))?;
                let Some(craft_node) = task_store.tree().sibling_craft(task_id) else {
                    return Ok(None);
                };
                let Some(craft_job) = self.db.get_job_by_task_id(&craft_node.id)? else {
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
        let rules = RuleEngine::new(&*self.db, 0); // max_review_rounds unused for status changes
        let chained = rules.on_status_change(job_id, new_status)?;
        for e in &chained {
            tracing::info!(?e, "chained rule engine effect");
        }

        // When a job completes, propagate to its task and collect any new job effects.
        // For craft jobs, `in_review` means the craft work is done — the review will be
        // handled by a separate review task in the task tree.
        let mut all_effects = chained;
        if new_status == JobStatus::Done || new_status == JobStatus::InReview {
            let task_job_effects = self.propagate_task_completion(job_id)?;
            all_effects.extend(task_job_effects);
        }

        // When a review job is Done (approved), mark the sibling craft job as Done too
        // (it was left in InReview while awaiting review).
        if new_status == JobStatus::Done {
            self.complete_sibling_craft_job(job_id)?;
        }

        Ok(all_effects)
    }

    /// When a review job is Done, find and complete the sibling craft job
    /// (which was left in InReview while awaiting review approval).
    fn complete_sibling_craft_job(&self, review_job_id: &JobId) -> crate::Result<()> {
        let Some(job) = self.db.get_job(review_job_id)? else {
            return Ok(());
        };
        if job.job_type != JobType::Review {
            return Ok(());
        }
        let Some(ref review_task_id) = job.task_id else {
            return Ok(());
        };
        let Some(task_state) = self.db.get_task_state(review_task_id)? else {
            return Ok(());
        };
        let task_store = TaskStoreImpl::from_db(&self.db, &task_state.workflow_id)
            .map_err(|e| crate::Error::Internal(e.to_string()))?;

        // Use task tree to find sibling craft task, then look up its job
        let Some(craft_node) = task_store.tree().sibling_craft(review_task_id) else {
            return Ok(());
        };
        let Some(craft_job) = self.db.get_job_by_task_id(&craft_node.id)? else {
            return Ok(());
        };
        if craft_job.status == JobStatus::InReview {
            self.db.update_job_status(&craft_job.id, JobStatus::Done)?;
            tracing::info!(
                craft_job_id = %craft_job.id,
                review_job_id = %review_job_id,
                "craft job completed via approved review"
            );
        }
        Ok(())
    }

    /// When a Job completes, mark its Task as Done and cascade effects through the tree.
    /// Returns any RuleEffects (e.g. AutoAssign) generated by newly created Jobs.
    fn propagate_task_completion(&self, job_id: &JobId) -> crate::Result<Vec<RuleEffect>> {
        let Some(job) = self.db.get_job(job_id)? else {
            return Ok(vec![]);
        };
        let Some(ref task_id) = job.task_id else {
            return Ok(vec![]); // Legacy job without task association
        };

        // Look up the workflow for this task
        let Some(task_state) = self.db.get_task_state(task_id)? else {
            return Ok(vec![]);
        };

        let task_store = TaskStoreImpl::from_db(&self.db, &task_state.workflow_id)
            .map_err(|e| crate::Error::Internal(e.to_string()))?;

        // Mark the completed task as Done
        task_store.update_task_status(task_id, TaskStatus::Done)?;

        tracing::info!(task_id = %task_id, "task completed via job");

        self.cascade_task_effects(task_id, &task_store)
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

                task_store.update_task_status(task_id, *new_status)?;
                tracing::info!(task_id = %task_id, status = ?new_status, "task status cascaded");

                match new_status {
                    TaskStatus::Ready => {
                        let (follow_up, new_job_effects) =
                            self.activate_ready_task(task_id, task_store, &task_engine)?;
                        next.extend(follow_up);
                        job_effects.extend(new_job_effects);
                    }
                    TaskStatus::Done => {
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
    /// Leaf tasks get a Job created; composite tasks resolve their children.
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
                // For review tasks, inherit plan_path and description from sibling craft task
                // so the reviewer sees the same context as the crafter
                if task.job_type == Some(JobType::Review)
                    && let Some(ref parent_id) = task.parent_id
                {
                    let siblings = task_store.get_child_tasks(parent_id)?;
                    if let Some(craft) =
                        siblings.iter().find(|s| s.job_type == Some(JobType::Craft))
                    {
                        if task.plan_path.is_none() {
                            task.plan_path = craft.plan_path.clone();
                        }
                        if task.description.is_none() {
                            task.description = craft.description.clone();
                        }
                    }
                }
                self.create_job_for_ready_task(&task)?
            } else {
                vec![]
            };
            Ok((vec![], job_effects))
        } else {
            // Composite task: transition to InProgress and resolve children
            task_store.update_task_status(task_id, TaskStatus::InProgress)?;
            let child_ids: Vec<TaskId> = children.iter().map(|c| c.id.clone()).collect();
            let task_effects = task_engine.resolve_ready_tasks(&child_ids)?;
            Ok((task_effects, vec![]))
        }
    }

    /// Create a Job for a leaf task that just became Ready.
    /// Returns RuleEffects (e.g. AutoAssign) that should be processed by the caller.
    fn create_job_for_ready_task(
        &self,
        task: &palette_domain::task::Task,
    ) -> crate::Result<Vec<RuleEffect>> {
        let job_type = task.job_type.expect("leaf task must have job_type");
        let job = self.db.create_job(&palette_domain::job::CreateJobRequest {
            id: Some(JobId::generate(job_type)),
            task_id: Some(task.id.clone()),
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
            depends_on: vec![],
        })?;

        let initial_status = match job_type {
            JobType::Craft => JobStatus::Ready,
            JobType::Review => JobStatus::Todo,
        };
        self.db.update_job_status(&job.id, initial_status)?;
        let rules = RuleEngine::new(&*self.db, 0);
        let effects = rules.on_status_change(&job.id, initial_status)?;

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
    use palette_db::Database;
    use palette_domain::agent::*;
    use palette_domain::review::*;
    use palette_domain::rule::*;

    use palette_domain::job::*;

    fn setup_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn jid(s: &str) -> JobId {
        JobId::new(s)
    }

    #[test]
    fn review_todo_triggers_auto_assign() {
        let db = setup_db();
        db.create_job(&CreateJobRequest {
            task_id: None,
            id: Some(jid("R-001")),
            job_type: JobType::Review,
            title: "Review".to_string(),
            plan_path: "test/R-001".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
            depends_on: vec![],
        })
        .unwrap();

        db.update_job_status(&jid("R-001"), JobStatus::Todo)
            .unwrap();

        let engine = RuleEngine::new(&db, 5);
        let effects = engine
            .on_status_change(&jid("R-001"), JobStatus::Todo)
            .unwrap();

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
        db.create_job(&CreateJobRequest {
            task_id: None,
            id: Some(jid("R-001")),
            job_type: JobType::Review,
            title: "Review".to_string(),
            plan_path: "test/R-001".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
            depends_on: vec![],
        })
        .unwrap();

        db.update_job_status(&jid("R-001"), JobStatus::Todo)
            .unwrap();
        db.assign_job(&jid("R-001"), &AgentId::new("member-b"))
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
                new_status: JobStatus::Done,
            }
        );
        assert_eq!(
            effects[1],
            RuleEffect::DestroyMember {
                member_id: AgentId::new("member-b"),
            }
        );

        let review = db.get_job(&jid("R-001")).unwrap().unwrap();
        assert_eq!(review.status, JobStatus::Done);
    }

    #[test]
    fn changes_requested_blocks_review() {
        let db = setup_db();
        db.create_job(&CreateJobRequest {
            task_id: None,
            id: Some(jid("R-001")),
            job_type: JobType::Review,
            title: "Review".to_string(),
            plan_path: "test/R-001".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
            depends_on: vec![],
        })
        .unwrap();

        db.update_job_status(&jid("R-001"), JobStatus::Todo)
            .unwrap();
        db.assign_job(&jid("R-001"), &AgentId::new("member-b"))
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

        assert!(effects.is_empty());

        let review = db.get_job(&jid("R-001")).unwrap().unwrap();
        assert_eq!(review.status, JobStatus::Blocked);
        assert_eq!(review.assignee, Some(AgentId::new("member-b")));
    }
}
