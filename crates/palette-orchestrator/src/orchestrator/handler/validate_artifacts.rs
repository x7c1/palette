use palette_domain::job::{Job, JobDetail, JobId};
use palette_domain::task::TaskId;
use palette_domain::worker::WorkerId;
use palette_usecase::task_store::TaskStore;

use super::Orchestrator;

impl Orchestrator {
    /// Log whether a reviewer's review.md artifact exists (informational).
    ///
    /// Individual reviewer artifact validation is enforced synchronously by
    /// the server's submit handler (rejecting submissions without review.md).
    /// This method provides observability in the orchestrator log.
    pub(super) fn log_review_artifact_status(&self, job_id: &JobId) {
        let job = match self.interactor.data_store.get_job(job_id) {
            Ok(Some(j)) if matches!(j.detail, JobDetail::Review { .. }) => j,
            _ => return,
        };
        let task_state = match self.interactor.data_store.get_task_state(&job.task_id) {
            Ok(Some(s)) => s,
            _ => return,
        };
        let task_store = match self.interactor.create_task_store(&task_state.workflow_id) {
            Ok(s) => s,
            Err(_) => return,
        };
        let anchor_job = match self.find_artifact_anchor(&task_store, &job.task_id) {
            Some(j) => j,
            None => return,
        };
        let submissions = match self.interactor.data_store.get_review_submissions(job_id) {
            Ok(s) => s,
            Err(_) => return,
        };
        let round = submissions.last().map(|s| s.round as u32).unwrap_or(1);

        let artifacts_base = self
            .workspace_manager
            .artifacts_path(task_state.workflow_id.as_ref(), anchor_job.id.as_ref());
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
            tracing::debug!(
                job_id = %job_id,
                path = %review_md.display(),
                "review.md artifact not found (server pre-check should have caught this)"
            );
        }
    }

    /// Validate that all child reviewers under an integrator's task have
    /// written their review.md files.
    ///
    /// Returns `true` if all review.md files are present.
    /// For each missing file, enqueues a re-instruction to the reviewer.
    pub(super) fn validate_all_reviewer_artifacts(&self, integrator_job_id: &JobId) -> bool {
        let job = match self.interactor.data_store.get_job(integrator_job_id) {
            Ok(Some(j)) => j,
            Ok(None) => return true,
            Err(e) => {
                tracing::error!(job_id = %integrator_job_id, error = %e, "failed to get integrator job");
                return true;
            }
        };
        let task_state = match self.interactor.data_store.get_task_state(&job.task_id) {
            Ok(Some(s)) => s,
            Ok(None) => return true,
            Err(e) => {
                tracing::error!(error = %e, "failed to get task state for integrator validation");
                return true;
            }
        };
        let task_store = match self.interactor.create_task_store(&task_state.workflow_id) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "failed to create task store for integrator validation");
                return true;
            }
        };

        // Find artifact anchor job (Craft parent or ReviewIntegrate self)
        let anchor_job = match self.find_artifact_anchor(&task_store, &job.task_id) {
            Some(j) => j,
            None => return true,
        };

        // Determine current round
        let submissions = match self
            .interactor
            .data_store
            .get_review_submissions(integrator_job_id)
        {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "failed to get submissions for round detection");
                return true;
            }
        };
        let round = submissions.last().map(|s| s.round as u32).unwrap_or(1);

        let artifacts_base = self
            .workspace_manager
            .artifacts_path(task_state.workflow_id.as_ref(), anchor_job.id.as_ref());

        // If the round directory doesn't exist, no reviewer has written
        // artifacts yet — skip validation. The base directory may exist
        // because assign_new_job pre-creates it for bind mounts.
        let round_dir = artifacts_base.join(format!("round-{round}"));
        if !round_dir.exists() {
            return true;
        }

        // Check each child reviewer's review.md
        let children = task_store.get_child_tasks(&job.task_id);
        let mut all_present = true;

        for child in &children {
            if !matches!(child.job_detail, Some(JobDetail::Review { .. })) {
                continue;
            }
            let child_job = match self.interactor.data_store.get_job_by_task_id(&child.id) {
                Ok(Some(j)) => j,
                Ok(None) => continue,
                Err(e) => {
                    tracing::error!(task_id = %child.id, error = %e, "failed to get child review job");
                    continue;
                }
            };

            let review_md = round_dir.join(child_job.id.to_string()).join("review.md");

            if !review_md.exists() {
                all_present = false;
                tracing::warn!(
                    job_id = %child_job.id,
                    path = %review_md.display(),
                    "review.md missing for child reviewer, sending re-instruction"
                );

                // Re-instruct the reviewer
                if let Some(ref assignee) = child_job.assignee_id {
                    let msg = format!(
                        "## Missing Artifact\n\n\
                         Your review.md file was not found at the expected location.\n\
                         Please write your review to: /home/agent/artifacts/round-{round}/{}/review.md\n\n\
                         Write the file first, then re-submit your review.",
                        child_job.id,
                    );
                    if let Err(e) = self.interactor.data_store.enqueue_message(assignee, &msg) {
                        tracing::error!(error = %e, "failed to enqueue re-instruction for reviewer");
                    }
                    let _ =
                        self.event_tx
                            .send(palette_domain::server::ServerEvent::DeliverMessages {
                                target_id: assignee.clone(),
                            });
                }
            }
        }

        if all_present {
            tracing::info!(
                job_id = %integrator_job_id,
                round,
                "all reviewer artifacts validated for integrator submission"
            );
        } else {
            tracing::warn!(
                job_id = %integrator_job_id,
                round,
                "integrator submission blocked: missing reviewer artifacts"
            );
        }

        all_present
    }

    /// Validate that a ReviewIntegrator wrote integrated-review.json.
    ///
    /// Called after a ReviewIntegrator's stop hook fires. The task_id is the
    /// review composite task whose parent is the craft task.
    pub(super) fn validate_integrated_review_artifact(
        &self,
        task_id: &TaskId,
        worker_id: &WorkerId,
    ) {
        let task_state = match self.interactor.data_store.get_task_state(task_id) {
            Ok(Some(s)) => s,
            _ => return,
        };
        let task_store = match self.interactor.create_task_store(&task_state.workflow_id) {
            Ok(s) => s,
            Err(_) => return,
        };
        let anchor_job = match self.find_artifact_anchor(&task_store, task_id) {
            Some(j) => j,
            None => return,
        };

        // Find the latest round from any child review job's submissions
        let children = task_store.get_child_tasks(task_id);
        let round = children
            .iter()
            .filter(|c| matches!(c.job_detail, Some(JobDetail::Review { .. })))
            .filter_map(|c| match self.interactor.data_store.get_job_by_task_id(&c.id) {
                Ok(j) => j,
                Err(e) => {
                    tracing::error!(task_id = %c.id, error = %e, "failed to get review job for round detection");
                    None
                }
            })
            .filter_map(|j| {
                match self.interactor.data_store.get_review_submissions(&j.id) {
                    Ok(subs) => Some(subs),
                    Err(e) => {
                        tracing::error!(job_id = %j.id, error = %e, "failed to get review submissions for round detection");
                        None
                    }
                }
            })
            .flat_map(|subs| subs.into_iter())
            .map(|s| s.round as u32)
            .max()
            .unwrap_or(1);

        let artifacts_base = self
            .workspace_manager
            .artifacts_path(task_state.workflow_id.as_ref(), anchor_job.id.as_ref());
        let integrated_json = artifacts_base
            .join(format!("round-{round}"))
            .join("integrated-review.json");

        if integrated_json.exists() {
            tracing::debug!(
                task_id = %task_id,
                path = %integrated_json.display(),
                "integrated-review.json artifact validated"
            );
        } else {
            tracing::warn!(
                task_id = %task_id,
                worker_id = %worker_id,
                path = %integrated_json.display(),
                "integrated-review.json artifact missing after integrator stop"
            );
            let msg = format!(
                "## Missing Artifact\n\n\
                 Your integrated-review.json file was not found at the expected location.\n\
                 Please write the integrated review to: /home/agent/artifacts/round-{round}/integrated-review.json\n\n\
                 Follow the format described in your prompt.",
            );
            if let Err(e) = self.interactor.data_store.enqueue_message(worker_id, &msg) {
                tracing::error!(error = %e, "failed to enqueue integrator artifact re-instruction");
            }
            let _ = self
                .event_tx
                .send(palette_domain::server::ServerEvent::DeliverMessages {
                    target_id: worker_id.clone(),
                });
        }
    }

    /// Walk up the task tree from `task_id` to find the nearest ancestor with a
    /// Craft job. Reviewer → composite review → craft, or composite review → craft.
    pub(crate) fn find_ancestor_craft_job(
        &self,
        task_store: &TaskStore<'_>,
        task_id: &TaskId,
    ) -> Option<Job> {
        let mut current_id = task_store.get_task(task_id)?.parent_id?;
        loop {
            let job = self
                .interactor
                .data_store
                .get_job_by_task_id(&current_id)
                .ok()??;
            if matches!(job.detail, JobDetail::Craft { .. }) {
                return Some(job);
            }
            current_id = task_store.get_task(&current_id)?.parent_id?;
        }
    }

    /// Find the job whose ID anchors the artifact path for a review task.
    ///
    /// For Craft-parented reviews, this is the ancestor Craft job (existing behavior).
    /// For standalone PR reviews (no Craft ancestor), this is the ReviewIntegrate
    /// job that serves as the review composite root.
    pub(crate) fn find_artifact_anchor(
        &self,
        task_store: &TaskStore<'_>,
        task_id: &TaskId,
    ) -> Option<Job> {
        // First, try the existing Craft-ancestor path
        if let Some(craft_job) = self.find_ancestor_craft_job(task_store, task_id) {
            return Some(craft_job);
        }

        // Fallback: check if the task itself is ReviewIntegrate, then walk up
        if let Some(job) = self
            .interactor
            .data_store
            .get_job_by_task_id(task_id)
            .ok()?
            && matches!(job.detail, JobDetail::ReviewIntegrate { .. })
        {
            return Some(job);
        }
        let mut current_id = task_store.get_task(task_id)?.parent_id?;
        loop {
            let job = self
                .interactor
                .data_store
                .get_job_by_task_id(&current_id)
                .ok()??;
            if matches!(job.detail, JobDetail::ReviewIntegrate { .. }) {
                return Some(job);
            }
            match task_store.get_task(&current_id)?.parent_id {
                Some(pid) => current_id = pid,
                None => return None,
            }
        }
    }
}
