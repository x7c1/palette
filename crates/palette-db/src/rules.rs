// RuleEngine and domain logic have moved to palette-domain.
// This module contains the TaskStore implementation for Database
// and integration tests.

use crate::db_error::DbError;
use crate::database::Database;
use palette_domain::*;

impl TaskStore for Database {
    type Error = DbError;

    fn get_task(&self, id: &TaskId) -> Result<Option<Task>, DbError> {
        self.get_task(id)
    }

    fn find_reviews_for_work(&self, work_id: &TaskId) -> Result<Vec<Task>, DbError> {
        self.find_reviews_for_work(work_id)
    }

    fn find_works_for_review(&self, review_id: &TaskId) -> Result<Vec<Task>, DbError> {
        self.find_works_for_review(review_id)
    }

    fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> Result<Task, DbError> {
        self.update_task_status(id, status)
    }

    fn find_assignable_tasks(&self) -> Result<Vec<Task>, DbError> {
        self.find_assignable_tasks()
    }

    fn get_review_submissions(&self, review_id: &TaskId) -> Result<Vec<ReviewSubmission>, DbError> {
        self.get_review_submissions(review_id)
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
}
