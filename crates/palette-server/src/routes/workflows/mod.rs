mod list;
pub use list::handle_list_workflows;

mod resume;
pub use resume::handle_resume_workflow;

mod start;
pub use start::handle_start_workflow;

mod suspend;
pub use suspend::handle_suspend_workflow;

mod apply_blueprint;
pub use apply_blueprint::handle_apply_blueprint;

mod start_pr_review;
pub use start_pr_review::handle_start_pr_review;

use crate::Error;
use crate::api_types::{ErrorCode, InputError, Location};
use palette_usecase::ReadBlueprintError;

/// Map a `ReadBlueprintError` to a 4xx/5xx server error for endpoints that
/// conceptually "use" the blueprint (start, apply-blueprint). File-level
/// NotFound collapses into a single `blueprint/not_found` InputError so
/// clients get a machine-readable reason in the existing `BlueprintInvalid`
/// envelope instead of a free-form message.
fn blueprint_read_error_to_server_error(e: ReadBlueprintError) -> Error {
    match e {
        ReadBlueprintError::NotFound { .. } => Error::BadRequest {
            code: ErrorCode::BlueprintInvalid,
            errors: vec![InputError {
                location: Location::Body,
                hint: "blueprint_path".into(),
                reason: "blueprint/not_found".into(),
            }],
        },
        ReadBlueprintError::Invalid(errors) => Error::BadRequest {
            code: ErrorCode::BlueprintInvalid,
            errors,
        },
        ReadBlueprintError::Internal(cause) => Error::internal(format!("{cause}")),
    }
}
