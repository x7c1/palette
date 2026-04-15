use axum::extract::FromRequest;
use axum::extract::rejection::JsonRejection;

use crate::Error;
use crate::api_types::{ErrorCode, InputError, Location};

/// A JSON body extractor that returns structured error responses on rejection.
///
/// Unlike `axum::Json`, deserialization failures produce
/// `Error::BadRequest { code: InputValidationFailed, .. }` instead of
/// Axum's default plain-text rejection.
#[derive(Debug)]
pub struct ValidJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidJson<T>
where
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(value)) => Ok(ValidJson(value)),
            Err(rejection) => Err(Error::BadRequest {
                code: ErrorCode::InputValidationFailed,
                errors: vec![InputError {
                    location: Location::Body,
                    hint: String::new(),
                    reason: rejection_reason(&rejection),
                    help: None,
                }],
            }),
        }
    }
}

fn rejection_reason(rejection: &JsonRejection) -> String {
    match rejection {
        JsonRejection::JsonDataError(_) => "body/invalid_data".into(),
        JsonRejection::JsonSyntaxError(_) => "body/invalid_syntax".into(),
        JsonRejection::MissingJsonContentType(_) => "body/missing_content_type".into(),
        JsonRejection::BytesRejection(_) => "body/unreadable".into(),
        _ => "body/unknown".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::response::IntoResponse;

    async fn response_body(resp: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[derive(Debug, serde::Deserialize)]
    struct Sample {
        #[expect(dead_code)]
        title: String,
    }

    async fn extract(body: &str, content_type: &str) -> Result<ValidJson<Sample>, Error> {
        let req = Request::builder()
            .method("POST")
            .header("content-type", content_type)
            .body(Body::from(body.to_string()))
            .unwrap();
        ValidJson::<Sample>::from_request(req, &()).await
    }

    #[tokio::test]
    async fn valid_json_extracts_successfully() {
        let result = extract(r#"{"title":"hello"}"#, "application/json").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn invalid_syntax_returns_400_with_json_error() {
        let err = extract("not json", "application/json").await.unwrap_err();
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let body = response_body(resp).await;
        assert_eq!(body["code"], "input_validation_failed");
        assert_eq!(body["errors"][0]["location"], "body");
        assert_eq!(body["errors"][0]["reason"], "body/invalid_syntax");
    }

    #[tokio::test]
    async fn invalid_data_returns_400_with_json_error() {
        // Valid JSON but wrong shape
        let err = extract(r#"{"wrong":"field"}"#, "application/json")
            .await
            .unwrap_err();
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let body = response_body(resp).await;
        assert_eq!(body["code"], "input_validation_failed");
        assert_eq!(body["errors"][0]["reason"], "body/invalid_data");
    }

    #[tokio::test]
    async fn missing_content_type_returns_400() {
        let err = extract(r#"{"title":"hello"}"#, "text/plain")
            .await
            .unwrap_err();
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let body = response_body(resp).await;
        assert_eq!(body["code"], "input_validation_failed");
        assert_eq!(body["errors"][0]["reason"], "body/missing_content_type");
    }
}
