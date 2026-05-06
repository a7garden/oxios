//! HTTP middleware for the Oxios web channel.
//!
//! Provides authentication and other cross-cutting concerns.

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::server::AppState;

/// Bearer token authentication middleware.
///
/// Applied via `from_fn_with_state`. Skips auth when `auth_enabled` is false.
/// `/health` and static assets are always accessible without auth.
pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip auth if disabled
    if !state.config.security.auth_enabled {
        return Ok(next.run(request).await);
    }

    // Allow health endpoint without auth
    let path = request.uri().path();
    if path == "/health" {
        return Ok(next.run(request).await);
    }

    // Allow static assets without auth
    if path.starts_with("/dioxus")
        || path.ends_with(".js")
        || path.ends_with(".css")
        || path.ends_with(".html")
    {
        return Ok(next.run(request).await);
    }

    // Extract Authorization header
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate against AuthManager
    let mut auth = state.auth_manager.lock();
    if !auth.validate(token) {
        tracing::warn!(path = %request.uri().path(), "Authentication failed");
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}
