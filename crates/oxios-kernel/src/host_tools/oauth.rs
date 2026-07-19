//! OAuth device-code broker (RFC-041 Phase 3).
//!
//! Implements the device-authorization grant (RFC 8628). Per H1, the
//! `device_code` is a polling bearer secret and **never leaves the daemon** —
//! the client receives only `{ handle, user_code, verification_url, expires_in }`
//! and polls an opaque `handle`. The daemon owns the `device_code` in a
//! transient, auto-expiring in-memory map.
//!
//! Provider HTTP clients are Rust impls of [`OAuthProvider`] (D9): TOML selects
//! *which* provider + scopes, Rust implements *how*. The first impl is GitHub.
//! GitHub device-flow tokens do not expire by default, so `refresh` is a no-op
//! for GitHub — the trait still supports expiring providers.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use async_trait::async_trait;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

// ─── Provider trait (D9) ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
/// Outcome of polling a provider's token endpoint.
pub enum PollOutcome {
    /// User hasn't authorized yet — keep polling.
    Pending,
    /// Success — carries the token data so the broker can persist it under the
    /// correct `store_key`. The provider does NOT save (it has no store_key).
    Success {
        access_token: String,
        refresh_token: Option<String>,
        expires_in: i64,
        scope: Option<String>,
    },
    /// The device code expired — restart the flow.
    Expired,
    /// The user denied authorization.
    Denied,
}

/// A Rust implementation of one OAuth provider's device-code flow.
#[async_trait]
pub trait OAuthProvider: Send + Sync {
    /// Stable provider name (matches the registry's `credential.provider`).
    fn name(&self) -> &str;

    /// Request a device code. Returns the data the user needs (`user_code`,
    /// `verification_url`) plus the secret `device_code` + polling interval.
    async fn start(&self, scopes: &[String]) -> Result<DeviceCode>;

    /// Poll the token endpoint once. On success, store the token and return
    /// `Success`; otherwise return the pending/expired/denied state.
    async fn poll(&self, device_code: &str) -> Result<PollOutcome>;

    /// Refresh an expiring token. Returns the new `TokenBundle` so the
    /// broker can persist the full bundle (rotated `refresh_token`,
    /// fresh `expires_in`, scope changes). Static-key providers leave
    /// this unimplemented.
    async fn refresh(&self, refresh_token: &str) -> Result<oxi_sdk::TokenBundle>;

    /// Revoke a token at the provider (best-effort).
    async fn revoke(&self, token: &str) -> Result<()>;
}

/// Device-authorization response from `start()`.
#[derive(Debug, Clone)]
pub struct DeviceCode {
    /// The secret code the daemon uses to poll — NEVER sent to the client.
    pub device_code: String,
    /// The short code the user enters at the verification URL.
    pub user_code: String,
    /// The URL the user visits to authorize.
    pub verification_url: String,
    /// Seconds the user has to authorize.
    pub expires_in: u64,
    /// Minimum seconds between polls.
    pub interval: u64,
}

/// What the client receives from `/oauth/start` (no `device_code`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeResponse {
    /// Opaque handle for `/oauth/poll`. Maps to the daemon-held `device_code`.
    pub handle: String,
    pub user_code: String,
    pub verification_url: String,
    pub expires_in: u64,
}

/// What `/oauth/poll` returns — a simple client-facing status. The token data
/// never leaves the daemon (it's persisted internally by the broker).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PollResponse {
    /// `pending` | `success` | `expired` | `denied`.
    pub status: String,
}

impl PollResponse {
    fn pending() -> Self {
        Self {
            status: "pending".into(),
        }
    }
    fn success() -> Self {
        Self {
            status: "success".into(),
        }
    }
    fn from_outcome(o: &PollOutcome) -> Self {
        match o {
            PollOutcome::Pending => Self::pending(),
            PollOutcome::Success { .. } => Self::success(),
            PollOutcome::Expired => Self {
                status: "expired".into(),
            },
            PollOutcome::Denied => Self {
                status: "denied".into(),
            },
        }
    }
}

// ─── GitHub provider ─────────────────────────────────────────────────────────

/// GitHub OAuth device-code provider.
///
/// GitHub's device flow (https://docs.github.com/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#device-flow)
/// uses a public `client_id` and returns non-expiring tokens (no refresh).
pub struct GitHubProvider {
    client: reqwest::Client,
    /// Oxios's registered OAuth App client_id. Must be set to a real value for
    /// production; GitHub device flow accepts any OAuth App's client_id.
    client_id: String,
}

const GITHUB_DEVICE_URL: &str = "https://github.com/login/device/code";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

impl GitHubProvider {
    /// Build with the default Oxios client_id (override via `with_client_id`).
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            // TODO(RFC-041 §18): ship Oxios's own registered OAuth App client_id.
            // Until then this is a placeholder — set OXIOS_GITHUB_CLIENT_ID to
            // use your own OAuth App.
            client_id: std::env::var("OXIOS_GITHUB_CLIENT_ID")
                .unwrap_or_else(|_| "OxiosOAuthAppPlaceholder".into()),
        }
    }

    /// Override the client_id (tests / private deployments).
    #[allow(dead_code)]
    pub fn with_client_id(client_id: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            client_id,
        }
    }
}

