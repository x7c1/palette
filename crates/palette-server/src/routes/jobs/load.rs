use crate::AppState;
use crate::api_types::{JobFile, JobResponse};
use axum::{Json, extract::State, http::StatusCode};
use palette_domain::job::{JobId, JobStatus, JobType};
use palette_domain::rule::validate_transition;
use palette_domain::server::ServerEvent;
use std::sync::Arc;

pub async fn handle_load_jobs(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Result<(StatusCode, Json<Vec<JobResponse>>), (StatusCode, String)> {
    let job_file = JobFile::parse(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid YAML: {e}")))?;

    let requests = job_file.into_requests();
    let mut created_jobs = Vec::new();

    // Create all jobs as draft
    for req in &requests {
        let job = state
            .db
            .create_job(req)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        tracing::info!(job_id = %job.id, "created job from YAML");
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
            tracing::info!(?effect, "rule engine effect (job load)");
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
