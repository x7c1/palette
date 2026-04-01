use crate::api_types::{
    ErrorCode, InputError, Location, ResourceKind, ReviewSubmissionResponse, SubmitReviewRequest,
};
use crate::{AppState, Error, ValidJson};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use palette_domain::job::{JobId, JobType};
use palette_domain::server::ServerEvent;
use palette_usecase::RuleEngine;
use std::sync::Arc;

pub async fn handle_submit_review(
    State(state): State<Arc<AppState>>,
    Path(review_job_id): Path<String>,
    ValidJson(api_req): ValidJson<SubmitReviewRequest>,
) -> crate::Result<(StatusCode, Json<ReviewSubmissionResponse>)> {
    let review_job_id =
        JobId::parse(review_job_id).map_err(Error::invalid_path("review_job_id"))?;
    let req = api_req.validate().map_err(|errors| Error::BadRequest {
        code: ErrorCode::InputValidationFailed,
        errors,
    })?;

    // Verify the job exists and is a review
    let job = state
        .interactor
        .data_store
        .get_job(&review_job_id)
        .map_err(Error::internal)?
        .ok_or_else(|| Error::NotFound {
            resource: ResourceKind::Job,
            id: review_job_id.to_string(),
        })?;

    if job.job_type != JobType::Review {
        return Err(Error::BadRequest {
            code: ErrorCode::NotReviewJob,
            errors: vec![InputError {
                location: Location::Path,
                hint: "review_job_id".into(),
                reason: "job/not_review_job".into(),
            }],
        });
    }

    // Determine if this is an integrator submission (composite review task with children)
    // or a regular reviewer submission. Must happen before submit_review so we can
    // reject integrator submissions when child reviewers are incomplete (preventing
    // the submission from being recorded and avoiding round number drift).
    let child_tasks = match state.interactor.data_store.get_task_state(&job.task_id) {
        Ok(Some(ts)) => match state.interactor.create_task_store(&ts.workflow_id) {
            Ok(task_store) => task_store.get_child_tasks(&job.task_id),
            Err(e) => {
                tracing::error!(error = %e, "failed to create task store for integrator check");
                vec![]
            }
        },
        Ok(None) => vec![],
        Err(e) => {
            tracing::error!(error = %e, "failed to get task state for integrator check");
            vec![]
        }
    };
    let is_integrator = !child_tasks.is_empty();

    // For integrator submissions, verify all child reviewer jobs are Done.
    // This prevents the integrator from finalizing before all reviews are in,
    // and avoids recording a submission that would cause round number drift.
    if is_integrator {
        let incomplete: Vec<String> = child_tasks
            .iter()
            .filter(|child| child.job_type == Some(JobType::Review))
            .filter_map(
                |child| match state.interactor.data_store.get_job_by_task_id(&child.id) {
                    Ok(Some(j)) if j.status.is_done() => None,
                    Ok(Some(j)) => Some(format!("{} (status: {})", j.id, j.status)),
                    Ok(None) => Some(format!("task {} (no job)", child.id)),
                    Err(e) => {
                        tracing::error!(task_id = %child.id, error = %e, "failed to get child job");
                        Some(format!("task {} (error)", child.id))
                    }
                },
            )
            .collect();

        if !incomplete.is_empty() {
            tracing::warn!(
                review_job_id = %review_job_id,
                incomplete = ?incomplete,
                "integrator submit rejected: child reviewers incomplete"
            );
            return Err(Error::BadRequest {
                code: ErrorCode::ChildReviewersIncomplete,
                errors: vec![InputError {
                    location: Location::Body,
                    hint: "verdict".into(),
                    reason: format!("child reviewer jobs not done: {}", incomplete.join(", ")),
                }],
            });
        }
    }

    // For individual reviewer submissions, verify that review.md exists before
    // recording the submission. This prevents round number drift: if the reviewer
    // submits without writing review.md, we reject synchronously rather than
    // recording a submission that the async validator will later discard.
    if !is_integrator
        && let Some(review_md_path) = find_reviewer_artifact_path(&state, &job)
        && !review_md_path.exists()
    {
        tracing::warn!(
            review_job_id = %review_job_id,
            path = %review_md_path.display(),
            "reviewer submit rejected: review.md not found"
        );
        return Err(Error::BadRequest {
            code: ErrorCode::ReviewArtifactMissing,
            errors: vec![InputError {
                location: Location::Body,
                hint: "verdict".into(),
                reason: "review.md not found at expected path; write the file before submitting"
                    .to_string(),
            }],
        });
    }

    let submission = state
        .interactor
        .data_store
        .submit_review(&review_job_id, &req)
        .map_err(Error::internal)?;

    // Apply rule engine
    let effects = RuleEngine::new(
        state.interactor.data_store.as_ref(),
        state.max_review_rounds,
    )
    .on_review_submitted(&review_job_id, &submission)
    .map_err(Error::internal)?;

    for effect in &effects {
        tracing::info!(?effect, "review rule engine effect");
    }

    // Notify the review member's supervisor about review results
    if let Some(ref assignee) = job.assignee_id
        && let Ok(Some(member)) = state.interactor.data_store.find_worker(assignee)
        && let Some(ref supervisor_id) = member.supervisor_id
        && let Ok(Some(supervisor)) = state.interactor.data_store.find_worker(supervisor_id)
    {
        let verdict_str = match submission.verdict {
            palette_domain::review::Verdict::Approved => "approved",
            palette_domain::review::Verdict::ChangesRequested => "changes_requested",
        };
        let notification = format!("[event] review={review_job_id} type={verdict_str}");
        let _ = state
            .interactor
            .data_store
            .enqueue_message(&supervisor.id, &notification);
        tracing::info!(
            review_job_id = %review_job_id,
            verdict = verdict_str,
            supervisor_id = %supervisor.id,
            "notified supervisor of review result"
        );
    }

    if is_integrator {
        // Integrator submission: validate all child reviewers' review.md files
        let _ = state
            .event_tx
            .send(ServerEvent::ValidateIntegratorSubmission {
                job_id: review_job_id.clone(),
                pending_effects: effects,
            });
    } else if let Some(ref assignee) = job.assignee_id {
        // Regular reviewer: validate this reviewer's review.md
        let _ = state.event_tx.send(ServerEvent::ValidateReviewArtifact {
            job_id: review_job_id.clone(),
            worker_id: assignee.clone(),
            pending_effects: effects,
        });
    } else if !effects.is_empty() {
        let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    }
    let _ = state.event_tx.send(ServerEvent::NotifyDeliveryLoop);

    Ok((
        StatusCode::CREATED,
        Json(ReviewSubmissionResponse::from(submission)),
    ))
}

