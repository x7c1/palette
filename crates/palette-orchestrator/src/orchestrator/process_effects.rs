use super::Orchestrator;
use palette_domain::agent::AgentId;
use palette_domain::rule::{RuleEffect, RuleEngine};
use palette_domain::server::{PendingDelivery, PersistentState};
use palette_domain::task::{Task, TaskId, TaskStatus};

impl Orchestrator {
    /// Processes rule engine effects: auto-assign tasks, spawn/destroy members.
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
                RuleEffect::AutoAssign { task_id } => {
                    self.handle_auto_assign(task_id, infra, &mut deliveries)?;
                }
                RuleEffect::DestroyMember { member_id } => {
                    self.handle_destroy_member(member_id, infra);
                }
                RuleEffect::StatusChanged {
                    task_id,
                    new_status,
                } => {
                    let chained = self.handle_status_changed(task_id, *new_status)?;
                    pending.extend(chained);
                }
                _ => {}
            }
        }

        Ok(deliveries)
    }

    fn handle_auto_assign(
        &self,
        task_id: &TaskId,
        infra: &mut PersistentState,
        deliveries: &mut Vec<PendingDelivery>,
    ) -> crate::Result<()> {
        // Only assign if the task is truly assignable (ready + all deps done)
        let assignable = self.db.find_assignable_tasks()?;
        let task = match assignable.iter().find(|t| t.id == *task_id) {
            Some(t) => t.clone(),
            None => return Ok(()),
        };
        let active = self.db.count_active_members()?;
        if active >= self.docker_config.max_members {
            tracing::info!(
                task_id = %task_id,
                active = active,
                max = self.docker_config.max_members,
                "max members reached, task waits"
            );
            return Ok(());
        }

        // Spawn a new member with leader_id based on task type
        let member_id = infra.next_member_id();
        let member = self.spawn_member(&member_id, task.task_type, infra)?;
        let terminal_target = member.terminal_target.clone();
        infra.members.push(member);

        // Assign task
        self.db.assign_task(task_id, &member_id)?;
        tracing::info!(
            task_id = %task_id,
            member_id = %member_id,
            "auto-assigned task"
        );

        // Build task instruction message
        let instruction = format_task_instruction(&task);
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

    fn handle_status_changed(
        &self,
        task_id: &TaskId,
        new_status: TaskStatus,
    ) -> crate::Result<Vec<RuleEffect>> {
        let rules = RuleEngine::new(&*self.db, 0); // max_review_rounds unused for status changes
        let chained = rules.on_status_change(task_id, new_status)?;
        for e in &chained {
            tracing::info!(?e, "chained rule engine effect");
        }
        Ok(chained)
    }
}

