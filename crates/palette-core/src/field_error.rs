/// A field-level validation error.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct FieldError {
    /// JSON field name (user-facing, not internal field name).
    pub field: String,
    /// Machine-readable reason code in `{namespace}/{value}` format.
    pub reason: String,
}