/// Compute the host-side path to a reviewer's review.md artifact.
///
/// Traverses the task tree upward to find the ancestor craft job, then builds
/// the path: `{data_dir}/artifacts/{workflow_id}/{craft_job_id}/round-{N}/{review_job_id}/review.md`
///
/// Returns `None` if any lookup fails (missing task, missing parent, etc.).
fn find_reviewer_artifact_path(
    state: &AppState,
    job: &palette_domain::job::Job,
) -> Option<std::path::PathBuf> {
    let task_state = state
        .interactor
        .data_store
        .get_task_state(&job.task_id)
        .ok()??;
    let task_store = state
        .interactor
        .create_task_store(&task_state.workflow_id)
        .ok()?;

    // Walk up the task tree to find the ancestor craft job.
    // Reviewer task → composite review task → craft task.
    let mut current_id = task_store.get_task(&job.task_id)?.parent_id?;
    let craft_job = loop {
        let j = state
            .interactor
            .data_store
            .get_job_by_task_id(&current_id)
            .ok()??;
        if j.job_type == JobType::Craft {
            break j;
        }
        current_id = task_store.get_task(&current_id)?.parent_id?;
    };

    // Round = existing submissions count + 1 (next round).
    // After a successful submit (not rejected), this increments correctly.
    let submissions = state
        .interactor
        .data_store
        .get_review_submissions(&job.id)
        .ok()?;
    let round = (submissions.len() as u32) + 1;

    let path = state
        .data_dir
        .join("artifacts")
        .join(task_state.workflow_id.as_ref())
        .join(craft_job.id.as_ref())
        .join(format!("round-{round}"))
        .join(job.id.to_string())
        .join("review.md");

    Some(path)
}
