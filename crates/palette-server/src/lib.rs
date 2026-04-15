pub mod api_types;

mod error;
pub use error::{Error, Result};

mod extract;
pub use extract::ValidJson;

pub mod permission_timeout;
mod routes;

use axum::Router;
use palette_domain::server::ServerEvent;
use palette_usecase::Interactor;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct AppState {
    pub interactor: Arc<Interactor>,
    pub max_review_rounds: u32,
    pub data_dir: PathBuf,
    pub event_log: tokio::sync::Mutex<Vec<EventRecord>>,
    pub pending_permission_events: tokio::sync::Mutex<HashMap<String, PendingPermission>>,
    pub event_tx: tokio::sync::mpsc::UnboundedSender<ServerEvent>,
    pub shutdown_notify: Arc<tokio::sync::Notify>,
}

#[derive(Debug, Clone)]
pub struct PendingPermission {
    pub event_id: String,
    pub created_at: std::time::Instant,
    pub supervisor_id: Option<palette_domain::worker::WorkerId>,
    pub notification: String,
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
