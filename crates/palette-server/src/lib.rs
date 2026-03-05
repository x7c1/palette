mod routes;

use axum::Router;
use palette_tmux::TmuxManagerImpl;
use std::sync::Arc;

pub struct AppState {
    pub tmux: TmuxManagerImpl,
    pub target: String,
    pub event_log: tokio::sync::Mutex<Vec<EventRecord>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EventRecord {
    pub timestamp: String,
    pub event_type: String,
    pub payload: serde_json::Value,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    routes::create_router(state)
}
