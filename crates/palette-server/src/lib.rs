pub mod api_types;

mod routes;

use axum::Router;
use palette_db::Database;
use palette_domain::rule::RuleEngine;
use palette_domain::server::{PersistentState, ServerEvent};
use palette_tmux::TmuxManager;
use std::sync::Arc;

pub struct AppState {
    pub tmux: Arc<TmuxManager>,
    pub db: Arc<Database>,
    pub rules: RuleEngine<Arc<Database>>,
    pub infra: Arc<tokio::sync::Mutex<PersistentState>>,
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
