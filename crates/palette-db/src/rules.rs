use crate::models::*;
use crate::repository::Database;
use anyhow::bail;

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
        task_id: &str,
        new_status: TaskStatus,
    ) -> anyhow::Result<Vec<RuleEffect>> {
        let task = db
            .get_task(task_id)?
            .ok_or_else(|| anyhow::anyhow!("task not found: {task_id}"))?;

        let mut effects = Vec::new();

        // work -> in_review: enable related reviews
        if let (TaskType::Work, TaskStatus::InReview) = (task.task_type, new_status) {
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

        Ok(effects)
    }

    /// Apply rules after a review submission. Returns side effects.
    pub fn on_review_submitted(
        &self,
        db: &Database,
        review_task_id: &str,
        submission: &ReviewSubmission,
    ) -> anyhow::Result<Vec<RuleEffect>> {
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
                        if r.id == review_task_id {
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
    ) -> anyhow::Result<()> {
        let valid = match (task_type, from, to) {
            // Work transitions
            (TaskType::Work, TaskStatus::Todo, TaskStatus::InProgress) => true,
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
            bail!(
                "invalid status transition for {} task: {} -> {}",
                task_type.as_str(),
                from.as_str(),
                to.as_str()
            );
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

    fn create_work_review_pair(db: &Database) {
        db.create_task(&CreateTaskRequest {
            id: Some("W-001".to_string()),
            task_type: TaskType::Work,
            title: "Work".to_string(),
            description: None,
            assignee: Some("member-a".to_string()),
            priority: None,
            repositories: None,
            branch: None,
            depends_on: vec![],
        })
        .unwrap();

        db.create_task(&CreateTaskRequest {
            id: Some("R-001".to_string()),
            task_type: TaskType::Review,
            title: "Review".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            branch: None,
            depends_on: vec!["W-001".to_string()],
        })
        .unwrap();
    }

    #[test]
    fn work_in_review_enables_reviews() {
        let (db, engine) = setup();
        create_work_review_pair(&db);

        db.update_task_status("W-001", TaskStatus::InProgress)
            .unwrap();
        db.update_task_status("W-001", TaskStatus::InReview)
            .unwrap();

        let effects = engine
            .on_status_change(&db, "W-001", TaskStatus::InReview)
            .unwrap();

        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            RuleEffect::StatusChanged {
                task_id: "R-001".to_string(),
                new_status: TaskStatus::Todo,
            }
        );
    }

    #[test]
    fn changes_requested_reverts_work() {
        let (db, engine) = setup();
        create_work_review_pair(&db);

        db.update_task_status("W-001", TaskStatus::InProgress)
            .unwrap();
        db.update_task_status("W-001", TaskStatus::InReview)
            .unwrap();
        db.update_task_status("R-001", TaskStatus::InProgress)
            .unwrap();

        let sub = db
            .submit_review(
                "R-001",
                &SubmitReviewRequest {
                    verdict: Verdict::ChangesRequested,
                    summary: None,
                    comments: vec![],
                },
            )
            .unwrap();

        let effects = engine.on_review_submitted(&db, "R-001", &sub).unwrap();

        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            RuleEffect::StatusChanged {
                task_id: "W-001".to_string(),
                new_status: TaskStatus::InProgress,
            }
        );

        let work = db.get_task("W-001").unwrap().unwrap();
        assert_eq!(work.status, TaskStatus::InProgress);
    }

    #[test]
    fn approved_completes_work() {
        let (db, engine) = setup();
        create_work_review_pair(&db);

        db.update_task_status("W-001", TaskStatus::InProgress)
            .unwrap();
        db.update_task_status("W-001", TaskStatus::InReview)
            .unwrap();
        db.update_task_status("R-001", TaskStatus::InProgress)
            .unwrap();

        let sub = db
            .submit_review(
                "R-001",
                &SubmitReviewRequest {
                    verdict: Verdict::Approved,
                    summary: Some("LGTM".to_string()),
                    comments: vec![],
                },
            )
            .unwrap();

        let effects = engine.on_review_submitted(&db, "R-001", &sub).unwrap();

        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            RuleEffect::StatusChanged {
                task_id: "W-001".to_string(),
                new_status: TaskStatus::Done,
            }
        );
    }

    #[test]
    fn escalation_on_max_rounds() {
        let (db, _) = setup();
        let engine = RuleEngine::new(2); // Low threshold for testing
        create_work_review_pair(&db);

        db.update_task_status("W-001", TaskStatus::InProgress)
            .unwrap();
        db.update_task_status("W-001", TaskStatus::InReview)
            .unwrap();
        db.update_task_status("R-001", TaskStatus::InProgress)
            .unwrap();

        // Round 1
        let sub1 = db
            .submit_review(
                "R-001",
                &SubmitReviewRequest {
                    verdict: Verdict::ChangesRequested,
                    summary: None,
                    comments: vec![],
                },
            )
            .unwrap();
        engine.on_review_submitted(&db, "R-001", &sub1).unwrap();

        // Reset work to in_review for round 2
        db.update_task_status("W-001", TaskStatus::InReview)
            .unwrap();

        // Round 2 - should escalate
        let sub2 = db
            .submit_review(
                "R-001",
                &SubmitReviewRequest {
                    verdict: Verdict::ChangesRequested,
                    summary: None,
                    comments: vec![],
                },
            )
            .unwrap();
        let effects = engine.on_review_submitted(&db, "R-001", &sub2).unwrap();

        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], RuleEffect::Escalated { .. }));
    }

    #[test]
    fn valid_transitions() {
        assert!(
            RuleEngine::validate_transition(
                TaskType::Work,
                TaskStatus::Todo,
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
            RuleEngine::validate_transition(TaskType::Work, TaskStatus::Todo, TaskStatus::Done)
                .is_err()
        );
        assert!(
            RuleEngine::validate_transition(TaskType::Work, TaskStatus::Done, TaskStatus::Todo)
                .is_err()
        );
        assert!(
            RuleEngine::validate_transition(TaskType::Review, TaskStatus::Done, TaskStatus::Todo)
                .is_err()
        );
    }
}
