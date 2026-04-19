use crate::api_types::{ErrorCode, InputError, Location, ResourceKind};
use crate::{AppState, Error, ValidJson};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palette_domain::workflow::WorkflowId;
use palette_usecase::{BlueprintSummary, ReadBlueprintError, validate_blueprint};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct ValidateBlueprintRequest {
    pub blueprint_path: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ValidateBlueprintResponse {
    Valid {
        valid: bool,
        summary: SummaryPayload,
    },
    Invalid {
        valid: bool,
        errors: Vec<InputError>,
    },
}

#[derive(Debug, Serialize)]
pub struct SummaryPayload {
    pub root_task_key: String,
    pub task_count: usize,
    pub craft_count: usize,
    pub review_count: usize,
    pub referenced_plans: Vec<String>,
}

impl From<BlueprintSummary> for SummaryPayload {
    fn from(s: BlueprintSummary) -> Self {
        SummaryPayload {
            root_task_key: s.root_task_key,
            task_count: s.task_count,
            craft_count: s.craft_count,
            review_count: s.review_count,
            referenced_plans: s.referenced_plans,
        }
    }
}

pub async fn handle_validate_blueprint(
    State(state): State<Arc<AppState>>,
    ValidJson(req): ValidJson<ValidateBlueprintRequest>,
) -> crate::Result<Response> {
    let path = Path::new(&req.blueprint_path);
    if !path.is_absolute() {
        return Err(Error::BadRequest {
            code: ErrorCode::InputValidationFailed,
            errors: vec![InputError {
                location: Location::Body,
                hint: "blueprint_path".into(),
                reason: "blueprint_path/not_absolute".into(),
            }],
        });
    }

    // A throwaway WorkflowId lets us reuse the existing read_blueprint path
    // without creating any persisted state. No DB writes, no events.
    let placeholder_id = WorkflowId::generate();

    match validate_blueprint(state.interactor.blueprint.as_ref(), path, &placeholder_id) {
        Ok(tree) => {
            let summary: SummaryPayload = BlueprintSummary::from_tree(&tree).into();
            let body = ValidateBlueprintResponse::Valid {
                valid: true,
                summary,
            };
            Ok((StatusCode::OK, Json(body)).into_response())
        }
        Err(ReadBlueprintError::NotFound { .. }) => Err(Error::NotFound {
            resource: ResourceKind::Blueprint,
            id: req.blueprint_path,
        }),
        Err(ReadBlueprintError::Invalid(errors)) => {
            let body = ValidateBlueprintResponse::Invalid {
                valid: false,
                errors,
            };
            Ok((StatusCode::OK, Json(body)).into_response())
        }
        Err(ReadBlueprintError::Internal(cause)) => Err(Error::internal(format!("{cause}"))),
    }
}
