//! Web API authentication routes (RFC-042 §7.2).
//!
//! `POST /api/auth/issue` — loopback-only, no-auth token issuance.
//!
//! Purpose: a browser hitting the dashboard from `http://127.0.0.1:<port>`
//! is necessarily on the same machine as the daemon. The browser asks
//! for a token, the daemon returns a freshly generated Bearer key, the
//! browser stores it in `sessionStorage` and proceeds. No copy/paste
//! from the terminal required.
//!
//! Security boundary:
//!   * The request must originate from a loopback IP (127.0.0.0/8 or
//!     ::1). A non-loopback client gets 403 — the token never leaves
//!     localhost.
//!   * The endpoint is only registered when `auth_enabled=true` (see
//!     `require_auth` exemption list) — without auth, the dashboard
//!     doesn't need a token at all.
//!   * The handler rejects with 503 when no keys have been bootstrapped
//!     yet (e.g. the user disabled the auto-issue banner and is running
//!     headless). The front-end then falls back to manual paste.
//!
//! This is the answer to the "how does a brand-new user get a token?"
//! UX problem: instead of `oxios auth init` then copy/paste, the Web UI
//! just *gets* the token on first load. The terminal banner remains as
//! a backup for headless / non-browser setups.

use axum::{
    Json,
    extract::{ConnectInfo, State},
};
use serde::Serialize;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use crate::api::error::AppError;
use crate::api::server::AppState;

#[derive(Debug, Serialize)]
pub(crate) struct IssueResponse {
    /// The full Bearer token. Treated as a one-time read; the browser
    /// stores it in `sessionStorage` and the user never sees it again.
    token: String,
    /// Stable key name (the "default" key from first-boot, or a
    /// `session:default` derivative). Lets the front-end label the
    /// token in dev tools if it ever wants to.
    name: String,
    /// Whether this daemon's gateway binds to a non-loopback interface.
    /// Front-end uses this to surface the "loopback-only" disclaimer.
    loopback_only: bool,
}

fn is_loopback_ip(addr: &SocketAddr) -> bool {
    match addr.ip() {
        IpAddr::V4(v4) => v4.is_loopback(),
        // `Ipv6Addr::is_loopback` covers `::1`. `is_unspecified` (::)
        // is included because in some proxy setups the daemon sees ::
        // but the connection is still local.
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified(),
    }
}

/// POST /api/auth/issue — issue a token for a loopback browser.
///
/// Returns 403 when called from a non-loopback client, 503 when the
/// daemon owns no keys, 200 with the token otherwise.
pub(crate) async fn handle_auth_issue(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
) -> Result<Json<IssueResponse>, AppError> {
    if !is_loopback_ip(&peer) {
        tracing::warn!(peer = %peer, "Rejected /api/auth/issue from non-loopback client");
        return Err(AppError::Forbidden(
            "/api/auth/issue is only available from loopback clients".into(),
        ));
    }

    if !state.config.read().security.auth_enabled {
        return Err(AppError::Forbidden("auth is disabled; no token to issue".into()));
    }

    let security = &state.kernel.security;
    let keys = security.list_api_keys();
    if keys.is_empty() {
        return Err(AppError::ServiceUnavailable(
            "no API keys configured; first-boot auto-issue did not run yet".into(),
        ));
    }

    // Generate a fresh `session:<base>` key for this browser. Each
    // browser session gets its own token so revoking one (via the
    // future `oxios auth revoke` CLI) doesn't lock out the others.
    // The base key is the "default" key created by
    // `auto_issue_first_boot_token` — we filter explicitly so that
    // the base does not drift into "session:default" on subsequent
    // calls (HashMap iteration order is non-deterministic and earlier
    // calls would have produced a `session:default` we do NOT want
    // to re-wrap).
    let base_name = keys
        .iter()
        .find(|k| k.name == "default")
        .map(|k| k.name.clone())
        .unwrap_or_else(|| keys[0].name.clone());
    let session_name = format!("session:{base_name}");
    let token = security
        .generate_api_key(&session_name)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Audit the issuance. This is the only call path that mints a new
    // Bearer token at runtime, so the audit row is uniquely valuable —
    // it tells the operator exactly when a browser session got a
    // credential and from which loopback peer.
    security.log_action("api:auth_issue", "issue", &format!("name={session_name} peer={peer}"));

    let cfg = state.config.read();
    let host = cfg.gateway.host.clone();
    let is_loopback_bind = is_loopback_bind_host(&host);

    Ok(Json(IssueResponse {
        token,
        name: session_name,
        loopback_only: is_loopback_bind,
    }))
}
/// Whether a gateway `host` string binds to a loopback interface. Mirrors
/// the private `is_loopback_host` in `plugin.rs` so the issue endpoint
/// can label its response without depending on plugin internals.
fn is_loopback_bind_host(host: &str) -> bool {
    let h = host.trim().to_ascii_lowercase();
    if h.is_empty() || h == "localhost" {
        return true;
    }
    let h = h.trim_start_matches('[').trim_end_matches(']');
    if h == "::1" {
        return true;
    }
    if let Some(rest) = h.strip_prefix("127.")
        && let Some(first) = rest.split('.').next()
        && first.bytes().all(|b| b.is_ascii_digit())
    {
        return true;
    }
    false
}
