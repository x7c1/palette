use crate::{AppState, EventRecord};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use palette_domain::server::ServerEvent;
use palette_domain::worker::{WorkerId, WorkerStatus};
use std::sync::Arc;

use super::HookQuery;
use crate::routes::now;

/// Payload sent by Claude Code's notification hook.
#[derive(serde::Deserialize)]
struct NotificationPayload {
    transcript_path: Option<String>,
}

pub async fn handle_notification(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HookQuery>,
    Json(payload): Json<serde_json::Value>,
) -> StatusCode {
    let member_id_str = query.member_id.as_deref().unwrap_or("unknown");
    let member_id = WorkerId::new(member_id_str);
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

    let notification_payload: NotificationPayload =
        serde_json::from_value(payload).unwrap_or(NotificationPayload {
            transcript_path: None,
        });

    // Update member status to WaitingPermission and resolve context
    let member_context = {
        let member = match state.db.find_worker(&member_id) {
            Ok(Some(m)) => m,
            _ => {
                let _ = state.event_tx.send(ServerEvent::NotifyDeliveryLoop);
                return StatusCode::OK;
            }
        };
        if let Err(e) = state
            .db
            .update_worker_status(&member_id, WorkerStatus::WaitingPermission)
        {
            tracing::error!(error = %e, "failed to update worker status");
        }
        MemberContext {
            supervisor_id: member.supervisor_id.clone(),
            terminal_target: member.terminal_target.clone(),
            container_id: member.container_id.clone(),
        }
    };

    // Extract the pending tool call from the member's JSONL transcript
    let pending_tool = match notification_payload.transcript_path {
        Some(ref path) if !path.is_empty() => {
            extract_pending_tool(&member_context.container_id, path)
        }
        _ => None,
    };

    // Capture the member's pane content (last 10 non-empty lines, joined as single line)
    let pane_content = state
        .tmux
        .capture_pane(&member_context.terminal_target)
        .ok()
        .map(|content| {
            let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
            let start = lines.len().saturating_sub(10);
            lines[start..].join(" | ")
        });

    // Build notification message
    let mut notification = format!("[event] member={member_id} type=permission_prompt");
    if let Some(ref tool) = pending_tool {
        notification.push_str(&format!(" tool={} input={}", tool.name, tool.input));
    }
    if let Some(ref pane) = pane_content {
        notification.push_str(&format!(" pane=[{pane}]"));
    }

    if let Err(e) = state
        .db
        .enqueue_message(&member_context.supervisor_id, &notification)
    {
        tracing::error!(error = %e, "failed to enqueue notification for supervisor");
    }

    let _ = state.event_tx.send(ServerEvent::NotifyDeliveryLoop);

    StatusCode::OK
}

struct MemberContext {
    supervisor_id: WorkerId,
    terminal_target: palette_domain::terminal::TerminalTarget,
    container_id: palette_domain::worker::ContainerId,
}

struct PendingTool {
    name: String,
    input: String,
}

/// A single entry in a Claude Code JSONL transcript.
#[derive(serde::Deserialize)]
struct TranscriptEntry {
    #[serde(rename = "type")]
    entry_type: String,
    message: Option<TranscriptMessage>,
}

#[derive(serde::Deserialize)]
struct TranscriptMessage {
    content: Vec<ContentBlock>,
}

#[derive(serde::Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "tool_use")]
    ToolUse {
        name: String,
        input: serde_json::Value,
    },
    #[serde(other)]
    Other,
}

/// Read the member's transcript and extract the last tool_use entry.
fn extract_pending_tool(
    container_id: &palette_domain::worker::ContainerId,
    transcript_path: &str,
) -> Option<PendingTool> {
    let content = palette_docker::read_container_file(container_id, transcript_path, 5).ok()?;

    for line in content.lines().rev() {
        let entry: TranscriptEntry = serde_json::from_str(line).ok()?;
        if entry.entry_type != "assistant" {
            continue;
        }
        let contents = entry.message?.content;
        for block in contents.iter().rev() {
            if let ContentBlock::ToolUse { name, input } = block {
                let input_str = input.to_string();
                let input_str = if input_str.chars().count() > 200 {
                    let truncated: String = input_str.chars().take(200).collect();
                    format!("{truncated}...")
                } else {
                    input_str
                };
                return Some(PendingTool {
                    name: name.clone(),
                    input: input_str,
                });
            }
        }
    }
    None
}
