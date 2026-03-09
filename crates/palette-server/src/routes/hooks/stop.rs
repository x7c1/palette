use crate::{AppState, EventRecord};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use palette_domain::{AgentId, AgentStatus, ServerEvent, TaskFilter, TaskStatus};
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

    // Update member status to Idle and resolve leader ID
    let leader_id = {
        let mut infra = state.infra.lock().await;
        if let Some(member) = infra.find_member_mut(&member_id) {
            member.status = AgentStatus::Idle;
            let leader_id = member.leader_id.clone();
            infra.touch();
            Some(leader_id)
        } else {
            if let Some(leader) = infra.find_leader_mut(&member_id) {
                leader.status = AgentStatus::Idle;
                infra.touch();
            }
            None
        }
    };

    // Transition member's in_progress tasks to in_review and notify leader
    if let Some(ref leader_id) = leader_id {
        let member_tasks = state
            .db
            .list_tasks(&TaskFilter {
                assignee: Some(member_id.clone()),
                status: Some(TaskStatus::InProgress),
                ..Default::default()
            })
            .unwrap_or_default();

        for task in &member_tasks {
            if let Err(e) = state.db.update_task_status(&task.id, TaskStatus::InReview) {
                tracing::error!(task_id = %task.id, error = %e, "failed to transition task to in_review");
                continue;
            }
            let effects = state
                .rules
                .on_status_change(state.db.as_ref(), &task.id, TaskStatus::InReview)
                .unwrap_or_default();
            for effect in &effects {
                tracing::info!(?effect, "rule engine effect (member stop)");
            }

            if !effects.is_empty() {
                let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
            }

            // Enqueue review instruction to leader
            let review_msg = format!(
                "[review] task={} member={} message: {}",
                task.id,
                member_id,
                payload
                    .get("last_assistant_message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(work completed)")
            );
            if let Err(e) = state.db.enqueue_message(leader_id, &review_msg) {
                tracing::error!(error = %e, "failed to enqueue review notification for leader");
            }
        }

        if member_tasks.is_empty() {
            // No tasks to transition; just send a stop event
            let notification = format!("[event] member={member_id} type=stop");
            if let Err(e) = state.db.enqueue_message(leader_id, &notification) {
                tracing::error!(error = %e, "failed to enqueue stop notification for leader");
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
