/// Where in the HTTP request the invalid input originated.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Location {
    Path,
    Query,
    Body,
}

/// An input validation error.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct InputError {
    /// Where in the request the error originated.
    pub location: Location,
    /// Dot-separated path to the problematic input (e.g. "title", "comments[0].file").
    pub hint: String,
    /// Machine-readable reason code in `{namespace}/{value}` format.
    pub reason: String,
}

impl InputError {
    pub fn path(hint: impl Into<String>, reason: impl crate::ReasonKey) -> Self {
        Self {
            location: Location::Path,
            hint: hint.into(),
            reason: reason.reason_key(),
        }
    }

    pub fn query(hint: impl Into<String>, reason: impl crate::ReasonKey) -> Self {
        Self {
            location: Location::Query,
            hint: hint.into(),
            reason: reason.reason_key(),
        }
    }

    pub fn body(hint: impl Into<String>, reason: impl crate::ReasonKey) -> Self {
        Self {
            location: Location::Body,
            hint: hint.into(),
            reason: reason.reason_key(),
        }
    }
}
