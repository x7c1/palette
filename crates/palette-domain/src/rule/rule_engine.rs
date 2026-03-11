use super::RuleEffect;
use crate::job::{JobError, JobId, JobStatus, JobStore, JobType, TransitionError};
use crate::review::{ReviewSubmission, Verdict};

pub struct RuleEngine<S> {
    store: S,
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
            // craft -> in_review: enable related reviews
            (JobType::Craft, JobStatus::InReview) => {
                let reviews = self.store.find_reviews_for_craft(job_id)?;
                for review in reviews {
                    if review.status == JobStatus::Todo || review.status == JobStatus::Blocked {
                        self.store.update_job_status(&review.id, JobStatus::Todo)?;
                        effects.push(RuleEffect::StatusChanged {
                            job_id: review.id,
                            new_status: JobStatus::Todo,
                        });
                    }
                }
            }
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
    pub fn on_review_submitted(
        &self,
        review_job_id: &JobId,
        submission: &ReviewSubmission,
    ) -> Result<Vec<RuleEffect>, S::Error> {
        let mut effects = Vec::new();
        let craft_jobs = self.store.find_crafts_for_review(review_job_id)?;

        match submission.verdict {
            Verdict::ChangesRequested => {
                // Check escalation threshold
                if submission.round as u32 >= self.max_review_rounds {
                    for craft in &craft_jobs {
                        self.store
                            .update_job_status(&craft.id, JobStatus::Escalated)?;
                        effects.push(RuleEffect::Escalated {
                            job_id: craft.id.clone(),
                            round: submission.round,
                        });
                    }
                    return Ok(effects);
                }

                // Revert craft jobs to in_progress
                for craft in &craft_jobs {
                    self.store
                        .update_job_status(&craft.id, JobStatus::InProgress)?;
                    effects.push(RuleEffect::StatusChanged {
                        job_id: craft.id.clone(),
                        new_status: JobStatus::InProgress,
                    });
                }
            }
            Verdict::Approved => {
                // Check if ALL reviews for each craft job are approved
                for craft in &craft_jobs {
                    let all_reviews = self.store.find_reviews_for_craft(&craft.id)?;
                    let all_approved = all_reviews.iter().all(|r| {
                        if r.id == *review_job_id {
                            return true; // This one is being approved now
                        }
                        let subs = self.store.get_review_submissions(&r.id).unwrap_or_default();
                        subs.last().is_some_and(|s| s.verdict == Verdict::Approved)
                    });
                    if all_approved {
                        self.store.update_job_status(&craft.id, JobStatus::Done)?;
                        effects.push(RuleEffect::StatusChanged {
                            job_id: craft.id.clone(),
                            new_status: JobStatus::Done,
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
        (JobType::Review, JobStatus::Todo, JobStatus::InProgress) => true,
        (JobType::Review, JobStatus::Blocked, JobStatus::Todo) => true,
        (JobType::Review, JobStatus::InProgress, JobStatus::Done) => true,

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
