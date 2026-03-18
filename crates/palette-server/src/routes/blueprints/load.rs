use crate::AppState;
use crate::api_types::{Blueprint, JobResponse};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use palette_domain::job::{JobId, JobStatus, JobType};
use palette_domain::rule::validate_transition;
use palette_domain::server::ServerEvent;
use std::sync::Arc;

pub async fn handle_load_blueprint(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<String>,
) -> Result<(StatusCode, Json<Vec<JobResponse>>), (StatusCode, String)> {
    let stored = state
        .db
        .get_blueprint(&task_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("blueprint not found: {task_id}"),
            )
        })?;

    let blueprint = Blueprint::parse(&stored.yaml).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("stored YAML is invalid: {e}"),
        )
    })?;

    let requests = blueprint.into_requests();
    let mut created_jobs = Vec::new();

    // Create all jobs as draft
    for req in &requests {
        let job = state
            .db
            .create_job(req)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        tracing::info!(job_id = %job.id, task_id = %task_id, "created job from blueprint");
        created_jobs.push(job);
    }

    // Transition craft jobs to ready, triggering auto-assign via rule engine
    let craft_ids: Vec<JobId> = created_jobs
        .iter()
        .filter(|t| t.job_type == JobType::Craft)
        .map(|t| t.id.clone())
        .collect();

    for craft_id in &craft_ids {
        validate_transition(JobType::Craft, JobStatus::Draft, JobStatus::Ready)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        state
            .db
            .update_job_status(craft_id, JobStatus::Ready)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let effects = state
            .rules
            .on_status_change(craft_id, JobStatus::Ready)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        for effect in &effects {
            tracing::info!(?effect, "rule engine effect (blueprint load)");
        }

        if !effects.is_empty() {
            let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
        }
    }

    // Re-fetch all jobs to return updated statuses
    let final_jobs: Vec<JobResponse> = created_jobs
        .iter()
        .filter_map(|t| state.db.get_job(&t.id).ok().flatten())
        .map(JobResponse::from)
        .collect();

    Ok((StatusCode::CREATED, Json(final_jobs)))
}
