use crate::{AppState, EventRecord};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use palette_domain::job::{CraftStatus, JobFilter, JobStatus};
use palette_domain::server::ServerEvent;
use palette_domain::worker::{WorkerId, WorkerStatus};
use std::sync::Arc;

use super::HookQuery;
use crate::routes::now;

pub async fn handle_stop(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HookQuery>,
    Json(payload): Json<serde_json::Value>,
) -> StatusCode {
    let member_id_str = query.member_id.as_deref().unwrap_or("unknown");
    let member_id = WorkerId::new(member_id_str);
    tracing::info!(member_id = member_id_str, payload = %payload, "received stop hook");

    let record = EventRecord {
        timestamp: now(),
        event_type: "stop".to_string(),
        payload: serde_json::json!({
            "member_id": member_id_str,
            "original": payload,
        }),
    };
    state.event_log.lock().await.push(record);

    // Update worker status to Idle and resolve supervisor ID
    let supervisor_id = {
        match state.db.find_worker(&member_id) {
            Ok(Some(worker)) => {
                if let Err(e) = state
                    .db
                    .update_worker_status(&member_id, WorkerStatus::Idle)
                {
                    tracing::error!(error = %e, "failed to update worker status to idle");
                }
                if worker.role == palette_domain::worker::WorkerRole::Member {
                    Some(worker.supervisor_id.clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    };

    // Transition member's in_progress jobs and notify supervisors
    if let Some(ref supervisor_id) = supervisor_id {
        let member_jobs = state
            .db
            .list_jobs(&JobFilter {
                assignee_id: Some(member_id.clone()),
                ..Default::default()
            })
            .unwrap_or_default();

        let last_message = payload
            .get("last_assistant_message")
            .and_then(|v| v.as_str())
            .unwrap_or("(work completed)");

        for job in &member_jobs {
            match job.job_type {
                palette_domain::job::JobType::Craft => {
                    let in_review = JobStatus::Craft(CraftStatus::InReview);
                    if palette_domain::rule::validate_transition(job.status, in_review).is_err() {
                        tracing::info!(
                            job_id = %job.id,
                            status = ?job.status,
                            "skipping craft stop transition (invalid transition)"
                        );
                        continue;
                    }
                    if let Err(e) = state.db.update_job_status(&job.id, in_review) {
                        tracing::error!(job_id = %job.id, error = %e, "failed to transition job to in_review");
                        continue;
                    }
                    let effects = vec![palette_domain::rule::RuleEffect::CraftReadyForReview {
                        craft_job_id: job.id.clone(),
                    }];
                    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
                }
                palette_domain::job::JobType::Review => {
                    // Only notify ReviewIntegrator supervisors (for multi-review aggregation).
                    // Single-review results are handled mechanically by the orchestrator.
                    let is_review_integrator = state
                        .db
                        .find_worker(supervisor_id)
                        .ok()
                        .flatten()
                        .is_some_and(|s| {
                            s.role == palette_domain::worker::WorkerRole::ReviewIntegrator
                        });
                    if is_review_integrator {
                        let report_msg = format!(
                            "[review] member={} job={} type=review_complete message: {}",
                            member_id, job.id, last_message,
                        );
                        if let Err(e) = state.db.enqueue_message(supervisor_id, &report_msg) {
                            tracing::error!(error = %e, "failed to enqueue review report");
                        }
                    }
                }
            }
        }

        if member_jobs.is_empty() {
            // No jobs to transition; just send a stop event
            let notification = format!("[event] member={member_id} type=stop");
            if let Err(e) = state.db.enqueue_message(supervisor_id, &notification) {
                tracing::error!(error = %e, "failed to enqueue stop notification for supervisor");
            }
        }
    }

    // Fire-and-forget: deliver queued messages to the now-idle member
    let _ = state.event_tx.send(ServerEvent::DeliverMessages {
        target_id: member_id,
    });
    let _ = state.event_tx.send(ServerEvent::NotifyDeliveryLoop);

    StatusCode::OK
}