impl Default for GitHubProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct GhDeviceResp {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Debug, Deserialize)]
struct GhTokenResp {
    access_token: Option<String>,
    error: Option<String>,
}

#[async_trait]
impl OAuthProvider for GitHubProvider {
    fn name(&self) -> &str {
        "github"
    }

    async fn start(&self, scopes: &[String]) -> Result<DeviceCode> {
        let resp = self
            .client
            .post(GITHUB_DEVICE_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("scope", &scopes.join(" ")),
            ])
            .send()
            .await
            .context("github device-code request")?;
        let body: GhDeviceResp = resp
            .json()
            .await
            .context("parsing github device response")?;
        Ok(DeviceCode {
            device_code: body.device_code,
            user_code: body.user_code,
            verification_url: body.verification_uri,
            expires_in: body.expires_in,
            interval: body.interval,
        })
    }

    async fn poll(&self, device_code: &str) -> Result<PollOutcome> {
        let grant = "urn:ietf:params:oauth:grant-type:device_code";
        let resp = self
            .client
            .post(GITHUB_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("device_code", device_code),
                ("grant_type", grant),
            ])
            .send()
            .await
            .context("github token poll request")?;
        let body: GhTokenResp = resp.json().await.context("parsing github token response")?;

        if let Some(token) = body.access_token {
            // GitHub device-flow tokens don't carry a refresh_token or expiry.
            // The provider does NOT persist — it returns the token data and the
            // broker saves it under the correct store_key (H1 fix).
            return Ok(PollOutcome::Success {
                access_token: token,
                refresh_token: None,
                expires_in: 0,
                scope: None,
            });
        }
        match body.error.as_deref() {
            Some("authorization_pending") => Ok(PollOutcome::Pending),
            Some("slow_down") => Ok(PollOutcome::Pending),
            Some("expired_token") => Ok(PollOutcome::Expired),
            Some("access_denied") => Ok(PollOutcome::Denied),
            Some(other) => anyhow::bail!("github oauth error: {other}"),
            None => anyhow::bail!("github oauth: no token and no error"),
        }
    }

    async fn refresh(&self, _refresh_token: &str) -> Result<oxi_sdk::TokenBundle> {
        // GitHub device-flow tokens don't expire — nothing to refresh.
        anyhow::bail!("github device-flow tokens do not support refresh")
    }

    async fn revoke(&self, token: &str) -> Result<()> {
        // GitHub revocation requires the client_secret (basic auth). Best-effort;
        // local removal proceeds even if this fails. With a placeholder
        // client_id this 401s, which is acceptable (the local token is still
        // deleted by the caller).
        let _ = self
            .client
            .delete(format!(
                "https://api.github.com/applications/{}/token",
                self.client_id
            ))
            .basic_auth(&self.client_id, Some(""))
            .json(&serde_json::json!({ "access_token": token }))
            .send()
            .await;
        Ok(())
    }
}

// ─── Broker — owns device_codes behind opaque handles (H1) ───────────────────

/// A pending device-code flow, keyed by an opaque handle.
struct PendingFlow {
    provider_name: String,
    device_code: String,
    store_key: String,
    expires_at: Instant,
}

/// The OAuth broker. Holds in-flight device-code flows in a transient,
/// auto-expiring map keyed by opaque handles. `device_code` never leaves here.
pub struct OAuthBroker {
    providers: HashMap<String, Arc<dyn OAuthProvider>>,
    flows: Mutex<HashMap<String, PendingFlow>>,
}

impl OAuthBroker {
    /// Assemble with the default provider set (GitHub).
    pub fn new() -> Self {
        let mut providers: HashMap<String, Arc<dyn OAuthProvider>> = HashMap::new();
        let gh: Arc<dyn OAuthProvider> = Arc::new(GitHubProvider::new());
        providers.insert(gh.name().to_string(), gh);
        Self {
            providers,
            flows: Mutex::new(HashMap::new()),
        }
    }

    fn provider(&self, name: &str) -> Result<Arc<dyn OAuthProvider>> {
        self.providers
            .get(name)
            .cloned()
            .with_context(|| format!("unknown OAuth provider '{name}'"))
    }

    /// Start a device-code flow. Returns the user-facing data (no `device_code`).
    /// The `device_code` is stored under a fresh opaque `handle`.
    pub async fn start(
        &self,
        provider_name: &str,
        store_key: &str,
        scopes: &[String],
    ) -> Result<DeviceCodeResponse> {
        let provider = self.provider(provider_name)?;
        let dc = provider.start(scopes).await?;
        let handle = format!("oc_{}", uuid::Uuid::new_v4().simple());
        let expires_at = Instant::now() + Duration::from_secs(dc.expires_in.max(1));
        self.flows.lock().insert(
            handle.clone(),
            PendingFlow {
                provider_name: provider_name.to_string(),
                device_code: dc.device_code,
                store_key: store_key.to_string(),
                expires_at,
            },
        );
        Ok(DeviceCodeResponse {
            handle,
            user_code: dc.user_code,
            verification_url: dc.verification_url,
            expires_in: dc.expires_in,
        })
    }

