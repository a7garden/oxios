//! Unified error type for the Oxios web API.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Application-level error with HTTP status mapping.
#[derive(Debug)]
pub enum AppError {
    /// Resource not found.
    NotFound(String),
    /// Bad request from the client.
    BadRequest(String),
    /// Internal server error.
    Internal(String),
    /// Authentication required or failed.
    Unauthorized(String),
    /// Permission denied.
    Forbidden(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::NotFound(m) => write!(f, "Not Found: {m}"),
            AppError::BadRequest(m) => write!(f, "Bad Request: {m}"),
            AppError::Internal(m) => write!(f, "Internal Error: {m}"),
            AppError::Unauthorized(m) => write!(f, "Unauthorized: {m}"),
            AppError::Forbidden(m) => write!(f, "Forbidden: {m}"),
        }
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(m) => (StatusCode::NOT_FOUND, m.clone()),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            AppError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m.clone()),
            AppError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m.clone()),
            AppError::Forbidden(m) => (StatusCode::FORBIDDEN, m.clone()),
        };
        let body = json!({ "error": message });
        (status, axum::Json(body)).into_response()
    }
}

// Convenience conversions

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        assert_eq!(
            AppError::NotFound("test".into()).to_string(),
            "Not Found: test"
        );
        assert_eq!(
            AppError::BadRequest("bad".into()).to_string(),
            "Bad Request: bad"
        );
    }

    #[test]
    fn test_anyhow_conversion() {
        let err: AppError = anyhow::anyhow!("something failed").into();
        match err {
            AppError::Internal(msg) => assert_eq!(msg, "something failed"),
            _ => panic!("Expected Internal variant"),
        }
    }
}
