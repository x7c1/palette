use super::{format_task_instruction, spawn_member};
use crate::DockerConfig;
use palette_db::Database;
use palette_docker::DockerManager;
use palette_domain::rule::{RuleEffect, RuleEngine};
use palette_domain::server::{PendingDelivery, PersistentState};
use palette_tmux::TmuxManager;

/// Processes rule engine effects: auto-assign tasks, spawn/destroy members.
/// Returns a list of messages that need to be sent to members via tmux.
///
/// The caller is responsible for saving state after this function returns.
pub fn process_effects(
    effects: &[RuleEffect],
    db: &Database,
    infra: &mut PersistentState,
    docker: &DockerManager,
    tmux: &TmuxManager,
    config: &DockerConfig,
) -> crate::Result<Vec<PendingDelivery>> {
    let mut deliveries = Vec::new();
    let mut pending: Vec<RuleEffect> = effects.to_vec();

    while let Some(effect) = pending.pop() {
        match &effect {
            RuleEffect::AutoAssign { task_id } => {
                // Only assign if the task is truly assignable (ready + all deps done)
                let assignable = db.find_assignable_tasks()?;
                let task = match assignable.iter().find(|t| t.id == *task_id) {
                    Some(t) => t.clone(),
                    None => continue,
                };
                let active = db.count_active_members()?;
                if active >= config.max_members {
                    tracing::info!(
                        task_id = %task_id,
                        active = active,
                        max = config.max_members,
                        "max members reached, task waits"
                    );
                    continue;
                }
                // Spawn a new member
                let member_id = infra.next_member_id();
                let member = spawn_member(&member_id, infra, docker, tmux, config)?;
                let terminal_target = member.terminal_target.clone();
                infra.members.push(member);

                // Assign task
                db.assign_task(task_id, &member_id)?;
                tracing::info!(
                    task_id = %task_id,
                    member_id = %member_id,
                    "auto-assigned task"
                );

                // Build task instruction message
                let instruction = format_task_instruction(&task);
                db.enqueue_message(&member_id, &instruction)?;

                deliveries.push(PendingDelivery {
                    target_id: member_id,
                    terminal_target,
                });

                infra.touch();
            }
            RuleEffect::DestroyMember { member_id } => {
                if let Some(member) = infra.remove_member(member_id) {
                    tracing::info!(member_id = %member_id, "destroying member container");
                    let _ = docker.stop_container(&member.container_id);
                    let _ = docker.remove_container(&member.container_id);
                    infra.touch();
                }
            }
            RuleEffect::StatusChanged {
                task_id,
                new_status,
            } => {
                // Chain: re-evaluate rules for the new status
                let rules = RuleEngine::new(0); // max_review_rounds unused for status changes
                let chained = rules.on_status_change(db, task_id, *new_status)?;
                for e in &chained {
                    tracing::info!(?e, "chained rule engine effect");
                }
                pending.extend(chained);
            }
            _ => {}
        }
    }

    Ok(deliveries)
}

#[cfg(test)]
mod tests {
    use palette_db::Database;
    use palette_domain::agent::*;
    use palette_domain::review::*;
    use palette_domain::rule::*;

    use palette_domain::task::*;

    fn setup() -> (Database, RuleEngine) {
        let db = Database::open_in_memory().unwrap();
        let engine = RuleEngine::new(5);
        (db, engine)
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
        let (db, engine) = setup();
        create_work_review_pair(&db);

        db.update_task_status(&tid("W-001"), TaskStatus::Ready)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::InProgress)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::InReview)
            .unwrap();

        let effects = engine
            .on_status_change(&db, &tid("W-001"), TaskStatus::InReview)
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
    fn changes_requested_reverts_work() {
        let (db, engine) = setup();
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

        let effects = engine
            .on_review_submitted(&db, &tid("R-001"), &sub)
            .unwrap();

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
        let (db, engine) = setup();
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

        let effects = engine
            .on_review_submitted(&db, &tid("R-001"), &sub)
            .unwrap();

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
        let (db, _) = setup();
        let engine = RuleEngine::new(2); // Low threshold for testing
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
        engine
            .on_review_submitted(&db, &tid("R-001"), &sub1)
            .unwrap();

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
        let effects = engine
            .on_review_submitted(&db, &tid("R-001"), &sub2)
            .unwrap();

        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], RuleEffect::Escalated { .. }));
    }
}
