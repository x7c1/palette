use crate::{AppState, Error, ValidJson};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palette_domain::job::{JobDetail, PerspectiveName, PullRequest, ReviewTarget};
use palette_domain::server::ServerEvent;
use palette_domain::task::{TaskId, TaskKey, TaskTree, TaskTreeNode};
use palette_domain::workflow::WorkflowId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    let pr =
        PullRequest::parse(&req.owner, &req.repo, req.number).map_err(|e| Error::BadRequest {
            code: crate::api_types::ErrorCode::InputValidationFailed,
            errors: vec![crate::api_types::InputError {
                location: crate::api_types::Location::Body,
                hint: "owner/repo/number".into(),
                reason: palette_core::ReasonKey::reason_key(&e),
            }],
        })?;

    if req.reviewers.is_empty() {
        return Err(Error::BadRequest {
            code: crate::api_types::ErrorCode::InputValidationFailed,
            errors: vec![crate::api_types::InputError {
                location: crate::api_types::Location::Body,
                hint: "reviewers".into(),
                reason: "at least one reviewer is required".into(),
            }],
        });
    }

    // Parse perspective names
    let mut perspectives = Vec::new();
    for (i, r) in req.reviewers.iter().enumerate() {
        let perspective = r
            .perspective
            .as_deref()
            .map(PerspectiveName::parse)
            .transpose()
            .map_err(|e| Error::BadRequest {
                code: crate::api_types::ErrorCode::InputValidationFailed,
                errors: vec![crate::api_types::InputError {
                    location: crate::api_types::Location::Body,
                    hint: format!("reviewers[{i}].perspective"),
                    reason: palette_core::ReasonKey::reason_key(&e),
                }],
            })?;
        perspectives.push(perspective);
    }

    let workflow_id = WorkflowId::generate();
    let tree = build_pr_review_tree(&workflow_id, &pr, &perspectives)?;
    let task_count = super::start::register_tasks(&state, &workflow_id, &tree, None)?;

    let _ = state.event_tx.send(ServerEvent::ActivateWorkflow {
        workflow_id: workflow_id.clone(),
    });

    tracing::info!(
        workflow_id = %workflow_id,
        pr = %pr,
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

/// Build a TaskTree for a standalone PR review workflow.
///
/// ```text
/// root (composite, no job)
/// └── review-integrate (ReviewIntegrate, target: PullRequest)
///     ├── review-1 (Review, target: PullRequest, perspective?)
///     ├── review-2 (Review, target: PullRequest, perspective?)
///     └── ...
/// ```
fn build_pr_review_tree(
    workflow_id: &WorkflowId,
    pr: &PullRequest,
    perspectives: &[Option<PerspectiveName>],
) -> crate::Result<TaskTree> {
    let root_key = TaskKey::parse("pr-review").map_err(|e| Error::internal(format!("{e:?}")))?;
    let ri_key =
        TaskKey::parse("review-integrate").map_err(|e| Error::internal(format!("{e:?}")))?;

    let root_id = TaskId::root(workflow_id, &root_key);
    let ri_id = root_id.child(&ri_key);

    let mut nodes = HashMap::new();
    let mut reviewer_ids = Vec::new();

    // Create reviewer leaf nodes
    for (i, perspective) in perspectives.iter().enumerate() {
        let key_str = format!("review-{}", i + 1);
        let key = TaskKey::parse(&key_str).map_err(|e| Error::internal(format!("{e:?}")))?;
        let reviewer_id = ri_id.child(&key);
        reviewer_ids.push(reviewer_id.clone());

        nodes.insert(
            reviewer_id.clone(),
            TaskTreeNode {
                id: reviewer_id,
                parent_id: Some(ri_id.clone()),
                key,
                plan_path: None,
                priority: None,
                children: vec![],
                depends_on: vec![],
                job_detail: Some(JobDetail::Review {
                    perspective: perspective.clone(),
                    target: ReviewTarget::PullRequest(pr.clone()),
                }),
            },
        );
    }

    // Create review-integrate composite node
    nodes.insert(
        ri_id.clone(),
        TaskTreeNode {
            id: ri_id.clone(),
            parent_id: Some(root_id.clone()),
            key: ri_key,
            plan_path: None,
            priority: None,
            children: reviewer_ids,
            depends_on: vec![],
            job_detail: Some(JobDetail::ReviewIntegrate {
                target: ReviewTarget::PullRequest(pr.clone()),
            }),
        },
    );

    // Create root composite node
    nodes.insert(
        root_id.clone(),
        TaskTreeNode {
            id: root_id.clone(),
            parent_id: None,
            key: root_key,
            plan_path: None,
            priority: None,
            children: vec![ri_id],
            depends_on: vec![],
            job_detail: None,
        },
    );

    Ok(TaskTree::new(root_id, nodes))
}
