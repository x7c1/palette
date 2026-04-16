use crate::{AppState, Error, ValidJson};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palette_domain::server::ServerEvent;
use palette_domain::workflow::WorkflowId;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct StartPrReviewRequest {
    pub owner: String,
    pub repo: String,
    pub number: u64,
    pub reviewers: Vec<ReviewerSpec>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewerSpec {
    pub perspective: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StartPrReviewResponse {
    pub workflow_id: String,
    pub task_count: usize,
}

pub async fn handle_start_pr_review(
    State(state): State<Arc<AppState>>,
    ValidJson(req): ValidJson<StartPrReviewRequest>,
) -> crate::Result<Response> {
    if req.reviewers.is_empty() {
        return Err(Error::BadRequest {
            code: crate::api_types::ErrorCode::InputValidationFailed,
            errors: vec![crate::api_types::InputError {
                location: crate::api_types::Location::Body,
                hint: "reviewers".into(),
                reason: "reviewers/empty".into(),
            }],
        });
    }

    let workflow_id = WorkflowId::generate();

    // Generate Blueprint YAML from request
    let yaml = generate_blueprint_yaml(&req);
    let blueprint_dir = state.data_dir.join("blueprints");
    std::fs::create_dir_all(&blueprint_dir).map_err(Error::internal)?;
    let blueprint_path = blueprint_dir.join(format!("{}.yaml", workflow_id));
    std::fs::write(&blueprint_path, &yaml).map_err(Error::internal)?;
    let blueprint_path_str = blueprint_path.to_string_lossy().to_string();

    tracing::info!(
        workflow_id = %workflow_id,
        path = %blueprint_path_str,
        "generated PR review blueprint"
    );

    // Use existing Blueprint read path
    let tree = state
        .interactor
        .blueprint
        .read_blueprint(Path::new(&blueprint_path_str), &workflow_id)
        .map_err(super::blueprint_read_error_to_server_error)?;

    let task_count =
        super::start::register_tasks(&state, &workflow_id, &tree, &blueprint_path_str)?;

    let _ = state.event_tx.send(ServerEvent::ActivateWorkflow {
        workflow_id: workflow_id.clone(),
    });

    tracing::info!(
        workflow_id = %workflow_id,
        owner = %req.owner,
        repo = %req.repo,
        number = req.number,
        task_count,
        "started PR review workflow"
    );

    Ok((
        StatusCode::CREATED,
        Json(StartPrReviewResponse {
            workflow_id: workflow_id.to_string(),
            task_count,
        }),
    )
        .into_response())
}

/// Generate a Blueprint YAML string for a PR review workflow.
fn generate_blueprint_yaml(req: &StartPrReviewRequest) -> String {
    let mut yaml = String::new();
    yaml.push_str("task:\n");
    yaml.push_str("  key: pr-review\n");
    yaml.push_str("  children:\n");
    yaml.push_str("    - key: review-integrate\n");
    yaml.push_str("      type: review_integrate\n");
    yaml.push_str("      pull_request:\n");
    yaml.push_str(&format!("        owner: \"{}\"\n", req.owner));
    yaml.push_str(&format!("        repo: \"{}\"\n", req.repo));
    yaml.push_str(&format!("        number: {}\n", req.number));
    yaml.push_str("      children:\n");

    for (i, reviewer) in req.reviewers.iter().enumerate() {
        yaml.push_str(&format!("        - key: review-{}\n", i + 1));
        yaml.push_str("          type: review\n");
        if let Some(ref perspective) = reviewer.perspective {
            yaml.push_str(&format!("          perspective: \"{perspective}\"\n"));
        }
        yaml.push_str("          pull_request:\n");
        yaml.push_str(&format!("            owner: \"{}\"\n", req.owner));
        yaml.push_str(&format!("            repo: \"{}\"\n", req.repo));
        yaml.push_str(&format!("            number: {}\n", req.number));
    }

    yaml
}
