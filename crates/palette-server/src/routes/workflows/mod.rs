mod task_activation;

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

use crate::Error;
use crate::api_types::{ErrorCode, FieldHint};

/// Convert a `BlueprintReader::read_blueprint` error into a server error.
fn blueprint_read_error_to_server_error(e: Box<dyn std::error::Error + Send + Sync>) -> Error {
    Error::BadRequest {
        code: ErrorCode::BlueprintInvalid,
        field_hints: vec![FieldHint {
            field: "blueprint_path".into(),
            reason: format!("{e}"),
        }],
    }
}
