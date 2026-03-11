use crate::AppState;
use crate::api_types::{JobResponse, UpdateJobRequest};
use axum::{Json, extract::State, http::StatusCode};
use palette_domain::rule::validate_transition;
use palette_domain::server::ServerEvent;
use std::sync::Arc;

pub async fn handle_update_job(
    State(state): State<Arc<AppState>>,
    Json(api_req): Json<UpdateJobRequest>,
) -> Result<Json<JobResponse>, (StatusCode, String)> {
    let req: palette_domain::job::UpdateJobRequest = api_req.into();
    let current = state
        .db
        .get_job(&req.id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (StatusCode::NOT_FOUND, {
                let id = &req.id;
                format!("job not found: {id}")
            })
        })?;

    validate_transition(current.job_type, current.status, req.status)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let job = state
        .db
        .update_job_status(&req.id, req.status)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Apply rule engine side effects
    let effects = state
        .rules
        .on_status_change(&req.id, req.status)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for effect in &effects {
        tracing::info!(?effect, "rule engine effect");
    }

    if !effects.is_empty() {
        let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    }

    Ok(Json(JobResponse::from(job)))
}
