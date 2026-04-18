use serde::Serialize;

/// Machine-readable error code for 400 Bad Request.
/// Managed as an enum so that the compiler enforces exhaustiveness
/// and prevents typos.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InputValidationFailed,
    InvalidStateTransition,
    NotReviewJob,
    ChildReviewersIncomplete,
    ReviewArtifactMissing,
    JobAlreadyDone,
    BlueprintInvalid,
}

/// Resource kind for 404 Not Found.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Job,
    Workflow,
    Worker,
    ReviewSubmission,
    Blueprint,
}

pub use palette_core::{InputError, Location};
