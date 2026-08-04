//! `oxios serve --remote --pairing-address` helpers (RFC-044 §6.2, §6.7).
//!
//! Pure-logic building blocks — no I/O, no logging. Wired into `cmd_serve`
//! from `main.rs` (Task 10). The companion `PairingOffer`/QR pieces live in
//! `crate::remote::pairing`; this module only resolves the advertised endpoint
//! host and renders the single-line readiness JSON contract that automation
//! scripts scrape.
#![cfg(feature = "remote")]
#![allow(dead_code)] // public surface; cmd_serve wiring lands in this same task

use std::process::Command;

/// Resolve the advertised pairing endpoint, in priority order:
///   1. CLI override (`--pairing-address <host>` or `host:port`).
///   2. First non-loopback `tailscale ip -4` line (if the `tailscale` CLI is
///      installed and reachable).
///   3. OS `hostname()`.
///
/// Returns `None` when no candidate can be advertised. A wildcard override
/// (`0.0.0.0`, `::`, `*`, or empty) is intentionally rejected — those bind
/// addresses are never reachable from a remote client and would silently
/// produce a broken pairing URL.
pub fn resolve_advertised(override_host: Option<&str>, port: u16) -> Option<String> {
    if let Some(raw) = override_host {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        // Reject wildcards — `0.0.0.0`, `::`, `*` cannot be advertised.
        if matches!(trimmed, "0.0.0.0" | "::" | "*") {
            return None;
        }
        // If the user passed `host:port`, use it verbatim; otherwise append the
        // bind port. Splitting on the LAST `:` keeps IPv6 literals like
        // `[::1]:6768` working in the future; for plain v4 we accept a single
        // trailing `:port`.
        if let Some((host, port_str)) = trimmed.rsplit_once(':')
            && let Ok(parsed) = port_str.parse::<u16>()
            && !host.is_empty()
        {
            return Some(format!("ws://{host}:{parsed}"));
        }
        // No explicit port — append the bind port.
        return Some(format!("ws://{trimmed}:{port}"));
    }

    if let Some(ts) = tailscale_ipv4() {
        return Some(format!("ws://{ts}:{port}"));
    }

    if let Some(host) = hostname() {
        return Some(format!("ws://{host}:{port}"));
    }

    None
}

/// First IPv4 line from `tailscale ip -4`, trimmed of whitespace.
fn tailscale_ipv4() -> Option<String> {
    let output = Command::new("tailscale")
        .arg("ip")
        .arg("-4")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
}

/// OS hostname (best-effort). Returns `None` on platforms where we can't
/// resolve it.
fn hostname() -> Option<String> {
    hostname::get().ok().and_then(|os| os.into_string().ok())
}

/// Readiness contract emitted as a single JSON line on `oxios serve` startup
/// when `--remote` is set. Scripts (and Task 11's `oxios pair` UX) scrape this
/// to discover the bound port, advertised endpoint, and pairing URL without
/// having to query `/api/v1/remote` (which doesn't exist yet — Phase 2).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReadinessContract {
    /// Wire schema version. Bump when fields are added/renamed/removed.
    schema_version: u32,
    /// Address the WS listener actually bound to (e.g. `ws://127.0.0.1:6768`).
    /// Reflects the in-process bind — may be loopback even when the advertised
    /// endpoint points at Tailscale/LAN.
    bound_endpoint: String,
    /// Host:port a remote client should connect to (`ws://<host>:<port>`).
    /// `None` when no candidate could be resolved (no override, no tailscale,
    /// no hostname).
    advertised_endpoint: Option<String>,
    /// `oxios://pair?code=...` URL. `None` when identity/endpoint enumeration
    /// failed (e.g. workspace state dir unreadable).
    pairing_url: Option<String>,
    /// 16-hex-char device fingerprint of the daemon's Noise static key.
    device_id: String,
}

impl ReadinessContract {
    /// Wire version. Bump when the JSON layout changes.
    pub const SCHEMA_VERSION: u32 = 1;

    pub fn new(
        bound_endpoint: impl Into<String>,
        advertised_endpoint: Option<String>,
        pairing_url: Option<String>,
        device_id: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            bound_endpoint: bound_endpoint.into(),
            advertised_endpoint,
            pairing_url,
            device_id: device_id.into(),
        }
    }

    /// Single-line JSON suitable for stdout scraping by automation.
    pub fn to_json_line(&self) -> String {
        // `serde_json::to_string` already emits compact JSON; we explicitly
        // avoid `pretty` so the line is grep-friendly and stable for tests.
        serde_json::to_string(self).expect("ReadinessContract is always serializable")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins() {
        assert_eq!(
            resolve_advertised(Some("100.64.1.20"), 6768).as_deref(),
            Some("ws://100.64.1.20:6768")
        );
    }

    #[test]
    fn override_with_explicit_port_is_respected() {
        // user passed `foo.ts.net:9000` — keep that port verbatim.
        assert_eq!(
            resolve_advertised(Some("foo.ts.net:9000"), 6768).as_deref(),
            Some("ws://foo.ts.net:9000")
        );
    }

    #[test]
    fn reject_wildcard_override() {
        // 0.0.0.0 / * / :: cannot be advertised.
        assert!(resolve_advertised(Some("0.0.0.0"), 6768).is_none());
        assert!(resolve_advertised(Some("::"), 6768).is_none());
        assert!(resolve_advertised(Some("*"), 6768).is_none());
        assert!(resolve_advertised(Some(""), 6768).is_none());
        assert!(resolve_advertised(Some("   "), 6768).is_none());
    }

    #[test]
    fn no_override_falls_back_to_tailscale_or_hostname() {
        // Without an override the function consults the network. The test
        // only asserts the result is well-formed — either a real ws:// URL or
        // `None` when both `tailscale` and `hostname()` are unavailable.
        let out = resolve_advertised(None, 6768);
        if let Some(url) = out {
            assert!(url.starts_with("ws://"));
            assert!(url.ends_with(":6768"));
        }
    }

    #[test]
    fn readiness_contract_serializes_as_single_line() {
        let rc = ReadinessContract::new(
            "ws://127.0.0.1:6768",
            Some("ws://100.64.1.20:6768".to_string()),
            Some("oxios://pair?code=abc".to_string()),
            "deadbeefdeadbeef",
        );
        let line = rc.to_json_line();
        assert!(!line.contains('\n'), "readiness contract must be one line");
        let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed["schema_version"], 1);
        assert_eq!(parsed["bound_endpoint"], "ws://127.0.0.1:6768");
        assert_eq!(parsed["advertised_endpoint"], "ws://100.64.1.20:6768");
        assert_eq!(parsed["pairing_url"], "oxios://pair?code=abc");
        assert_eq!(parsed["device_id"], "deadbeefdeadbeef");
    }
}
