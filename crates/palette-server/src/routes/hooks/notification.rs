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
    let worker_id_str = query.worker_id.as_deref().unwrap_or("unknown");
    let worker_id = match WorkerId::parse(worker_id_str) {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!(worker_id = worker_id_str, error = ?e, "invalid worker_id in notification hook");
            return StatusCode::BAD_REQUEST;
        }
    };
    tracing::info!(worker_id = worker_id_str, payload = %payload, "received notification hook");

    let record = EventRecord {
        timestamp: now(),
        event_type: "notification".to_string(),
        payload: serde_json::json!({
            "worker_id": worker_id_str,
            "original": payload,
        }),
    };
    state.event_log.lock().await.push(record);

    super::save_session_id(state.interactor.data_store.as_ref(), &worker_id, &payload);

    let notification_payload: NotificationPayload =
        serde_json::from_value(payload).unwrap_or(NotificationPayload {
            transcript_path: None,
        });

    // Update worker status to WaitingPermission and resolve context
    let worker_context = {
        let worker = match state.interactor.data_store.find_worker(&worker_id) {
            Ok(Some(w)) => w,
            _ => {
                let _ = state.event_tx.send(ServerEvent::NotifyDeliveryLoop);
                return StatusCode::OK;
            }
        };
        if let Err(e) = state
            .interactor
            .data_store
            .update_worker_status(&worker_id, WorkerStatus::WaitingPermission)
        {
            tracing::error!(error = %e, "failed to update worker status");
        }
        WorkerContext {
            supervisor_id: worker.supervisor_id.clone(),
            terminal_target: worker.terminal_target.clone(),
            container_id: worker.container_id.clone(),
        }
    };

    // Extract the pending tool call from the member's JSONL transcript
    let pending_tool = match notification_payload.transcript_path {
        Some(ref path) if !path.is_empty() => extract_pending_tool(
            state.interactor.container.as_ref(),
            &worker_context.container_id,
            path,
        ),
        _ => None,
    };

    // Capture the worker's pane content (last 10 non-empty lines, joined as single line)
    let pane_content = state
        .interactor
        .terminal
        .capture_pane(&worker_context.terminal_target)
        .ok()
        .map(|content| {
            let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
            let start = lines.len().saturating_sub(10);
            lines[start..].join(" | ")
        });

    // Build notification message
    let mut notification = format!("[event] member={worker_id} type=permission_prompt");
    if let Some(ref tool) = pending_tool {
        notification.push_str(&format!(" tool={} input={}", tool.name, tool.input));
    }
    if let Some(ref pane) = pane_content {
        notification.push_str(&format!(" pane=[{pane}]"));
    }

    if let Some(ref supervisor_id) = worker_context.supervisor_id
        && let Err(e) = state
            .interactor
            .data_store
            .enqueue_message(supervisor_id, &notification)
    {
        tracing::error!(error = %e, "failed to enqueue notification for supervisor");
    }

    let _ = state.event_tx.send(ServerEvent::NotifyDeliveryLoop);

    StatusCode::OK
}

struct WorkerContext {
    supervisor_id: Option<WorkerId>,
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

/// Read the worker's transcript and extract the last tool_use entry.
fn extract_pending_tool(
    container: &dyn palette_usecase::ContainerRuntime,
    container_id: &palette_domain::worker::ContainerId,
    transcript_path: &str,
) -> Option<PendingTool> {
    let content = container
        .read_container_file(container_id, transcript_path, 5)
        .ok()?;

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
