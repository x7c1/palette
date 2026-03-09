use crate::{AppState, EventRecord};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use palette_domain::{AgentId, AgentStatus, ServerEvent};
use std::sync::Arc;

use super::HookQuery;
use crate::routes::now;

pub async fn handle_notification(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HookQuery>,
    Json(payload): Json<serde_json::Value>,
) -> StatusCode {
    let member_id_str = query.member_id.as_deref().unwrap_or("unknown");
    let member_id = AgentId::new(member_id_str);
    tracing::info!(member_id = member_id_str, payload = %payload, "received notification hook");

    let record = EventRecord {
        timestamp: now(),
        event_type: "notification".to_string(),
        payload: serde_json::json!({
            "member_id": member_id_str,
            "original": payload,
        }),
    };
    state.event_log.lock().await.push(record);

    // Update member status to WaitingPermission and resolve leader ID
    let leader_id = {
        let mut infra = state.infra.lock().await;
        if let Some(member) = infra.find_member_mut(&member_id) {
            member.status = AgentStatus::WaitingPermission;
            let leader_id = member.leader_id.clone();
            infra.touch();
            Some(leader_id)
        } else {
            None
        }
    };

    // Enqueue event notification to leader
    if let Some(ref leader_id) = leader_id {
        let notification = format!(
            "[event] member={} type=permission_prompt payload={}",
            member_id,
            serde_json::to_string(&payload).unwrap_or_default()
        );
        if let Err(e) = state.db.enqueue_message(leader_id, &notification) {
            tracing::error!(error = %e, "failed to enqueue notification for leader");
        }
    }

    // Fire-and-forget: notify delivery loop
    let _ = state.event_tx.send(ServerEvent::NotifyDeliveryLoop);

    StatusCode::OK
}
