//! HTTP middleware for the Oxios web channel.
//!
//! Provides authentication and rate limiting for API endpoints.

use std::sync::Arc;
use std::time::Instant;
use parking_lot::Mutex;

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};

use crate::server::AppState;

/// Simple token-bucket rate limiter for API endpoints.
/// Refills tokens at `refill_rate` per second, up to `max_tokens`.
#[derive(Debug)]
pub struct RateLimiter {
    state: Arc<Mutex<RateLimiterState>>,
    max_tokens: f64,
    refill_rate: f64,
}

#[derive(Debug)]
struct RateLimiterState {
    tokens: f64,
    last_refill: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter.
    /// `max_requests_per_minute` determines both burst size and refill rate.
    pub fn new(max_requests_per_minute: u32) -> Self {
        let max_tokens = max_requests_per_minute as f64;
        Self {
            state: Arc::new(Mutex::new(RateLimiterState {
                tokens: max_tokens,
                last_refill: Instant::now(),
            })),
            max_tokens,
            refill_rate: max_tokens / 60.0,
        }
    }

    /// Try to acquire one token. Returns true if allowed, false if rate limited.
    pub fn try_acquire(&self) -> bool {
        let mut state = self.state.lock();
        let now = Instant::now();
        let elapsed = (now - state.last_refill).as_secs_f64();
        
        // Refill tokens based on elapsed time.
        state.tokens = (state.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        state.last_refill = now;

        if state.tokens >= 1.0 {
            state.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            max_tokens: self.max_tokens,
            refill_rate: self.refill_rate,
        }
    }
}

/// Axum middleware that applies rate limiting.
pub async fn rate_limit_layer(
    State(limiter): State<RateLimiter>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if limiter.try_acquire() {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::TOO_MANY_REQUESTS)
    }
}

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
    if !state.config.read().security.auth_enabled {
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

    // Also allow OXIOS_API_KEY env var or static config key as fallback
    let env_key = std::env::var("OXIOS_API_KEY").ok().filter(|k| !k.is_empty());
    let config_key = state.config.read().security.default_api_key.clone();

    let is_valid = {
        let mut auth = state.auth_manager.lock();
        let key_valid = auth.validate(token);
        // Also accept OXIOS_API_KEY env or static config key
        let env_valid = env_key.as_deref().map(|k| *k == token).unwrap_or(false);
        let config_valid = config_key.as_deref().map(|k| k == token).unwrap_or(false);
        key_valid || env_valid || config_valid
    }; // guard dropped here
    if !is_valid {
        tracing::warn!(path = %request.uri().path(), "Authentication failed");
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}
