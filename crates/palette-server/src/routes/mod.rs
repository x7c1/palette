mod hooks;
mod jobs;
mod reviews;
mod send;
mod send_permission;
mod shutdown;
mod workers;
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
        // Health
        .route("/health", get(handle_health))
        // Shutdown
        .route("/shutdown", post(shutdown::handle_shutdown))
        // Hooks
        .route("/hooks/stop", post(hooks::handle_stop))
        .route("/hooks/notification", post(hooks::handle_notification))
        .route("/hooks/session-start", post(hooks::handle_session_start))
        // Send
        .route("/send", post(send::handle_send))
        .route(
            "/send/permission",
            post(send_permission::handle_send_permission),
        )
        // Events
        .route("/events", get(handle_events))
        // Workflow API
        .route("/workflows/start", post(workflows::handle_start_workflow))
        .route(
            "/workflows/start-pr-review",
            post(workflows::handle_start_pr_review),
        )
        .route(
            "/workflows/{id}/suspend",
            post(workflows::handle_suspend_workflow),
        )
        .route(
            "/workflows/{id}/resume",
            post(workflows::handle_resume_workflow),
        )
        .route(
            "/workflows/{id}/apply-blueprint",
            post(workflows::handle_apply_blueprint),
        )
        .route("/workflows", get(workflows::handle_list_workflows))
        // Job API
        .route("/jobs/create", post(jobs::handle_create_job))
        .route("/jobs/update", post(jobs::handle_update_job))
        .route("/jobs", get(jobs::handle_list_jobs))
        // Review API
        // Worker API
        .route("/workers", get(workers::handle_list_workers))
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

async fn handle_health() -> &'static str {
    "ok"
}

fn now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string()
}