/// Format a task into an instruction message for a member.
fn format_task_instruction(task: &Task) -> String {
    let mut msg = format!("## Task: {}\n\nID: {}\n", task.title, task.id);
    if let Some(ref desc) = task.description {
        msg.push_str(&format!("\n{desc}\n"));
    }
    if let Some(ref repos) = task.repositories {
        msg.push('\n');
        for repo in repos {
            if let Some(ref branch) = repo.branch {
                msg.push_str(&format!("- {} (branch: {branch})\n", repo.name));
            } else {
                msg.push_str(&format!("- {}\n", repo.name));
            }
        }
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

    use palette_domain::task::*;

    fn setup_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn tid(s: &str) -> TaskId {
        TaskId::new(s)
    }

    fn create_work_review_pair(db: &Database) {
        db.create_task(&CreateTaskRequest {
            id: Some(tid("W-001")),
            task_type: TaskType::Work,
            title: "Work".to_string(),
            description: None,
            assignee: Some(AgentId::new("member-a")),
            priority: None,
            repositories: None,
            depends_on: vec![],
        })
        .unwrap();

        db.create_task(&CreateTaskRequest {
            id: Some(tid("R-001")),
            task_type: TaskType::Review,
            title: "Review".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![tid("W-001")],
        })
        .unwrap();
    }

    #[test]
    fn work_in_review_enables_reviews() {
        let db = setup_db();
        create_work_review_pair(&db);

        db.update_task_status(&tid("W-001"), TaskStatus::Ready)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::InProgress)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::InReview)
            .unwrap();

        let engine = RuleEngine::new(&db, 5);
        let effects = engine
            .on_status_change(&tid("W-001"), TaskStatus::InReview)
            .unwrap();

        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            RuleEffect::StatusChanged {
                task_id: tid("R-001"),
                new_status: TaskStatus::Todo,
            }
        );
    }

    #[test]
    fn review_todo_triggers_auto_assign() {
        let db = setup_db();
        create_work_review_pair(&db);

        db.update_task_status(&tid("W-001"), TaskStatus::Ready)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::InProgress)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::InReview)
            .unwrap();
        db.update_task_status(&tid("R-001"), TaskStatus::Todo)
            .unwrap();

        let engine = RuleEngine::new(&db, 5);
        let effects = engine
            .on_status_change(&tid("R-001"), TaskStatus::Todo)
            .unwrap();

        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            RuleEffect::AutoAssign {
                task_id: tid("R-001"),
            }
        );
    }

    #[test]
    fn review_auto_assign_chains_from_work_in_review() {
        // Verify the full chain: work → in_review → review → todo → auto_assign
        let db = setup_db();
        create_work_review_pair(&db);

        db.update_task_status(&tid("W-001"), TaskStatus::Ready)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::InProgress)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::InReview)
            .unwrap();

        let engine = RuleEngine::new(&db, 5);

        // Step 1: work → in_review produces StatusChanged for review
        let effects = engine
            .on_status_change(&tid("W-001"), TaskStatus::InReview)
            .unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            RuleEffect::StatusChanged {
                task_id: tid("R-001"),
                new_status: TaskStatus::Todo,
            }
        );

        // Step 2: chained StatusChanged(R-001, Todo) produces AutoAssign
        let chained = engine
            .on_status_change(&tid("R-001"), TaskStatus::Todo)
            .unwrap();
        assert_eq!(chained.len(), 1);
        assert_eq!(
            chained[0],
            RuleEffect::AutoAssign {
                task_id: tid("R-001"),
            }
        );
    }

    #[test]
    fn changes_requested_reverts_work() {
        let db = setup_db();
        create_work_review_pair(&db);

        db.update_task_status(&tid("W-001"), TaskStatus::Ready)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::InProgress)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::InReview)
            .unwrap();
        db.update_task_status(&tid("R-001"), TaskStatus::InProgress)
            .unwrap();

        let sub = db
            .submit_review(
                &tid("R-001"),
                &SubmitReviewRequest {
                    verdict: Verdict::ChangesRequested,
                    summary: None,
                    comments: vec![],
                },
            )
            .unwrap();

        let engine = RuleEngine::new(&db, 5);
        let effects = engine.on_review_submitted(&tid("R-001"), &sub).unwrap();

        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            RuleEffect::StatusChanged {
                task_id: tid("W-001"),
                new_status: TaskStatus::InProgress,
            }
        );

        let work = db.get_task(&tid("W-001")).unwrap().unwrap();
        assert_eq!(work.status, TaskStatus::InProgress);
    }

    #[test]
    fn approved_completes_work() {
        let db = setup_db();
        create_work_review_pair(&db);

        db.update_task_status(&tid("W-001"), TaskStatus::Ready)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::InProgress)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::InReview)
            .unwrap();
        db.update_task_status(&tid("R-001"), TaskStatus::InProgress)
            .unwrap();

        let sub = db
            .submit_review(
                &tid("R-001"),
                &SubmitReviewRequest {
                    verdict: Verdict::Approved,
                    summary: Some("LGTM".to_string()),
                    comments: vec![],
                },
            )
            .unwrap();

        let engine = RuleEngine::new(&db, 5);
        let effects = engine.on_review_submitted(&tid("R-001"), &sub).unwrap();

        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            RuleEffect::StatusChanged {
                task_id: tid("W-001"),
                new_status: TaskStatus::Done,
            }
        );
    }

    #[test]
    fn escalation_on_max_rounds() {
        let db = setup_db();
        create_work_review_pair(&db);

        db.update_task_status(&tid("W-001"), TaskStatus::Ready)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::InProgress)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::InReview)
            .unwrap();
        db.update_task_status(&tid("R-001"), TaskStatus::InProgress)
            .unwrap();

        // Round 1
        let sub1 = db
            .submit_review(
                &tid("R-001"),
                &SubmitReviewRequest {
                    verdict: Verdict::ChangesRequested,
                    summary: None,
                    comments: vec![],
                },
            )
            .unwrap();
        let engine = RuleEngine::new(&db, 2);
        engine.on_review_submitted(&tid("R-001"), &sub1).unwrap();

        // Reset work to in_review for round 2
        db.update_task_status(&tid("W-001"), TaskStatus::InReview)
            .unwrap();

        // Round 2 - should escalate
        let sub2 = db
            .submit_review(
                &tid("R-001"),
                &SubmitReviewRequest {
                    verdict: Verdict::ChangesRequested,
                    summary: None,
                    comments: vec![],
                },
            )
            .unwrap();
        let engine = RuleEngine::new(&db, 2);
        let effects = engine.on_review_submitted(&tid("R-001"), &sub2).unwrap();

        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], RuleEffect::Escalated { .. }));
    }
}
