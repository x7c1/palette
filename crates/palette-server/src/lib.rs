pub mod api_types;

mod error;
pub use error::{Error, Result};

mod extract;
pub use extract::ValidJson;

mod routes;

use axum::Router;
use palette_domain::server::ServerEvent;
use palette_usecase::Interactor;
use std::sync::Arc;

pub struct AppState {
    pub interactor: Arc<Interactor>,
    pub max_review_rounds: u32,
    pub data_dir: std::path::PathBuf,
    pub event_log: tokio::sync::Mutex<Vec<EventRecord>>,
    pub event_tx: tokio::sync::mpsc::UnboundedSender<ServerEvent>,
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
