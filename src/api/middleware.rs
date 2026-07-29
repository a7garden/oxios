//! HTTP middleware for the Oxios web channel.
//!
//! Provides authentication and rate limiting for API endpoints.

use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Instant;

#[cfg(test)]
use axum::extract::ConnectInfo;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::api::server::AppState;

/// Simple token-bucket rate limiter for API endpoints.
/// Refills tokens at `refill_rate` per second, up to `max_tokens`.
#[derive(Debug)]
pub struct RateLimiter {
    state: Arc<Mutex<RateLimiterState>>,
    max_tokens: f64,
    refill_rate: f64,
    /// When true (max_requests_per_minute == 0), allow all requests.
    unlimited: bool,
}

#[derive(Debug)]
struct RateLimiterState {
    tokens: f64,
    last_refill: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter.
    ///
    /// `max_requests_per_minute` determines both burst size and refill rate.
    /// Pass `0` to disable rate limiting entirely (always allow).
    pub fn new(max_requests_per_minute: u32) -> Self {
        let unlimited = max_requests_per_minute == 0;
        let max_tokens = max_requests_per_minute as f64;
        Self {
            state: Arc::new(Mutex::new(RateLimiterState {
                tokens: max_tokens,
                last_refill: Instant::now(),
            })),
            max_tokens,
            refill_rate: max_tokens / 60.0,
            unlimited,
        }
    }

    /// Try to acquire one token. Returns true if allowed, false if rate limited.
    pub fn try_acquire(&self) -> bool {
        if self.unlimited {
            return true;
        }
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
            unlimited: self.unlimited,
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

/// Tailscale identity headers used for auth-proxy mode. Documented at
/// <https://tailscale.com/docs/features/tailscale-serve#identity-headers>.
/// `tailscale serve` injects these from the local Tailscale daemon after
/// stripping any client-supplied copies, so they are trustworthy when
/// the request arrives from a loopback peer.
pub const TAILSCALE_USER_LOGIN: &str = "tailscale-user-login";
/// Tries the Tailscale identity-header trust path. Returns `Some(user_login)`
/// when the request qualifies: Tailscale auth is enabled, the peer is
/// loopback (the local Tailscale daemon proxy), the identity header is
/// present, and the user is in the optional allowlist.
fn tailscale_identity_trust<B>(
    request: &Request<B>,
    tailscale_auth_enabled: bool,
    allow_users: &[String],
) -> Option<String> {
    if !tailscale_auth_enabled {
        return None;
    }
    let peer = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0)?;
    if !peer.ip().is_loopback() {
        // Tailscale Serve connects from 127.0.0.1, so a non-loopback
        // peer carrying these headers is a local-process spoof attempt —
        // we already trust loopback, so reject.
        return None;
    }
    let user = request
        .headers()
        .get(TAILSCALE_USER_LOGIN)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    if !allow_users.is_empty() && !allow_users.iter().any(|u| u == &user) {
        tracing::warn!(user = %user, "Rejected Tailscale user not in allowlist");
        return None;
    }
    Some(user)
}

/// Bearer token authentication middleware.
///
/// Applied via `from_fn_with_state`. Skips auth when `auth_enabled` is false.
/// `/health` and static assets are always accessible without auth.
///
/// Two valid credential paths:
/// 1. **`Authorization: Bearer <token>`** — the standard path, validated
///    against the kernel's auth manager, `[engine].api_key`, or the
///    `OXIOS_API_KEY` env var.
/// 2. **Tailscale identity headers** — when `tailscale_auth = true` AND
///    the peer is loopback AND `Tailscale-User-Login` is present AND
///    the user is in `tailscale_allow_users` (or the list is empty).
///    This is the `tailscale serve` auth-proxy model.
pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip auth if disabled
    if !state.config.read().security.auth_enabled {
        return Ok(next.run(request).await);
    }
    // Tailscale identity-header trust. This runs BEFORE the static-asset
    // short-circuit so that identity-header-authenticated remote clients
    // can hit any endpoint, not just API ones. Static assets don't need
    // auth at all (they're served by the SPA fallback), but routing them
    // through identity trust is harmless and keeps the contract uniform.
    let (tailscale_auth_enabled, tailscale_allow_users) = {
        let cfg = state.config.read();
        (cfg.security.tailscale_auth, cfg.security.tailscale_allow_users.clone())
    };
    if let Some(user) = tailscale_identity_trust(
        &request,
        tailscale_auth_enabled,
        &tailscale_allow_users,
    ) {
        // Audit the first identity-trusted request per user. Per-request
        // logging would drown the trail in repeats; one row per user is
        // the right signal-to-noise ratio. The session-id dimension was
        // dropped: there is no reliable browser session id that every
        // request carries, and per-user is sufficient for forensics.
        let audit_key = format!("tailscale-session:{user}");
        if !state.identity_trust_audit.lock().contains(&audit_key) {
            state.identity_trust_audit.lock().insert(audit_key);
            state.kernel.security.log_action(
                "tailscale",
                "auth_trust",
                &format!("user={user}"),
            );
        }
        return Ok(next.run(request).await);
    }

    // Allow health endpoint without auth
    let path = request.uri().path();
    if path == "/health" {
        return Ok(next.run(request).await);
    }
    // The WebSocket upgrade cannot carry a Bearer header (browsers forbid
    // custom headers on `new WebSocket()`). Authentication for the chat stream
    // is enforced by the handler via a short-lived `?ticket=` query param
    // (see `handle_chat_stream` in routes/chat.rs), so exempt it from the
    // header-based middleware check here.
    if path == "/api/chat/stream" {
        return Ok(next.run(request).await);
    }

