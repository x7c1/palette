mod hooks;
mod reviews;
mod send;
mod tasks;

use crate::{AppState, EventRecord};
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use std::sync::Arc;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Hooks
        .route("/hooks/stop", post(hooks::handle_stop))
        .route("/hooks/notification", post(hooks::handle_notification))
        // Send
        .route("/send", post(send::handle_send))
        // Events
        .route("/events", get(handle_events))
        // Task API
        .route("/tasks/create", post(tasks::handle_create_task))
        .route("/tasks/update", post(tasks::handle_update_task))
        .route("/tasks/load", post(tasks::handle_load_tasks))
        .route("/tasks", get(tasks::handle_list_tasks))
        // Review API
        .route("/reviews/{id}/submit", post(reviews::handle_submit_review))
        .route(
            "/reviews/{id}/submissions",
            get(reviews::handle_get_submissions),
        )
        .with_state(state)
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
