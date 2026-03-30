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
}

/// A hint indicating which user-facing field caused the error.
#[derive(Debug, Clone, Serialize)]
pub struct FieldHint {
    /// JSON field name (user-facing, not internal field name).
    pub field: String,
    /// Machine-readable reason code derived from the source error type.
    pub reason: String,
}
