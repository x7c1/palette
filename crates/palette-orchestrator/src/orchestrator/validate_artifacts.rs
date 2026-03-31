use palette_domain::job::{JobId, JobType};
use palette_domain::worker::WorkerId;

use super::Orchestrator;

impl Orchestrator {
    /// Validate that a reviewer wrote their review.md artifact.
    ///
    /// Called after a reviewer's stop hook fires. If the file is missing,
    /// enqueue a re-instruction message to the reviewer.
    pub(super) fn validate_review_artifact(&self, job_id: &JobId, worker_id: &WorkerId) {
        let job = match self.interactor.data_store.get_job(job_id) {
            Ok(Some(j)) => j,
            _ => return,
        };
        if job.job_type != JobType::Review {
            return;
        }

        let task_state = match self.interactor.data_store.get_task_state(&job.task_id) {
            Ok(Some(s)) => s,
            _ => return,
        };

        // Find the parent craft job to determine the artifacts path
        let task_store = match self.interactor.create_task_store(&task_state.workflow_id) {
            Ok(s) => s,
            Err(_) => return,
        };
        let task = match task_store.get_task(&job.task_id) {
            Some(t) => t,
            None => return,
        };
        let parent_id = match task.parent_id.as_ref() {
            Some(id) => id,
            None => return,
        };
        let craft_job = match self.interactor.data_store.get_job_by_task_id(parent_id) {
            Ok(Some(j)) => j,
            _ => return,
        };

        // Determine the round number
        let submissions = match self.interactor.data_store.get_review_submissions(job_id) {
            Ok(s) => s,
            Err(_) => return,
        };
        // After a successful review stop, the submission was already recorded,
        // so the current round is the latest submission's round.
        let round = submissions.last().map(|s| s.round as u32).unwrap_or(1);

        let artifacts_base = self
            .workspace_manager
            .artifacts_path(task_state.workflow_id.as_ref(), craft_job.id.as_ref());
        let review_md = artifacts_base
            .join(format!("round-{round}"))
            .join(job.id.to_string())
            .join("review.md");

        if review_md.exists() {
            tracing::debug!(
                job_id = %job_id,
                path = %review_md.display(),
                "review.md artifact validated"
            );
        } else {
            tracing::warn!(
                job_id = %job_id,
                worker_id = %worker_id,
                path = %review_md.display(),
                "review.md artifact missing after reviewer stop"
            );
            // Enqueue a re-instruction message to the reviewer
            let msg = format!(
                "## Missing Artifact\n\n\
                 Your review.md file was not found at the expected location.\n\
                 Please write your review to: /home/agent/artifacts/round-{round}/{}/review.md\n\n\
                 Follow the format described in your prompt.",
                job.id,
            );
            if let Err(e) = self.interactor.data_store.enqueue_message(worker_id, &msg) {
                tracing::error!(error = %e, "failed to enqueue review artifact re-instruction");
            }
            // Deliver the message to the idle worker
            let _ = self
                .event_tx
                .send(palette_domain::server::ServerEvent::DeliverMessages {
                    target_id: worker_id.clone(),
                });
        }
    }
}
