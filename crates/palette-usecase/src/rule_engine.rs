use crate::DataStore;
use palette_domain::job::{JobError, JobId, ReviewTransition};
use palette_domain::review::{ReviewSubmission, Verdict};
use palette_domain::rule::RuleEffect;

pub struct RuleEngine<'a> {
    store: &'a dyn DataStore,
    #[allow(dead_code)]
    // TODO: re-enable when escalation trigger conditions are designed
    max_review_rounds: u32,
}

impl<'a> RuleEngine<'a> {
    pub fn new(store: &'a dyn DataStore, max_review_rounds: u32) -> Self {
        Self {
            store,
            max_review_rounds,
        }
    }

    /// Apply rules after a new job is created (Todo status). Returns side effects.
    pub fn on_job_created(
        &self,
        job_id: &JobId,
    ) -> Result<Vec<RuleEffect>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![RuleEffect::AssignNewJob {
            job_id: job_id.clone(),
        }])
    }

    /// Apply rules after a craft job completes. Returns side effects.
    pub fn on_craft_done(
        &self,
        job_id: &JobId,
    ) -> Result<Vec<RuleEffect>, Box<dyn std::error::Error + Send + Sync>> {
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
    ) -> Result<Vec<RuleEffect>, Box<dyn std::error::Error + Send + Sync>> {
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
                    ReviewTransition::RequestChanges.to_job_status(),
                )?;
            }
            Verdict::Approved => {
                self.store
                    .update_job_status(review_job_id, ReviewTransition::Approve.to_job_status())?;
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