    // Allow only actual static asset paths (prefix-based, not suffix)
    let static_prefixes = ["/assets/", "/favicon", "/knowledge/"];
    let is_static =
        static_prefixes.iter().any(|p| path.starts_with(p)) || path == "/" || path == "/index.html";
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

    // Resolve API key: [engine].api_key → OXIOS_API_KEY env var
    let config_key = state.config.read().api_key();
    let env_key = std::env::var("OXIOS_API_KEY")
        .ok()
        .filter(|k| !k.is_empty());

    let is_valid = {
        // Validate against auth_manager (kernel subsystem), config key, or env var
        let key_valid = state.kernel.security.validate_token(token);
        let config_valid = config_key.as_deref().map(|k| k == token).unwrap_or(false);
        let env_valid = env_key.as_deref().map(|k| k == token).unwrap_or(false);
        key_valid || config_valid || env_valid
    }; // guard dropped here
    if !is_valid {
        tracing::warn!(path = %request.uri().path(), "Authentication failed");
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}

/// Readiness gate middleware (RFC-024 SP4).
///
/// Returns 503 Service Unavailable for protected API routes while subsystems
/// are still warming up. Health endpoints (`/health`, `/health/ready`,
/// `/metrics`) and the SPA / static assets are always allowed so probes and
/// the dashboard shell can render. The deadline (30 s default) is enforced
/// here so a permanently missing engine cannot lock the gate forever.
pub async fn require_ready(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = request.uri().path();
    // Always-on endpoints bypass the gate.
    if path == "/health"
        || path == "/health/ready"
        || path == "/metrics"
        || path.starts_with("/assets/")
        || path == "/"
        || path == "/index.html"
    {
        return Ok(next.run(request).await);
    }

    // Deadline → any still-Warming subsystem becomes Degraded (still
    // counts as ready, but signals a partial setup to operators).
    state.readiness.enforce_deadline();

    if state.readiness.is_ready() {
        Ok(next.run(request).await)
    } else {
        tracing::debug!(path = %path, "Request blocked — subsystem not yet ready");
        let resp = (
            StatusCode::SERVICE_UNAVAILABLE,
            [(axum::http::header::RETRY_AFTER, "2")],
            "warming up",
        )
            .into_response();
        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use std::net::SocketAddr;

    fn req_with_peer(headers: &[(&str, &str)], peer: SocketAddr) -> Request<Body> {
        let mut builder = HttpRequest::builder().uri("/api/agents");
        for (k, v) in headers {
            builder = builder.header(*k, *v);
        }
        let mut req = builder.body(Body::empty()).expect("build request");
        req.extensions_mut().insert(ConnectInfo(peer));
        req
    }

    #[test]
    fn tailscale_identity_trust_disabled_returns_none() {
        let req = req_with_peer(
            &[("tailscale-user-login", "alice@example.com")],
            "127.0.0.1:4200".parse().unwrap(),
        );
        let out = tailscale_identity_trust(&req, false, &[]);
        assert!(out.is_none(), "tailscale_auth=false must skip trust");
    }

    #[test]
    fn tailscale_identity_trust_happy_path() {
        let req = req_with_peer(
            &[("tailscale-user-login", "alice@example.com")],
            "127.0.0.1:4200".parse().unwrap(),
        );
        let out = tailscale_identity_trust(&req, true, &[]);
        assert_eq!(out.as_deref(), Some("alice@example.com"));
    }

    #[test]
    fn tailscale_identity_trust_rejects_non_loopback_peer() {
        // 100.64.x.x is the Tailscale CGNAT range; a request from
        // there would be remote-spoofed (Caddy on the same host would
        // still connect from 127.0.0.1, so this case is unreachable in
        // practice — but the test pins the contract).
        let req = req_with_peer(
            &[("tailscale-user-login", "alice@example.com")],
            "100.64.0.1:4200".parse().unwrap(),
        );
        let out = tailscale_identity_trust(&req, true, &[]);
        assert!(out.is_none(), "non-loopback peer must not be trusted");
    }

    #[test]
    fn tailscale_identity_trust_without_header_returns_none() {
        let req = req_with_peer(&[], "127.0.0.1:4200".parse().unwrap());
        let out = tailscale_identity_trust(&req, true, &[]);
        assert!(out.is_none(), "missing identity header must not be trusted");
    }

    #[test]
    fn tailscale_identity_trust_respects_allowlist() {
        let allow = vec!["alice@example.com".to_string()];
        let req_alice = req_with_peer(
            &[("tailscale-user-login", "alice@example.com")],
            "127.0.0.1:4200".parse().unwrap(),
        );
        assert_eq!(
            tailscale_identity_trust(&req_alice, true, &allow).as_deref(),
            Some("alice@example.com")
        );

        let req_eve = req_with_peer(
            &[("tailscale-user-login", "eve@example.com")],
            "127.0.0.1:4200".parse().unwrap(),
        );
        assert!(
            tailscale_identity_trust(&req_eve, true, &allow).is_none(),
            "user not in allowlist must be rejected"
        );

        // Empty allowlist = allow everyone.
        let req_anyone = req_with_peer(
            &[("tailscale-user-login", "anyone@anywhere.com")],
            "127.0.0.1:4200".parse().unwrap(),
        );
        assert_eq!(
            tailscale_identity_trust(&req_anyone, true, &[]).as_deref(),
            Some("anyone@anywhere.com")
        );
    }

    #[test]
    fn tailscale_identity_trust_rejects_empty_header_value() {
        let req = req_with_peer(
            &[("tailscale-user-login", "   ")],
            "127.0.0.1:4200".parse().unwrap(),
        );
        let out = tailscale_identity_trust(&req, true, &[]);
        assert!(out.is_none(), "whitespace-only identity must be rejected");
    }
}
