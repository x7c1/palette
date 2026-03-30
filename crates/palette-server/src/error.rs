use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::api_types::{ErrorCode, FieldError, ResourceKind};

pub type Result<T> = std::result::Result<T, Error>;

/// Server-level error type.
/// Variants correspond to HTTP status categories, not individual error codes.
#[derive(Debug)]
pub enum Error {
    /// Client input error (400).
    /// `code` identifies the error kind; `errors` indicates which fields are problematic.
    BadRequest {
        code: ErrorCode,
        errors: Vec<FieldError>,
    },
    /// Resource not found (404).
    NotFound { resource: ResourceKind, id: String },
    /// Server-side failure (500). Cause is logged, never exposed in response.
    Internal {
        cause: Box<dyn std::fmt::Debug + Send + Sync>,
    },
}

impl Error {
    pub fn internal(cause: impl std::fmt::Debug + Send + Sync + 'static) -> Self {
        Error::Internal {
            cause: Box::new(cause),
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Error::BadRequest { code, errors } => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "code": code,
                    "errors": errors,
                })),
            )
                .into_response(),
            Error::NotFound { resource, id } => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "code": "not_found",
                    "resource": resource,
                    "id": id,
                })),
            )
                .into_response(),
            Error::Internal { cause } => {
                tracing::error!(?cause, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "code": "internal" })),
                )
                    .into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::response::IntoResponse;

    async fn response_body(resp: Response) -> serde_json::Value {
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn bad_request_returns_400_with_code_and_errors() {
        let error = Error::BadRequest {
            code: ErrorCode::InputValidationFailed,
            errors: vec![FieldError {
                field: "title".into(),
                reason: "title/required".into(),
            }],
        };
        let resp = error.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let body = response_body(resp).await;
        assert_eq!(body["code"], "input_validation_failed");
        assert_eq!(body["errors"][0]["field"], "title");
        assert_eq!(body["errors"][0]["reason"], "title/required");
    }

    #[tokio::test]
    async fn bad_request_empty_errors() {
        let error = Error::BadRequest {
            code: ErrorCode::InvalidStateTransition,
            errors: vec![],
        };
        let resp = error.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let body = response_body(resp).await;
        assert_eq!(body["code"], "invalid_state_transition");
        assert!(body["errors"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn not_found_returns_404_with_resource_and_id() {
        let error = Error::NotFound {
            resource: ResourceKind::Job,
            id: "abc123".into(),
        };
        let resp = error.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let body = response_body(resp).await;
        assert_eq!(body["code"], "not_found");
        assert_eq!(body["resource"], "job");
        assert_eq!(body["id"], "abc123");
    }

    #[tokio::test]
    async fn internal_returns_500_without_cause() {
        let error = Error::internal("something went wrong");
        let resp = error.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = response_body(resp).await;
        assert_eq!(body["code"], "internal");
        // Cause must not be in the response
        assert!(body.get("cause").is_none());
    }

    #[tokio::test]
    async fn response_has_no_message_field() {
        let error = Error::BadRequest {
            code: ErrorCode::InputValidationFailed,
            errors: vec![],
        };
        let resp = error.into_response();
        let body = response_body(resp).await;
        assert!(body.get("message").is_none());
    }
}
