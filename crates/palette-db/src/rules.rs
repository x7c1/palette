use crate::errors::{DbError, TaskError};
use crate::models::*;
use crate::repository::Database;

pub struct RuleEngine {
    max_review_rounds: u32,
}

impl RuleEngine {
    pub fn new(max_review_rounds: u32) -> Self {
        Self { max_review_rounds }
    }

    /// Apply rules after a task status change. Returns side effects.
    pub fn on_status_change(
        &self,
        db: &Database,
        task_id: &TaskId,
        new_status: TaskStatus,
    ) -> Result<Vec<RuleEffect>, DbError> {
        let task = db
            .get_task(task_id)?
            .ok_or_else(|| TaskError::NotFound {
                task_id: task_id.clone(),
            })?;

        let mut effects = Vec::new();

        match (task.task_type, new_status) {
            // work -> ready: trigger auto-assign evaluation
            (TaskType::Work, TaskStatus::Ready) => {
                effects.push(RuleEffect::AutoAssign {
                    task_id: task_id.clone(),
                });
            }
            // work -> in_review: enable related reviews
            (TaskType::Work, TaskStatus::InReview) => {
                let reviews = db.find_reviews_for_work(task_id)?;
                for review in reviews {
                    if review.status == TaskStatus::Todo || review.status == TaskStatus::Blocked {
                        db.update_task_status(&review.id, TaskStatus::Todo)?;
                        effects.push(RuleEffect::StatusChanged {
                            task_id: review.id,
                            new_status: TaskStatus::Todo,
                        });
                    }
                }
            }
            // work -> done: destroy member container, trigger auto-assign for waiting tasks
            (TaskType::Work, TaskStatus::Done) => {
                if let Some(ref assignee) = task.assignee {
                    effects.push(RuleEffect::DestroyMember {
                        member_id: assignee.clone(),
                    });
                }
                // Check if any blocked tasks can now proceed
                let assignable = db.find_assignable_tasks()?;
                for t in assignable {
                    effects.push(RuleEffect::AutoAssign {
                        task_id: t.id.clone(),
                    });
                }
            }
            _ => {}
        }

        Ok(effects)
    }

    /// Apply rules after a review submission. Returns side effects.
    pub fn on_review_submitted(
        &self,
        db: &Database,
        review_task_id: &TaskId,
        submission: &ReviewSubmission,
    ) -> Result<Vec<RuleEffect>, DbError> {
        let mut effects = Vec::new();
        let work_tasks = db.find_works_for_review(review_task_id)?;

        match submission.verdict {
            Verdict::ChangesRequested => {
                // Check escalation threshold
                if submission.round as u32 >= self.max_review_rounds {
                    for work in &work_tasks {
                        db.update_task_status(&work.id, TaskStatus::Escalated)?;
                        effects.push(RuleEffect::Escalated {
                            task_id: work.id.clone(),
                            round: submission.round,
                        });
                    }
                    return Ok(effects);
                }

                // Revert work tasks to in_progress
                for work in &work_tasks {
                    db.update_task_status(&work.id, TaskStatus::InProgress)?;
                    effects.push(RuleEffect::StatusChanged {
                        task_id: work.id.clone(),
                        new_status: TaskStatus::InProgress,
                    });
                }
            }
            Verdict::Approved => {
                // Check if ALL reviews for each work task are approved
                for work in &work_tasks {
                    let all_reviews = db.find_reviews_for_work(&work.id)?;
                    let all_approved = all_reviews.iter().all(|r| {
                        if r.id == *review_task_id {
                            return true; // This one is being approved now
                        }
                        let subs = db.get_review_submissions(&r.id).unwrap_or_default();
                        subs.last().is_some_and(|s| s.verdict == Verdict::Approved)
                    });
                    if all_approved {
                        db.update_task_status(&work.id, TaskStatus::Done)?;
                        effects.push(RuleEffect::StatusChanged {
                            task_id: work.id.clone(),
                            new_status: TaskStatus::Done,
                        });
                    }
                }
            }
        }

        Ok(effects)
    }

    /// Validate a status transition.
    pub fn validate_transition(
        task_type: TaskType,
        from: TaskStatus,
        to: TaskStatus,
    ) -> Result<(), DbError> {
        let valid = match (task_type, from, to) {
            // Work transitions
            (TaskType::Work, TaskStatus::Draft, TaskStatus::Ready) => true,
            (TaskType::Work, TaskStatus::Ready, TaskStatus::InProgress) => true,
            (TaskType::Work, TaskStatus::InProgress, TaskStatus::InReview) => true,
            (TaskType::Work, TaskStatus::InReview, TaskStatus::Done) => true,
            (TaskType::Work, TaskStatus::InReview, TaskStatus::InProgress) => true, // changes_requested
            (TaskType::Work, TaskStatus::InProgress, TaskStatus::Blocked) => true,
            (TaskType::Work, TaskStatus::Blocked, TaskStatus::InProgress) => true,
            (TaskType::Work, _, TaskStatus::Escalated) => true,

            // Review transitions
            (TaskType::Review, TaskStatus::Todo, TaskStatus::InProgress) => true,
            (TaskType::Review, TaskStatus::Blocked, TaskStatus::Todo) => true,
            (TaskType::Review, TaskStatus::InProgress, TaskStatus::Done) => true,

            _ => false,
        };

        if !valid {
            return Err(DbError::InvalidTransition {
                task_type,
                from,
                to,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn valid_transitions() {
        assert!(
            RuleEngine::validate_transition(TaskType::Work, TaskStatus::Draft, TaskStatus::Ready)
                .is_ok()
        );
        assert!(
            RuleEngine::validate_transition(
                TaskType::Work,
                TaskStatus::Ready,
                TaskStatus::InProgress
            )
            .is_ok()
        );
        assert!(
            RuleEngine::validate_transition(
                TaskType::Work,
                TaskStatus::InProgress,
                TaskStatus::InReview
            )
            .is_ok()
        );
        assert!(
            RuleEngine::validate_transition(TaskType::Work, TaskStatus::InReview, TaskStatus::Done)
                .is_ok()
        );
        assert!(
            RuleEngine::validate_transition(
                TaskType::Work,
                TaskStatus::InReview,
                TaskStatus::InProgress
            )
            .is_ok()
        );
    }

    #[test]
    fn invalid_transitions() {
        assert!(
            RuleEngine::validate_transition(TaskType::Work, TaskStatus::Draft, TaskStatus::Done)
                .is_err()
        );
        assert!(
            RuleEngine::validate_transition(
                TaskType::Work,
                TaskStatus::Draft,
                TaskStatus::InProgress
            )
            .is_err()
        );
        assert!(
            RuleEngine::validate_transition(TaskType::Work, TaskStatus::Done, TaskStatus::Draft)
                .is_err()
        );
        assert!(
            RuleEngine::validate_transition(TaskType::Review, TaskStatus::Done, TaskStatus::Todo)
                .is_err()
        );
    }
}
