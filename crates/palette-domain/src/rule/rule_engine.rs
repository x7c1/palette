use super::RuleEffect;
use crate::review::{ReviewSubmission, Verdict};
use crate::task::{TaskError, TaskId, TaskStatus, TaskStore, TaskType, TransitionError};

pub struct RuleEngine<S> {
    store: S,
    max_review_rounds: u32,
}

impl<S: TaskStore> RuleEngine<S> {
    pub fn new(store: S, max_review_rounds: u32) -> Self {
        Self {
            store,
            max_review_rounds,
        }
    }

    /// Apply rules after a task status change. Returns side effects.
    pub fn on_status_change(
        &self,
        task_id: &TaskId,
        new_status: TaskStatus,
    ) -> Result<Vec<RuleEffect>, S::Error> {
        let task = self
            .store
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
            // review -> todo: trigger auto-assign for reviewer member
            (TaskType::Review, TaskStatus::Todo) => {
                effects.push(RuleEffect::AutoAssign {
                    task_id: task_id.clone(),
                });
            }
            // work -> in_review: enable related reviews
            (TaskType::Work, TaskStatus::InReview) => {
                let reviews = self.store.find_reviews_for_work(task_id)?;
                for review in reviews {
                    if review.status == TaskStatus::Todo || review.status == TaskStatus::Blocked {
                        self.store
                            .update_task_status(&review.id, TaskStatus::Todo)?;
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
                let assignable = self.store.find_assignable_tasks()?;
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
        review_task_id: &TaskId,
        submission: &ReviewSubmission,
    ) -> Result<Vec<RuleEffect>, S::Error> {
        let mut effects = Vec::new();
        let work_tasks = self.store.find_works_for_review(review_task_id)?;

        match submission.verdict {
            Verdict::ChangesRequested => {
                // Check escalation threshold
                if submission.round as u32 >= self.max_review_rounds {
                    for work in &work_tasks {
                        self.store
                            .update_task_status(&work.id, TaskStatus::Escalated)?;
                        effects.push(RuleEffect::Escalated {
                            task_id: work.id.clone(),
                            round: submission.round,
                        });
                    }
                    return Ok(effects);
                }

                // Revert work tasks to in_progress
                for work in &work_tasks {
                    self.store
                        .update_task_status(&work.id, TaskStatus::InProgress)?;
                    effects.push(RuleEffect::StatusChanged {
                        task_id: work.id.clone(),
                        new_status: TaskStatus::InProgress,
                    });
                }
            }
            Verdict::Approved => {
                // Check if ALL reviews for each work task are approved
                for work in &work_tasks {
                    let all_reviews = self.store.find_reviews_for_work(&work.id)?;
                    let all_approved = all_reviews.iter().all(|r| {
                        if r.id == *review_task_id {
                            return true; // This one is being approved now
                        }
                        let subs = self.store.get_review_submissions(&r.id).unwrap_or_default();
                        subs.last().is_some_and(|s| s.verdict == Verdict::Approved)
                    });
                    if all_approved {
                        self.store.update_task_status(&work.id, TaskStatus::Done)?;
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
}

/// Validate a status transition.
pub fn validate_transition(
    task_type: TaskType,
    from: TaskStatus,
    to: TaskStatus,
) -> Result<(), TransitionError> {
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
        return Err(TransitionError {
            task_type,
            from,
            to,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_transitions() {
        assert!(validate_transition(TaskType::Work, TaskStatus::Draft, TaskStatus::Ready).is_ok());
        assert!(
            validate_transition(TaskType::Work, TaskStatus::Ready, TaskStatus::InProgress).is_ok()
        );
        assert!(
            validate_transition(TaskType::Work, TaskStatus::InProgress, TaskStatus::InReview)
                .is_ok()
        );
        assert!(
            validate_transition(TaskType::Work, TaskStatus::InReview, TaskStatus::Done).is_ok()
        );
        assert!(
            validate_transition(TaskType::Work, TaskStatus::InReview, TaskStatus::InProgress)
                .is_ok()
        );
    }

    #[test]
    fn invalid_transitions() {
        assert!(validate_transition(TaskType::Work, TaskStatus::Draft, TaskStatus::Done).is_err());
        assert!(
            validate_transition(TaskType::Work, TaskStatus::Draft, TaskStatus::InProgress).is_err()
        );
        assert!(validate_transition(TaskType::Work, TaskStatus::Done, TaskStatus::Draft).is_err());
        assert!(validate_transition(TaskType::Review, TaskStatus::Done, TaskStatus::Todo).is_err());
    }
}
