mod blueprints;
mod hooks;
mod jobs;
mod reviews;
mod send;
mod workflows;

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
        // Blueprint API
        .route(
            "/blueprints/submit",
            post(blueprints::handle_submit_blueprint),
        )
        .route("/blueprints", get(blueprints::handle_list_blueprints))
        .route(
            "/blueprints/{task_id}/load",
            post(blueprints::handle_load_blueprint),
        )
        .route(
            "/blueprints/{task_id}",
            get(blueprints::handle_get_blueprint),
        )
        // Workflow API
        .route("/workflows/start", post(workflows::handle_start_workflow))
        // Job API
        .route("/jobs/create", post(jobs::handle_create_job))
        .route("/jobs/update", post(jobs::handle_update_job))
        .route("/jobs", get(jobs::handle_list_jobs))
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