    /// Poll a flow by handle. Removes the flow on a terminal outcome.
    /// Per H1 the client never sees the `device_code`; the daemon polls the
    /// provider using the stored secret.
    pub async fn poll(&self, handle: &str) -> Result<PollResponse> {
        let entry = {
            let mut flows = self.flows.lock();
            // Expire stale flows.
            if let Some(flow) = flows.get(handle)
                && Instant::now() > flow.expires_at
            {
                flows.remove(handle);
                return Ok(PollResponse {
                    status: "expired".into(),
                });
            }
            flows.get(handle).map(|f| {
                (
                    f.provider_name.clone(),
                    f.device_code.clone(),
                    f.store_key.clone(),
                )
            })
        };
        let (provider_name, device_code, store_key) =
            entry.ok_or_else(|| anyhow::anyhow!("unknown or expired OAuth handle"))?;
        let provider = self.provider(&provider_name)?;
        let outcome = provider.poll(&device_code).await?;
        // On Success, the broker persists the token under the flow's store_key
        // (H1 fix: not hardcoded, not discarded, uses CredentialStore for legacy
        // migration). Error propagates so the UI doesn't show a false "Connected".
        let (terminal, response) = match outcome {
            PollOutcome::Success {
                access_token,
                refresh_token,
                expires_in,
                scope,
            } => {
                let bundle = oxi_sdk::TokenBundle {
                    access_token,
                    refresh_token,
                    token_type: "Bearer".to_string(),
                    obtained_at: chrono::Utc::now(),
                    expires_in: expires_in.max(0) as u64,
                    scope,
                };
                // Persist under the flow's store_key via CredentialStore (legacy
                // migration). Error propagates — no false "Connected".
                crate::credential::CredentialStore::store_token(&store_key, bundle)?;
                (true, PollResponse::success())
            }
            PollOutcome::Pending => (false, PollResponse::pending()),
            ref other => (true, PollResponse::from_outcome(other)),
        };
        if terminal {
            self.flows.lock().remove(handle);
        }
        Ok(response)
    }

    /// Revoke + remove a stored token for `store_key` (DELETE credential).
    pub async fn revoke(&self, provider_name: &str, token: &str) -> Result<()> {
        let provider = self.provider(provider_name)?;
        // Best-effort revoke; local removal is the caller's responsibility.
        let _ = provider.revoke(token).await;
        Ok(())
    }
}

impl Default for OAuthBroker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A provider that never succeeds — used to test broker handle/expiry logic
    /// without hitting the network.
    struct PendingProvider;
    #[async_trait]
    impl OAuthProvider for PendingProvider {
        fn name(&self) -> &str {
            "pending"
        }
        async fn start(&self, _scopes: &[String]) -> Result<DeviceCode> {
            Ok(DeviceCode {
                device_code: "dev-secret".into(),
                user_code: "USER-CODE".into(),
                verification_url: "https://example.com/device".into(),
                expires_in: 1,
                interval: 1,
            })
        }
        async fn poll(&self, _device_code: &str) -> Result<PollOutcome> {
            Ok(PollOutcome::Pending)
        }
        async fn refresh(&self, _: &str) -> Result<oxi_sdk::TokenBundle> {
            unreachable!()
        }
        async fn revoke(&self, _: &str) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn start_returns_no_device_code() {
        let broker = OAuthBroker {
            providers: {
                let mut m: HashMap<String, Arc<dyn OAuthProvider>> = HashMap::new();
                m.insert("pending".into(), Arc::new(PendingProvider));
                m
            },
            flows: Mutex::new(HashMap::new()),
        };
        let resp = broker.start("pending", "pending", &[]).await.unwrap();
        // The response must NOT contain the device_code — only user_code + handle.
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("user_code"));
        assert!(json.contains("handle"));
        assert!(
            !json.contains("dev-secret"),
            "device_code leaked to client!"
        );
    }

    #[tokio::test]
    async fn expired_handle_returns_expired() {
        let broker = OAuthBroker {
            providers: {
                let mut m: HashMap<String, Arc<dyn OAuthProvider>> = HashMap::new();
                m.insert("pending".into(), Arc::new(PendingProvider));
                m
            },
            flows: Mutex::new(HashMap::new()),
        };
        let resp = broker.start("pending", "pending", &[]).await.unwrap();
        // expires_in=1s; wait for expiry.
        tokio::time::sleep(Duration::from_millis(1100)).await;
        let pr = broker.poll(&resp.handle).await.unwrap();
        assert_eq!(pr.status, "expired");
    }

    #[tokio::test]
    async fn unknown_handle_errors() {
        let broker = OAuthBroker::new();
        assert!(broker.poll("nonexistent").await.is_err());
    }
}
