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

    // Allow only actual static asset paths (prefix-based, not suffix)
    let static_prefixes = ["/assets/", "/dioxus/", "/favicon"];
    let is_static = static_prefixes.iter().any(|p| path.starts_with(p))
        || path == "/" || path == "/index.html";
    if is_static {
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
    let is_valid = {
        let mut auth = state.auth_manager.lock();
        auth.validate(token)
    }; // guard dropped here
    if !is_valid {
        tracing::warn!(path = %request.uri().path(), "Authentication failed");
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}
