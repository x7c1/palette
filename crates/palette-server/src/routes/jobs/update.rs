use crate::AppState;
use crate::api_types::{JobResponse, UpdateJobRequest};
use axum::{Json, extract::State, http::StatusCode};
use palette_domain::job::JobId;
use palette_domain::rule::validate_transition;
use palette_domain::server::ServerEvent;
use std::sync::Arc;

pub async fn handle_update_job(
    State(state): State<Arc<AppState>>,
    Json(api_req): Json<UpdateJobRequest>,
) -> Result<Json<JobResponse>, (StatusCode, String)> {
    let job_id = JobId::new(&api_req.id);
    let current = state
        .db
        .get_job(&job_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("job not found: {job_id}")))?;

    // Convert API status to domain status using the job's type
    let new_status = api_req.status.to_domain(current.job_type);

    validate_transition(current.status, new_status)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let job = state
        .db
        .update_job_status(&job_id, new_status)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Apply rule engine side effects
    let effects = state
        .rules
        .on_status_change(&job_id, new_status)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for effect in &effects {
        tracing::info!(?effect, "rule engine effect");
    }

    if !effects.is_empty() {
        let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    }

    Ok(Json(JobResponse::from(job)))
}
