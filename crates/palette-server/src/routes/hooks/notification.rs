use crate::{AppState, EventRecord};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use palette_domain::agent::{AgentId, AgentStatus};
use palette_domain::server::ServerEvent;
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

    // Update member status to WaitingPermission and resolve context
    let member_context = {
        let mut infra = state.infra.lock().await;
        if let Some(member) = infra.find_member_mut(&member_id) {
            member.status = AgentStatus::WaitingPermission;
            let ctx = MemberContext {
                leader_id: member.leader_id.clone(),
                terminal_target: member.terminal_target.clone(),
                container_id: member.container_id.clone(),
            };
            infra.touch();
            Some(ctx)
        } else {
            None
        }
    };

    let Some(ctx) = member_context else {
        let _ = state.event_tx.send(ServerEvent::NotifyDeliveryLoop);
        return StatusCode::OK;
    };

    // Extract the pending tool call from the member's JSONL transcript
    let transcript_path = payload
        .get("transcript_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let pending_tool = if !transcript_path.is_empty() {
        extract_pending_tool(&ctx.container_id, transcript_path)
    } else {
        None
    };

    // Capture the member's pane content (last 10 non-empty lines, joined as single line)
    let pane_content = state
        .tmux
        .capture_pane(&ctx.terminal_target)
        .ok()
        .map(|content| {
            let lines: Vec<&str> = content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .collect();
            let start = lines.len().saturating_sub(10);
            lines[start..].join(" | ")
        });

    // Build notification message
    let mut notification = format!(
        "[event] member={member_id} type=permission_prompt"
    );
    if let Some(ref tool) = pending_tool {
        notification.push_str(&format!(" tool={} input={}", tool.name, tool.input));
    }
    if let Some(ref pane) = pane_content {
        notification.push_str(&format!(" pane=[{pane}]"));
    }

    if let Err(e) = state.db.enqueue_message(&ctx.leader_id, &notification) {
        tracing::error!(error = %e, "failed to enqueue notification for leader");
    }

    let _ = state.event_tx.send(ServerEvent::NotifyDeliveryLoop);

    StatusCode::OK
}

struct MemberContext {
    leader_id: AgentId,
    terminal_target: palette_domain::terminal::TerminalTarget,
    container_id: palette_domain::agent::ContainerId,
}

struct PendingTool {
    name: String,
    input: String,
}

/// Read the member's transcript and extract the last tool_use entry.
fn extract_pending_tool(
    container_id: &palette_domain::agent::ContainerId,
    transcript_path: &str,
) -> Option<PendingTool> {
    let content =
        palette_docker::read_container_file(container_id, transcript_path, 5).ok()?;

    for line in content.lines().rev() {
        let entry: serde_json::Value = serde_json::from_str(line).ok()?;
        if entry.get("type")?.as_str()? != "assistant" {
            continue;
        }
        let contents = entry.get("message")?.get("content")?.as_array()?;
        for item in contents.iter().rev() {
            if item.get("type")?.as_str()? == "tool_use" {
                let name = item.get("name")?.as_str()?.to_string();
                let input = item.get("input")?.to_string();
                let input = if input.len() > 200 {
                    format!("{}...", &input[..200])
                } else {
                    input
                };
                return Some(PendingTool { name, input });
            }
        }
    }
    None
}
