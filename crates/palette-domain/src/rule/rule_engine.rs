use super::RuleEffect;
use crate::job::{
    CraftStatus, JobError, JobId, JobStatus, JobStore, ReviewStatus, TransitionError,
};
use crate::review::{ReviewSubmission, Verdict};

pub struct RuleEngine<S> {
    store: S,
    #[allow(dead_code)]
    // TODO: re-enable when escalation trigger conditions are designed
    max_review_rounds: u32,
}

impl<S: JobStore> RuleEngine<S> {
    pub fn new(store: S, max_review_rounds: u32) -> Self {
        Self {
            store,
            max_review_rounds,
        }
    }

    /// Apply rules after a new job is created (Todo status). Returns side effects.
    pub fn on_job_created(&self, job_id: &JobId) -> Result<Vec<RuleEffect>, S::Error> {
        Ok(vec![RuleEffect::AssignNewJob {
            job_id: job_id.clone(),
        }])
    }

    /// Apply rules after a craft job completes. Returns side effects.
    pub fn on_craft_done(&self, job_id: &JobId) -> Result<Vec<RuleEffect>, S::Error> {
        let job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| JobError::NotFound {
                job_id: job_id.clone(),
            })?;

        let mut effects = Vec::new();
        if let Some(ref assignee) = job.assignee_id {
            effects.push(RuleEffect::DestroyMember {
                member_id: assignee.clone(),
            });
        }
        effects.push(RuleEffect::JobCompleted {
            job_id: job_id.clone(),
        });
        Ok(effects)
    }

    /// Apply rules after a review submission. Returns side effects.
    pub fn on_review_submitted(
        &self,
        review_job_id: &JobId,
        submission: &ReviewSubmission,
    ) -> Result<Vec<RuleEffect>, S::Error> {
        let review_job = self
            .store
            .get_job(review_job_id)?
            .ok_or_else(|| JobError::NotFound {
                job_id: review_job_id.clone(),
            })?;

        let mut effects = vec![RuleEffect::ReviewVerdict {
            review_job_id: review_job_id.clone(),
            verdict: submission.verdict,
        }];

        match submission.verdict {
            Verdict::ChangesRequested => {
                self.store.update_job_status(
                    review_job_id,
                    JobStatus::Review(ReviewStatus::ChangesRequested),
                )?;
            }
            Verdict::Approved => {
                self.store
                    .update_job_status(review_job_id, JobStatus::Review(ReviewStatus::Done))?;
                if let Some(ref assignee) = review_job.assignee_id {
                    effects.push(RuleEffect::DestroyMember {
                        member_id: assignee.clone(),
                    });
                }
            }
        }

        Ok(effects)
    }
}

/// Validate a craft status transition.
pub fn validate_craft_transition(
    from: CraftStatus,
    to: CraftStatus,
) -> Result<(), TransitionError> {
    let valid = matches!(
        (from, to),
        (CraftStatus::Todo, CraftStatus::InProgress)
            | (CraftStatus::InProgress, CraftStatus::InReview)
            | (CraftStatus::InReview, CraftStatus::Done)
            | (CraftStatus::InReview, CraftStatus::InProgress) // changes_requested
            | (_, CraftStatus::Escalated)
    );

    if !valid {
        return Err(TransitionError {
            from: JobStatus::Craft(from),
            to: JobStatus::Craft(to),
        });
    }

    Ok(())
}

/// Validate a review status transition.
pub fn validate_review_transition(
    from: ReviewStatus,
    to: ReviewStatus,
) -> Result<(), TransitionError> {
    let valid = matches!(
        (from, to),
        (ReviewStatus::Todo, ReviewStatus::InProgress)
            | (ReviewStatus::InProgress, ReviewStatus::Done)
            | (ReviewStatus::InProgress, ReviewStatus::ChangesRequested)
            | (ReviewStatus::ChangesRequested, ReviewStatus::InProgress) // re-review
            | (_, ReviewStatus::Escalated)
    );

    if !valid {
        return Err(TransitionError {
            from: JobStatus::Review(from),
            to: JobStatus::Review(to),
        });
    }

    Ok(())
}

/// Validate a job status transition, dispatching by job type.
pub fn validate_transition(from: JobStatus, to: JobStatus) -> Result<(), TransitionError> {
    match (from, to) {
        (JobStatus::Craft(f), JobStatus::Craft(t)) => validate_craft_transition(f, t),
        (JobStatus::Review(f), JobStatus::Review(t)) => validate_review_transition(f, t),
        _ => Err(TransitionError { from, to }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_craft_transitions() {
        assert!(validate_craft_transition(CraftStatus::Todo, CraftStatus::InProgress).is_ok());
        assert!(validate_craft_transition(CraftStatus::InProgress, CraftStatus::InReview).is_ok());
        assert!(validate_craft_transition(CraftStatus::InReview, CraftStatus::Done).is_ok());
        assert!(validate_craft_transition(CraftStatus::InReview, CraftStatus::InProgress).is_ok());
        assert!(validate_craft_transition(CraftStatus::Todo, CraftStatus::Escalated).is_ok());
    }

    #[test]
    fn invalid_craft_transitions() {
        assert!(validate_craft_transition(CraftStatus::Todo, CraftStatus::Done).is_err());
        assert!(validate_craft_transition(CraftStatus::Todo, CraftStatus::InReview).is_err());
        assert!(validate_craft_transition(CraftStatus::Done, CraftStatus::Todo).is_err());
    }

    #[test]
    fn valid_review_transitions() {
        assert!(validate_review_transition(ReviewStatus::Todo, ReviewStatus::InProgress).is_ok());
        assert!(validate_review_transition(ReviewStatus::InProgress, ReviewStatus::Done).is_ok());
        assert!(
            validate_review_transition(ReviewStatus::InProgress, ReviewStatus::ChangesRequested)
                .is_ok()
        );
        assert!(
            validate_review_transition(ReviewStatus::ChangesRequested, ReviewStatus::InProgress)
                .is_ok()
        );
    }

    #[test]
    fn invalid_review_transitions() {
        assert!(validate_review_transition(ReviewStatus::Todo, ReviewStatus::Done).is_err());
        assert!(validate_review_transition(ReviewStatus::Done, ReviewStatus::Todo).is_err());
        assert!(
            validate_review_transition(ReviewStatus::Todo, ReviewStatus::ChangesRequested).is_err()
        );
    }

    #[test]
    fn cross_type_transition_is_invalid() {
        assert!(
            validate_transition(
                JobStatus::Craft(CraftStatus::InProgress),
                JobStatus::Review(ReviewStatus::InProgress),
            )
            .is_err()
        );
    }
}
