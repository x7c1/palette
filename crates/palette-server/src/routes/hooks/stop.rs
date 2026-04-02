use crate::{AppState, EventRecord};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use palette_domain::job::{CraftTransition, Job, JobStatus, JobType};
use palette_domain::server::ServerEvent;
use palette_domain::worker::{WorkerId, WorkerRole, WorkerStatus};
use std::sync::Arc;

use super::HookQuery;
use crate::routes::now;

pub async fn handle_stop(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HookQuery>,
    Json(payload): Json<serde_json::Value>,
) -> StatusCode {
    let worker_id_str = query.worker_id.as_deref().unwrap_or("unknown");
    let worker_id = match WorkerId::parse(worker_id_str) {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!(worker_id = worker_id_str, error = ?e, "invalid worker_id in stop hook");
            return StatusCode::BAD_REQUEST;
        }
    };
    tracing::info!(worker_id = worker_id_str, payload = %payload, "received stop hook");

    let record = EventRecord {
        timestamp: now(),
        event_type: "stop".to_string(),
        payload: serde_json::json!({
            "worker_id": worker_id_str,
            "original": payload,
        }),
    };
    state.event_log.lock().await.push(record);

    super::save_session_id(state.interactor.data_store.as_ref(), &worker_id, &payload);

    // If the stopping worker is a ReviewIntegrator, validate integrated-review.md
    if let Ok(Some(worker)) = state.interactor.data_store.find_worker(&worker_id)
        && worker.role == WorkerRole::ReviewIntegrator
    {
        let _ = state.event_tx.send(ServerEvent::ReviewIntegratorStopped {
            task_id: worker.task_id.clone(),
            worker_id: worker_id.clone(),
        });
    }

    let supervisor_id = transition_worker_to_idle(&state, &worker_id);

    if let Some(ref supervisor_id) = supervisor_id {
        let last_message = payload
            .get("last_assistant_message")
            .and_then(|v| v.as_str())
            .unwrap_or("(work completed)");

        process_member_jobs(&state, &worker_id, supervisor_id, last_message);
    }

    // Fire-and-forget: deliver queued messages to the now-idle worker
    let _ = state.event_tx.send(ServerEvent::DeliverMessages {
        target_id: worker_id,
    });
    let _ = state.event_tx.send(ServerEvent::NotifyDeliveryLoop);

    StatusCode::OK
}

/// Transition the worker to Idle and return its supervisor ID (if it's an active Member).
///
/// Only transitions workers that were actively working. A Booting worker that fires
/// a stop hook means its startup failed (e.g. `--resume` "No conversation found") —
/// leave it in Booting so the readiness watcher can detect the failure and fall back
/// to a fresh start.
fn transition_worker_to_idle(state: &AppState, worker_id: &WorkerId) -> Option<WorkerId> {
    let worker = match state.interactor.data_store.find_worker(worker_id) {
        Ok(Some(w)) => w,
        _ => return None,
    };

    let should_idle = matches!(
        worker.status,
        WorkerStatus::Working | WorkerStatus::WaitingPermission | WorkerStatus::Idle
    );

    if !should_idle {
        tracing::info!(
            worker_id = %worker_id,
            status = ?worker.status,
            "stop hook ignored: worker not in active state"
        );
        return None;
    }

    if let Err(e) = state
        .interactor
        .data_store
        .update_worker_status(worker_id, WorkerStatus::Idle)
    {
        tracing::error!(error = %e, "failed to update worker status to idle");
    }

    if worker.role == WorkerRole::Member {
        worker.supervisor_id
    } else {
        None
    }
}

/// Transition the member's in-progress jobs and notify the supervisor.
fn process_member_jobs(
    state: &AppState,
    worker_id: &WorkerId,
    supervisor_id: &WorkerId,
    last_message: &str,
) {
    let jobs = match state
        .interactor
        .data_store
        .list_jobs(&palette_domain::job::JobFilter {
            assignee_id: Some(worker_id.clone()),
            ..Default::default()
        }) {
        Ok(jobs) => jobs,
        Err(e) => {
            tracing::error!(worker_id = %worker_id, error = %e, "failed to list jobs for stop transition");
            return;
        }
    };

    if jobs.is_empty() {
        let notification = format!("[event] member={worker_id} type=stop");
        if let Err(e) = state
            .interactor
            .data_store
            .enqueue_message(supervisor_id, &notification)
        {
            tracing::error!(error = %e, "failed to enqueue stop notification for supervisor");
        }
        return;
    }

    for job in &jobs {
        match job.job_type {
            JobType::Craft => handle_craft_stop(state, job),
            JobType::Review => {
                handle_review_stop(state, job, worker_id, supervisor_id, last_message)
            }
            // ReviewIntegrate, Orchestrator, and Operator jobs don't have member workers
            JobType::ReviewIntegrate | JobType::Orchestrator | JobType::Operator => {}
        }
    }
}

/// Transition a craft job to InReview on member stop.
fn handle_craft_stop(state: &AppState, job: &Job) {
    let JobStatus::Craft(current) = job.status else {
        return;
    };
    let in_review = match CraftTransition::SubmitForReview.validate(current) {
        Ok(status) => status,
        Err(e) => {
            tracing::info!(
                job_id = %job.id,
                error = %e,
                "skipping craft stop transition"
            );
            return;
        }
    };
    if let Err(e) = state
        .interactor
        .data_store
        .update_job_status(&job.id, in_review)
    {
        tracing::error!(job_id = %job.id, error = %e, "failed to transition job to in_review");
        return;
    }
    let _ = state.event_tx.send(ServerEvent::CraftReadyForReview {
        craft_job_id: job.id.clone(),
    });
}

/// Notify the ReviewIntegrator supervisor that a review member has stopped.
fn handle_review_stop(
    state: &AppState,
    job: &Job,
    worker_id: &WorkerId,
    supervisor_id: &WorkerId,
    last_message: &str,
) {
    // Only notify ReviewIntegrator supervisors (for multi-review aggregation).
    // Single-review results are handled mechanically by the orchestrator.
    let is_review_integrator = match state.interactor.data_store.find_worker(supervisor_id) {
        Ok(Some(s)) => s.role == WorkerRole::ReviewIntegrator,
        Ok(None) => false,
        Err(e) => {
            tracing::error!(error = %e, supervisor_id = %supervisor_id, "failed to find supervisor for review notification");
            false
        }
    };
    if !is_review_integrator {
        return;
    }
    let report_msg = format!(
        "[review] member={} job={} type=review_complete message: {}",
        worker_id, job.id, last_message,
    );
    if let Err(e) = state
        .interactor
        .data_store
        .enqueue_message(supervisor_id, &report_msg)
    {
        tracing::error!(error = %e, "failed to enqueue review report");
    }
}
