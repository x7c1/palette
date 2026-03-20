use super::RuleEffect;
use crate::job::{JobError, JobId, JobStatus, JobStore, JobType, TransitionError};
use crate::review::{ReviewSubmission, Verdict};

pub struct RuleEngine<S> {
    store: S,
    #[allow(dead_code)] // TODO: re-enable when changes_requested is implemented for task tree model
    max_review_rounds: u32,
}

impl<S: JobStore> RuleEngine<S> {
    pub fn new(store: S, max_review_rounds: u32) -> Self {
        Self {
            store,
            max_review_rounds,
        }
    }

    /// Apply rules after a job status change. Returns side effects.
    pub fn on_status_change(
        &self,
        job_id: &JobId,
        new_status: JobStatus,
    ) -> Result<Vec<RuleEffect>, S::Error> {
        let job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| JobError::NotFound {
                job_id: job_id.clone(),
            })?;

        let mut effects = Vec::new();

        match (job.job_type, new_status) {
            // craft -> ready: trigger auto-assign evaluation
            (JobType::Craft, JobStatus::Ready) => {
                effects.push(RuleEffect::AutoAssign {
                    job_id: job_id.clone(),
                });
            }
            // review -> todo: trigger auto-assign for reviewer member
            (JobType::Review, JobStatus::Todo) => {
                effects.push(RuleEffect::AutoAssign {
                    job_id: job_id.clone(),
                });
            }
            // craft -> in_review: task cascade handles review task activation
            (JobType::Craft, JobStatus::InReview) => {}
            // craft -> done: destroy member container, trigger auto-assign for waiting jobs
            (JobType::Craft, JobStatus::Done) => {
                if let Some(ref assignee) = job.assignee {
                    effects.push(RuleEffect::DestroyMember {
                        member_id: assignee.clone(),
                    });
                }
                // Check if any blocked jobs can now proceed
                let assignable = self.store.find_assignable_jobs()?;
                for j in assignable {
                    effects.push(RuleEffect::AutoAssign {
                        job_id: j.id.clone(),
                    });
                }
            }
            _ => {}
        }

        Ok(effects)
    }

    /// Apply rules after a review submission. Returns side effects.
    ///
    /// In the task tree model, craft and review are sibling tasks. The craft task
    /// is already Done (marked when its job reached InReview). This method only
    /// needs to handle the review job's own status transitions:
    /// - Approved: review job → Done → task cascade handles the rest
    /// - ChangesRequested: review job → Done (with verdict recorded) → task cascade
    ///   Note: changes_requested handling in the task tree model is a future design task
    pub fn on_review_submitted(
        &self,
        review_job_id: &JobId,
        submission: &ReviewSubmission,
    ) -> Result<Vec<RuleEffect>, S::Error> {
        let mut effects = Vec::new();
        let review_job = self
            .store
            .get_job(review_job_id)?
            .ok_or_else(|| JobError::NotFound {
                job_id: review_job_id.clone(),
            })?;

        match submission.verdict {
            Verdict::ChangesRequested => {
                // Review job → Blocked. The reviewer keeps their assignee for re-review.
                // TODO: In the task tree model, changes_requested should trigger
                // a new craft cycle. For now, the review stays blocked.
                self.store
                    .update_job_status(review_job_id, JobStatus::Blocked)?;
            }
            Verdict::Approved => {
                // Review job → Done. Task cascade will propagate completion.
                self.store
                    .update_job_status(review_job_id, JobStatus::Done)?;
                effects.push(RuleEffect::StatusChanged {
                    job_id: review_job_id.clone(),
                    new_status: JobStatus::Done,
                });
                if let Some(ref assignee) = review_job.assignee {
                    effects.push(RuleEffect::DestroyMember {
                        member_id: assignee.clone(),
                    });
                }
            }
        }

        Ok(effects)
    }
}

/// Validate a status transition.
pub fn validate_transition(
    job_type: JobType,
    from: JobStatus,
    to: JobStatus,
) -> Result<(), TransitionError> {
    let valid = match (job_type, from, to) {
        // Craft transitions
        (JobType::Craft, JobStatus::Draft, JobStatus::Ready) => true,
        (JobType::Craft, JobStatus::Ready, JobStatus::InProgress) => true,
        (JobType::Craft, JobStatus::InProgress, JobStatus::InReview) => true,
        (JobType::Craft, JobStatus::InReview, JobStatus::Done) => true,
        (JobType::Craft, JobStatus::InReview, JobStatus::InProgress) => true, // changes_requested
        (JobType::Craft, JobStatus::InProgress, JobStatus::Blocked) => true,
        (JobType::Craft, JobStatus::Blocked, JobStatus::InProgress) => true,
        (JobType::Craft, _, JobStatus::Escalated) => true,

        // Review transitions
        (JobType::Review, JobStatus::Draft, JobStatus::Todo) => true,
        (JobType::Review, JobStatus::Todo, JobStatus::InProgress) => true,
        (JobType::Review, JobStatus::Blocked, JobStatus::Todo) => true,
        (JobType::Review, JobStatus::InProgress, JobStatus::Done) => true,
        (JobType::Review, JobStatus::InProgress, JobStatus::Blocked) => true, // changes_requested

        _ => false,
    };

    if !valid {
        return Err(TransitionError { job_type, from, to });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_transitions() {
        assert!(validate_transition(JobType::Craft, JobStatus::Draft, JobStatus::Ready).is_ok());
        assert!(
            validate_transition(JobType::Craft, JobStatus::Ready, JobStatus::InProgress).is_ok()
        );
        assert!(
            validate_transition(JobType::Craft, JobStatus::InProgress, JobStatus::InReview).is_ok()
        );
        assert!(validate_transition(JobType::Craft, JobStatus::InReview, JobStatus::Done).is_ok());
        assert!(
            validate_transition(JobType::Craft, JobStatus::InReview, JobStatus::InProgress).is_ok()
        );
    }

    #[test]
    fn invalid_transitions() {
        assert!(validate_transition(JobType::Craft, JobStatus::Draft, JobStatus::Done).is_err());
        assert!(
            validate_transition(JobType::Craft, JobStatus::Draft, JobStatus::InProgress).is_err()
        );
        assert!(validate_transition(JobType::Craft, JobStatus::Done, JobStatus::Draft).is_err());
        assert!(validate_transition(JobType::Review, JobStatus::Done, JobStatus::Todo).is_err());
    }
}
