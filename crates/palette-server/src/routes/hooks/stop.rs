use crate::{AppState, EventRecord};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use palette_domain::agent::{AgentId, AgentStatus};
use palette_domain::job::{JobFilter, JobStatus};
use palette_domain::server::ServerEvent;
use std::sync::Arc;

use super::HookQuery;
use crate::routes::now;

pub async fn handle_stop(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HookQuery>,
    Json(payload): Json<serde_json::Value>,
) -> StatusCode {
    let member_id_str = query.member_id.as_deref().unwrap_or("unknown");
    let member_id = AgentId::new(member_id_str);
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

    // Update member status to Idle and resolve supervisor ID
    let supervisor_id = {
        let mut infra = state.infra.lock().await;
        if let Some(member) = infra.find_member_mut(&member_id) {
            member.status = AgentStatus::Idle;
            let supervisor_id = member.supervisor_id.clone();
            infra.touch();
            Some(supervisor_id)
        } else {
            if let Some(supervisor) = infra.find_supervisor_mut(&member_id) {
                supervisor.status = AgentStatus::Idle;
                infra.touch();
            }
            None
        }
    };

    // Transition member's in_progress jobs and notify supervisors
    if let Some(ref supervisor_id) = supervisor_id {
        let member_jobs = state
            .db
            .list_jobs(&JobFilter {
                assignee: Some(member_id.clone()),
                status: Some(JobStatus::InProgress),
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
                    // Craft jobs: in_progress -> in_review.
                    if let Err(e) = state.db.update_job_status(&job.id, JobStatus::InReview) {
                        tracing::error!(job_id = %job.id, error = %e, "failed to transition job to in_review");
                        continue;
                    }
                    let mut effects = state
                        .rules
                        .on_status_change(&job.id, JobStatus::InReview)
                        .unwrap_or_default();
                    for effect in &effects {
                        tracing::info!(?effect, "rule engine effect (member stop)");
                    }

                    // Always notify orchestrator of the status change so it can
                    // propagate task completion through the task tree.
                    effects.push(palette_domain::rule::RuleEffect::StatusChanged {
                        job_id: job.id.clone(),
                        new_status: JobStatus::InReview,
                    });
                    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
                }
                palette_domain::job::JobType::Review => {
                    // Review jobs: notify the member's supervisor (review integrator)
                    // with findings so it can aggregate and submit a verdict.
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
