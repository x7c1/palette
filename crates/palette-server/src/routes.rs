use crate::{AppState, EventRecord};
use palette_tmux::TmuxManager as _;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use std::sync::Arc;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/hooks/stop", post(handle_stop))
        .route("/hooks/notification", post(handle_notification))
        .route("/send", post(handle_send))
        .route("/events", get(handle_events))
        .with_state(state)
}

async fn handle_stop(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> StatusCode {
    tracing::info!(payload = %payload, "received stop hook");
    let record = EventRecord {
        timestamp: now(),
        event_type: "stop".to_string(),
        payload,
    };
    state.event_log.lock().await.push(record);
    StatusCode::OK
}

async fn handle_notification(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> StatusCode {
    tracing::info!(payload = %payload, "received notification hook");
    let record = EventRecord {
        timestamp: now(),
        event_type: "notification".to_string(),
        payload,
    };
    state.event_log.lock().await.push(record);
    StatusCode::OK
}

#[derive(serde::Deserialize)]
struct SendRequest {
    message: String,
}

async fn handle_send(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SendRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    tracing::info!(target = %state.target, message = %req.message, "sending keys via tmux");
    let record = EventRecord {
        timestamp: now(),
        event_type: "send".to_string(),
        payload: serde_json::json!({
            "target": state.target,
            "message": req.message,
        }),
    };
    state.event_log.lock().await.push(record);

    state
        .tmux
        .send_keys(&state.target, &req.message)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::OK)
}

async fn handle_events(State(state): State<Arc<AppState>>) -> Json<Vec<EventRecord>> {
    let events = state.event_log.lock().await;
    Json(events.clone())
}

fn now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string()
}
