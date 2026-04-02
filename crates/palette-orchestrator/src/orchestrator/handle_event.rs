use super::Orchestrator;
use super::process_effects::EffectResult;
use palette_domain::job::{JobId, JobType, ReviewTransition};
use palette_domain::review::Verdict;
use palette_domain::server::ServerEvent;
use std::sync::Arc;

impl Orchestrator {
    pub(super) async fn handle_event(self: &Arc<Self>, event: ServerEvent) {
        match event {
            // --- Domain events ---
            ServerEvent::CraftDone { job_id } => {
                self.handle_craft_done(&job_id).await;
            }
            ServerEvent::CraftReadyForReview { craft_job_id } => {
                self.handle_craft_ready_for_review(&craft_job_id).await;
            }
            ServerEvent::ReviewSubmitted { review_job_id } => {
                self.handle_review_submitted(&review_job_id).await;
            }
            ServerEvent::ReviewIntegratorStopped { task_id, worker_id } => {
                self.validate_integrated_review_artifact(&task_id, &worker_id);
            }

            // --- Workflow lifecycle ---
            ServerEvent::ActivateWorkflow { workflow_id } => {
                self.handle_activate_workflow(&workflow_id).await;
            }
            ServerEvent::ActivateNewTasks { workflow_id } => {
                self.handle_activate_new_tasks(&workflow_id).await;
            }

            // --- Infrastructure events ---
            ServerEvent::DeliverMessages { target_id } => {
                let _ = self.deliver_queued_messages(&target_id);
            }
            ServerEvent::NotifyDeliveryLoop => self.deliver_to_all_idle(),
            ServerEvent::ResumeWorkers { worker_ids } => {
                for worker_id in worker_ids {
                    self.spawn_readiness_watcher(worker_id);
                }
                // Re-assign jobs that were deferred during suspend.
                // Delayed to give workers time to boot and become ready.
                let this = Arc::clone(self);
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                    this.assign_deferred_jobs();
                });
            }
            ServerEvent::SuspendWorkflow { workflow_id } => {
                let this = Arc::clone(self);
                tokio::task::spawn_blocking(move || this.suspend(&workflow_id));
            }
            ServerEvent::OrchestratorTaskCompleted {
                job_id,
                success,
                stdout,
                stderr,
                exit_code,
                duration_ms,
            } => {
                self.handle_orchestrator_task_completed(
                    &job_id,
                    success,
                    &stdout,
                    &stderr,
                    exit_code,
                    duration_ms,
                )
                .await;
            }
        }
    }

    /// A craft job has been marked Done.
    /// Destroy the crafter member and cascade task completion.
    async fn handle_craft_done(self: &Arc<Self>, job_id: &JobId) {
        let mut result = EffectResult::new();

        let outcome: crate::Result<()> = (|| {
            let job = match self.interactor.data_store.get_job(job_id)? {
                Some(j) => j,
                None => {
                    tracing::error!(job_id = %job_id, "CraftDone: job not found");
                    return Ok(());
                }
            };

            if let Some(ref assignee) = job.assignee_id {
                self.destroy_member(assignee);
            }
            self.complete_job(job_id, &mut result)?;
            Ok(())
        })();

        if let Err(e) = outcome {
            tracing::error!(error = %e, job_id = %job_id, "failed to handle CraftDone");
        }
        self.dispatch_effect_result(result);
    }

    /// A craft job has reached InReview.
    /// Activate child review tasks.
    async fn handle_craft_ready_for_review(self: &Arc<Self>, craft_job_id: &JobId) {
        let mut result = EffectResult::new();
        if let Err(e) = self.activate_child_review_tasks(craft_job_id, &mut result) {
            tracing::error!(error = %e, craft_job_id = %craft_job_id, "failed to activate review tasks");
        }
        self.dispatch_effect_result(result);
    }

    /// A review submission has been recorded.
    /// Validate artifacts and handle the verdict.
    async fn handle_review_submitted(self: &Arc<Self>, review_job_id: &JobId) {
        let mut result = EffectResult::new();

        let outcome: crate::Result<()> = (|| {
            let job = match self.interactor.data_store.get_job(review_job_id)? {
                Some(j) => j,
                None => {
                    tracing::error!(review_job_id = %review_job_id, "ReviewSubmitted: job not found");
                    return Ok(());
                }
            };

            let is_integrator = job.job_type == JobType::ReviewIntegrate;

            // Validate artifacts. Integrator submissions are validated by the
            // orchestrator (all child review.md must exist). Individual reviewer
            // submissions are validated synchronously by the server's submit
            // handler; the orchestrator logs the result for observability only.
            if is_integrator {
                if !self.validate_all_reviewer_artifacts(review_job_id) {
                    return Ok(());
                }
            } else {
                self.log_review_artifact_status(review_job_id);
            }

            // Get the latest submission to determine the verdict
            let submissions = self
                .interactor
                .data_store
                .get_review_submissions(review_job_id)?;
            let submission = submissions
                .last()
                .ok_or_else(|| crate::Error::InvalidTaskState {
                    task_id: job.task_id.clone(),
                    detail: "no submissions found for review job".into(),
                })?;
            let verdict = submission.verdict;

            // Apply status transition (previously done by RuleEngine)
            match verdict {
                Verdict::ChangesRequested => {
                    self.interactor.data_store.update_job_status(
                        review_job_id,
                        ReviewTransition::RequestChanges.to_job_status(),
                    )?;
                }
                Verdict::Approved => {
                    self.interactor.data_store.update_job_status(
                        review_job_id,
                        ReviewTransition::Approve.to_job_status(),
                    )?;
                    if let Some(ref assignee) = job.assignee_id {
                        self.destroy_member(assignee);
                    }
                }
            }

            // Handle the verdict (cascade effects)
            self.handle_review_verdict(review_job_id, verdict, &mut result)?;

            Ok(())
        })();

        if let Err(e) = outcome {
            tracing::error!(error = %e, review_job_id = %review_job_id, "failed to handle ReviewSubmitted");
        }
        self.dispatch_effect_result(result);
    }

    /// A new workflow was created. Activate root and initial tasks.
    async fn handle_activate_workflow(
        self: &Arc<Self>,
        workflow_id: &palette_domain::workflow::WorkflowId,
    ) {
        let mut result = EffectResult::new();
        if let Err(e) = self.activate_workflow(workflow_id, &mut result) {
            tracing::error!(error = %e, workflow_id = %workflow_id, "failed to activate workflow");
        }
        self.dispatch_effect_result(result);
    }

    /// Blueprint re-applied; activate new tasks.
    async fn handle_activate_new_tasks(
        self: &Arc<Self>,
        workflow_id: &palette_domain::workflow::WorkflowId,
    ) {
        let mut result = EffectResult::new();
        if let Err(e) = self.activate_new_tasks(workflow_id, &mut result) {
            tracing::error!(error = %e, workflow_id = %workflow_id, "failed to activate new tasks");
        }
        self.dispatch_effect_result(result);
    }

    /// Dispatch the accumulated results: deliver messages and spawn readiness watchers.
    pub(super) fn dispatch_effect_result(self: &Arc<Self>, result: EffectResult) {
        for d in &result.deliveries {
            let _ = self.deliver_queued_messages(&d.target_id);
        }
        for d in result.deliveries {
            self.spawn_readiness_watcher(d.target_id);
        }
        for sup_id in result.spawned_supervisors {
            self.spawn_readiness_watcher(sup_id);
        }
    }
}
