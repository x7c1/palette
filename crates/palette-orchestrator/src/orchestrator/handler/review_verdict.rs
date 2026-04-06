use super::Orchestrator;
use super::PendingActions;
use palette_domain::job::{CraftStatus, CraftTransition, JobDetail, JobId, JobStatus};
use palette_domain::review::Verdict;

impl Orchestrator {
    /// Handle a review verdict (Approved or ChangesRequested).
    pub(crate) fn handle_review_verdict(
        &self,
        review_job_id: &JobId,
        verdict: Verdict,
    ) -> crate::Result<PendingActions> {
        match verdict {
            Verdict::Approved => self.handle_review_approved(review_job_id),
            Verdict::ChangesRequested => self.handle_review_changes_requested(review_job_id),
        }
    }

    /// When a review is approved: check if all sibling reviews are done,
    /// complete the parent craft job if so, and try to complete the task.
    fn handle_review_approved(&self, review_job_id: &JobId) -> crate::Result<PendingActions> {
        let result = self
            .try_complete_parent_craft_job(review_job_id)?
            .merge(self.try_complete_task_by_job(review_job_id)?);
        Ok(result)
    }

    /// When a review requests changes: revert the parent craft job to InProgress.
    fn handle_review_changes_requested(
        &self,
        review_job_id: &JobId,
    ) -> crate::Result<PendingActions> {
        self.revert_parent_craft_to_in_progress(review_job_id)
    }

    /// When a review job becomes Done, check if all sibling review tasks under
    /// the parent craft task are also done. If so, transition the parent craft job
    /// from InReview to Done.
    fn try_complete_parent_craft_job(
        &self,
        review_job_id: &JobId,
    ) -> crate::Result<PendingActions> {
        let Some(review_job) = self.interactor.data_store.get_job(review_job_id)? else {
            return Ok(PendingActions::new());
        };
        let review_task_id = &review_job.task_id;
        let Some(task_state) = self.interactor.data_store.get_task_state(review_task_id)? else {
            return Ok(PendingActions::new());
        };

        let task_store = self.interactor.create_task_store(&task_state.workflow_id)?;

        let Some(review_task) = task_store.get_task(review_task_id) else {
            return Ok(PendingActions::new());
        };

        let Some(ref parent_id) = review_task.parent_id else {
            return Ok(PendingActions::new());
        };

        let Some(craft_job) = self.interactor.data_store.get_job_by_task_id(parent_id)? else {
            return Ok(PendingActions::new());
        };
        if craft_job.status != JobStatus::Craft(CraftStatus::InReview) {
            return Ok(PendingActions::new());
        }

        // Check if ALL review children of the parent have their jobs Done
        let siblings = task_store.get_child_tasks(parent_id);
        let all_reviews_done = siblings.iter().all(|child| {
            if !matches!(child.job_detail, Some(JobDetail::Review { .. })) {
                return true;
            }
            match self.interactor.data_store.get_job_by_task_id(&child.id) {
                Ok(Some(j)) => j.status.is_done(),
                Ok(None) => false,
                Err(e) => {
                    tracing::error!(error = %e, task_id = %child.id, "failed to get job for review completion check");
                    false
                }
            }
        });

        if !all_reviews_done {
            return Ok(PendingActions::new());
        }

        // All review children are done — transition craft job to Done
        self.interactor
            .data_store
            .update_job_status(&craft_job.id, CraftTransition::Approve.to_job_status())?;
        tracing::info!(
            craft_job_id = %craft_job.id,
            "craft job completed (all child reviews done)"
        );

        // Craft job completed → destroy crafter member + complete task
        if let Some(ref assignee) = craft_job.assignee_id {
            self.destroy_member(assignee);
        }
        self.try_complete_task_by_job(&craft_job.id)
    }

    /// When a review job gets ChangesRequested, move the parent craft job
    /// from InReview back to InProgress so the crafter can address feedback.
    fn revert_parent_craft_to_in_progress(
        &self,
        review_job_id: &JobId,
    ) -> crate::Result<PendingActions> {
        let Some(review_job) = self.interactor.data_store.get_job(review_job_id)? else {
            return Ok(PendingActions::new());
        };
        let review_task_id = &review_job.task_id;
        let Some(task_state) = self.interactor.data_store.get_task_state(review_task_id)? else {
            return Ok(PendingActions::new());
        };

        let task_store = self.interactor.create_task_store(&task_state.workflow_id)?;

        let Some(review_task) = task_store.get_task(review_task_id) else {
            return Ok(PendingActions::new());
        };

        let Some(ref parent_id) = review_task.parent_id else {
            return Ok(PendingActions::new());
        };

        let Some(craft_job) = self.interactor.data_store.get_job_by_task_id(parent_id)? else {
            return Ok(PendingActions::new());
        };
        if craft_job.status != JobStatus::Craft(CraftStatus::InReview) {
            return Ok(PendingActions::new());
        }

        // Move craft job back to InProgress
        self.interactor.data_store.update_job_status(
            &craft_job.id,
            CraftTransition::RequestChanges.to_job_status(),
        )?;
        tracing::info!(
            craft_job_id = %craft_job.id,
            review_job_id = %review_job_id,
            "craft job reverted to InProgress due to changes_requested"
        );

        // Enqueue review feedback to the crafter
        if let Some(ref assignee) = craft_job.assignee_id {
            let submissions = self
                .interactor
                .data_store
                .get_review_submissions(review_job_id)?;
            let feedback = submissions
                .last()
                .and_then(|s| s.summary.clone())
                .unwrap_or_else(|| "Changes requested (no summary provided)".to_string());
            let msg = format!(
                "## Review Feedback (changes requested)\n\nReview job {} has requested changes:\n\n{}\n\nPlease address the feedback and complete the task.",
                review_job_id, feedback
            );
            self.interactor.data_store.enqueue_message(assignee, &msg)?;
        }

        // Reactivate the crafter member
        if let Some(ref assignee) = craft_job.assignee_id {
            self.reactivate_member(&craft_job.id, assignee)
        } else {
            Ok(PendingActions::new())
        }
    }
}
