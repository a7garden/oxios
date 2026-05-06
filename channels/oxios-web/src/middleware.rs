//! HTTP middleware for the Oxios web channel.
//!
//! Provides authentication, input validation, and other cross-cutting concerns.

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::server::AppState;

/// Bearer token authentication middleware.
///
/// Skips authentication if `auth_enabled` is false in config.
/// Excludes `/health` from authentication requirements.
pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip auth if disabled
    if !state.config.security.auth_enabled {
        return Ok(next.run(request).await);
    }

    // Allow health endpoint without auth
    if request.uri().path() == "/health" {
        return Ok(next.run(request).await);
    }

    // Extract Authorization header
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v: &axum::http::HeaderValue| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate against AuthManager
    let mut auth = state.auth_manager.lock();
    if !auth.validate(token) {
        tracing::warn!(
            path = %request.uri().path(),
            "Authentication failed for request"
        );
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}
