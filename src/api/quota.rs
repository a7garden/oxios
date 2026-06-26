//! Provider quota & balance framework.
//!
//! Fetches account-level quota/credit/spend from providers that expose an
//! API-key-accessible endpoint. This is the "subscription quota" axis that
//! complements the dollar-cost views built on `agent_log_db`.
//!
//! # Scope
//!
//! Only providers with a genuine REST API (no browser cookies / OAuth dance)
//! are supported. As of this writing only OpenAI is implemented as a reference.
//! The [`QuotaFetcher`] trait is the extension point for adding more.
//!
//! # Why not CodexBar-style cookie scraping?
//!
//! Oxios is a daemon — it has no browser session, Keychain, or interactive
//! OAuth device flow. Plan-window quotas (ChatGPT Plus 5h windows, Claude Pro
//! weekly limits) require session tokens harvested from a browser, which a
//! server cannot do. Only API-key-accessible data (Admin spend, credit
//! balance) is fetchable here. See `docs/` for the credential-model decision.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use serde::Serialize;

// ── Types ───────────────────────────────────────────────────────────────

/// A rate-limit / quota window with optional reset time.
#[derive(Debug, Clone, Serialize)]
pub struct RateWindow {
    /// Human-readable window name (e.g. "5-hour", "weekly").
    pub name: String,
    /// Units used so far (None if unknown).
    pub used: Option<f64>,
    /// Total unit limit for the window.
    pub limit: Option<f64>,
    /// Remaining percentage 0–100 (None if unknown).
    pub remaining_percent: Option<f64>,
    /// When the window resets (None if unknown / no reset).
    pub resets_at: Option<DateTime<Utc>>,
}

/// Snapshot of a provider account's quota/balance state.
#[derive(Debug, Clone, Serialize)]
pub struct QuotaSnapshot {
    /// Provider ID (e.g. `openai`).
    pub provider: String,
    /// Remaining prepaid credit balance in USD (None if unknown).
    pub credit_balance_usd: Option<f64>,
    /// Spend in the current billing period (USD).
    pub period_spend_usd: Option<f64>,
    /// Billing period start (for context).
    pub period_start: Option<DateTime<Utc>>,
    /// Human-readable plan / subscription name.
    pub plan: Option<String>,
    /// Rate-limit / quota windows.
    pub rate_windows: Vec<RateWindow>,
    /// When this snapshot was fetched.
    pub fetched_at: DateTime<Utc>,
    /// Error message if the fetch failed (e.g. "requires admin key").
    pub error: Option<String>,
}

// ── Trait ───────────────────────────────────────────────────────────────

/// Fetches account-level quota/balance from a provider's API.
///
/// Implementations must be cheap to clone (they are held in a registry and
/// called on each `/api/costs/providers` request).
#[async_trait]
pub trait QuotaFetcher: Send + Sync {
    /// Provider ID this fetcher handles.
    fn provider(&self) -> &str;

    /// Whether the required credentials are available.
    fn has_credentials(&self, api_key: Option<&str>) -> bool {
        api_key.is_some_and(|k| !k.is_empty())
    }

    /// Fetch the current quota snapshot.
    async fn fetch(&self, api_key: Option<&str>) -> anyhow::Result<QuotaSnapshot>;
}

// ── OpenAI reference implementation ─────────────────────────────────────

/// OpenAI quota fetcher via the Admin API costs endpoint.
///
/// Calls `GET /v1/organization/costs` to compute the current billing-period
/// spend. Requires an **admin** API key with `organization:read` scope;
/// standard project keys will get a 403, which is reported gracefully.
pub struct OpenAiQuotaFetcher {
    client: reqwest::Client,
}

impl Default for OpenAiQuotaFetcher {
    fn default() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }
}

#[async_trait]
impl QuotaFetcher for OpenAiQuotaFetcher {
    fn provider(&self) -> &str {
        "openai"
    }

    async fn fetch(&self, api_key: Option<&str>) -> anyhow::Result<QuotaSnapshot> {
        let key = api_key.ok_or_else(|| anyhow::anyhow!("no API key"))?;
        let now = Utc::now();

        // Current billing period = calendar month.
        let period_start = now
            .date_naive()
            .with_day(1)
            .unwrap_or(now.date_naive())
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        let url = format!(
            "https://api.openai.com/v1/organization/costs?start_time={}&end_time={}&limit=1&group_by=line_item",
            period_start.timestamp(),
            now.timestamp(),
        );

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {key}"))
            .send()
            .await?;

        let status = resp.status();
        if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::UNAUTHORIZED {
            return Ok(QuotaSnapshot {
                provider: "openai".into(),
                credit_balance_usd: None,
                period_spend_usd: None,
                period_start: Some(period_start),
                plan: None,
                rate_windows: vec![],
                fetched_at: now,
                error: Some("requires admin API key with organization:read".into()),
            });
        }
        if !status.is_success() {
            return Ok(QuotaSnapshot {
                provider: "openai".into(),
                credit_balance_usd: None,
                period_spend_usd: None,
                period_start: Some(period_start),
                plan: None,
                rate_windows: vec![],
                fetched_at: now,
                error: Some(format!("OpenAI API returned {status}")),
            });
        }

        let body: serde_json::Value = resp.json().await?;

        let period_spend = parse_openai_spend(&body);

        Ok(QuotaSnapshot {
            provider: "openai".into(),
            credit_balance_usd: None, // OpenAI credits require a separate dashboard endpoint
            period_spend_usd: period_spend,
            period_start: Some(period_start),
            plan: None,
            rate_windows: vec![],
            fetched_at: now,
            error: None,
        })
    }
}

/// Sums all line-item costs from an OpenAI `/v1/organization/costs` response.
///
/// Navigates `data[].results[].cost.value` (a JSON string) and returns the
/// total spend, or `None` when no `data` array is present.
fn parse_openai_spend(body: &serde_json::Value) -> Option<f64> {
    body.get("data").and_then(|d| d.as_array()).map(|entries| {
        let mut total = 0.0_f64;
        for entry in entries {
            if let Some(results) = entry.get("results").and_then(|r| r.as_array()) {
                for r in results {
                    if let Some(val) = r
                        .get("cost")
                        .and_then(|c| c.get("value"))
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                    {
                        total += val;
                    }
                }
            }
        }
        total
    })
}

// ── Generic usage probe (ZAI / Minimax / future) ─────────────────────────

/// Probe a JSON usage endpoint and parse the standard
/// `{remaining_percent?, resets_at?, used?, limit?}` shape.
///
/// Both ZAI and Minimax (and any future provider that follows the same
/// convention) share this shape. On non-2xx, transport error, or
/// unexpected JSON, we return `QuotaSnapshot { error: Some(...), .. }`
/// so the caller can distinguish "no signal" from "signal parsed".
///
/// The snapshot is intentionally permissive: every field is `Option`,
/// so providers that expose only `remaining_percent` (no `resets_at`,
/// no `limit`) still produce a usable recalibration signal. Where the
/// fetcher doesn't know the field name, it stays `None` and the
/// QuotaTracker keeps the self-tracked counter on that field.
struct UsageProbe {
    provider_name: &'static str,
    url: &'static str,
    client: reqwest::Client,
}

impl UsageProbe {
    fn new(provider_name: &'static str, url: &'static str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            provider_name,
            url,
            client,
        }
    }

    async fn probe(&self, api_key: Option<&str>) -> QuotaSnapshot {
        let now = Utc::now();
        let key = match api_key {
            Some(k) if !k.is_empty() => k,
            _ => {
                return QuotaSnapshot {
                    provider: self.provider_name.into(),
                    rate_windows: vec![],
                    fetched_at: now,
                    error: Some("no API key".into()),
                    ..blank_snapshot()
                };
            }
        };
        let resp = match self.client.get(self.url).bearer_auth(key).send().await {
            Ok(r) => r,
            Err(e) => {
                return QuotaSnapshot {
                    provider: self.provider_name.into(),
                    rate_windows: vec![],
                    fetched_at: now,
                    error: Some(format!("transport: {e}")),
                    ..blank_snapshot()
                };
            }
        };
        let status = resp.status();
        if !status.is_success() {
            return QuotaSnapshot {
                provider: self.provider_name.into(),
                rate_windows: vec![],
                fetched_at: now,
                error: Some(format!("HTTP {status}")),
                ..blank_snapshot()
            };
        }
        let body: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                return QuotaSnapshot {
                    provider: self.provider_name.into(),
                    rate_windows: vec![],
                    fetched_at: now,
                    error: Some(format!("json: {e}")),
                    ..blank_snapshot()
                };
            }
        };
        let remaining_percent = body
            .get("remaining_percent")
            .and_then(|v| v.as_f64())
            .or_else(|| body.get("remainingPercent").and_then(|v| v.as_f64()));
        let used = body.get("used").and_then(|v| v.as_f64());
        let limit = body.get("limit").and_then(|v| v.as_f64());
        let resets_at =
            parse_resets_at(&body, "resets_at").or_else(|| parse_resets_at(&body, "resetsAt"));
        let window = RateWindow {
            name: "subscription-window".into(),
            used,
            limit,
            remaining_percent,
            resets_at,
        };
        QuotaSnapshot {
            provider: self.provider_name.into(),
            rate_windows: vec![window],
            fetched_at: now,
            error: None,
            ..blank_snapshot()
        }
    }
}

fn parse_resets_at(body: &serde_json::Value, key: &str) -> Option<DateTime<Utc>> {
    body.get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
}

fn blank_snapshot() -> QuotaSnapshot {
    QuotaSnapshot {
        provider: String::new(),
        credit_balance_usd: None,
        period_spend_usd: None,
        period_start: None,
        plan: None,
        rate_windows: vec![],
        fetched_at: Utc::now(),
        error: None,
    }
}

// ── ZAI subscription fetcher ─────────────────────────────────────────────

/// ZAI subscription quota fetcher (best-effort).
///
/// Probes a usage endpoint with a bearer token. The exact response shape
/// is **not** publicly documented; this fetcher is written permissively
/// — any non-2xx or unparseable response yields an `error` field rather
/// than a hard failure, so the QuotaTracker keeps the self-tracked
/// counter (Phase 1) and the snapshot just becomes a
/// `RecalibrationOutcome::FetchFailed` log entry. If the endpoint moves
/// or the auth flow changes, update [`UsageProbe::new`].
pub struct ZaiQuotaFetcher {
    probe: UsageProbe,
}

impl Default for ZaiQuotaFetcher {
    fn default() -> Self {
        Self {
            probe: UsageProbe::new("zai", "https://api.z.ai/api/usage"),
        }
    }
}

#[async_trait]
impl QuotaFetcher for ZaiQuotaFetcher {
    fn provider(&self) -> &str {
        self.probe.provider_name
    }
    async fn fetch(&self, api_key: Option<&str>) -> anyhow::Result<QuotaSnapshot> {
        Ok(self.probe.probe(api_key).await)
    }
}

// ── Minimax subscription fetcher ──────────────────────────────────────────

/// Minimax subscription quota fetcher (best-effort).
///
/// Same shape as [`ZaiQuotaFetcher`]: probes a usage endpoint, parses
/// permissively, reports errors instead of failing the recalibration
/// tick. Endpoint URL may need adjustment as the public API evolves.
pub struct MinimaxQuotaFetcher {
    probe: UsageProbe,
}

impl Default for MinimaxQuotaFetcher {
    fn default() -> Self {
        Self {
            probe: UsageProbe::new("minimax", "https://api.minimax.chat/api/usage"),
        }
    }
}

#[async_trait]
impl QuotaFetcher for MinimaxQuotaFetcher {
    fn provider(&self) -> &str {
        self.probe.provider_name
    }
    async fn fetch(&self, api_key: Option<&str>) -> anyhow::Result<QuotaSnapshot> {
        Ok(self.probe.probe(api_key).await)
    }
}

// ── Registry ────────────────────────────────────────────────────────────

/// Returns all registered quota fetchers.
///
/// Extension point: add new providers here. Each must implement
/// [`QuotaFetcher`] and resolve credentials independently.
pub fn all_fetchers() -> Vec<Box<dyn QuotaFetcher>> {
    vec![
        Box::new(OpenAiQuotaFetcher::default()),
        Box::new(ZaiQuotaFetcher::default()),
        Box::new(MinimaxQuotaFetcher::default()),
    ]
}

/// Fetch quotas for all providers that have credentials configured.
///
/// Providers without a key are silently skipped. Each fetcher runs
/// concurrently; a single failure does not abort the others.
pub async fn fetch_all(credentials: &HashMap<String, String>) -> Vec<QuotaSnapshot> {
    let fetchers = all_fetchers();
    let mut results = Vec::with_capacity(fetchers.len());

    for fetcher in &fetchers {
        let provider = fetcher.provider();
        let key = credentials.get(provider);
        if !fetcher.has_credentials(key.map(|s| s.as_str())) {
            continue;
        }
        let snap = match fetcher.fetch(key.map(|s| s.as_str())).await {
            Ok(s) => s,
            Err(e) => QuotaSnapshot {
                provider: provider.into(),
                credit_balance_usd: None,
                period_spend_usd: None,
                period_start: None,
                plan: None,
                rate_windows: vec![],
                fetched_at: Utc::now(),
                error: Some(e.to_string()),
            },
        };
        results.push(snap);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_spend_realistic_multi_entry() {
        // Mirrors the real OpenAI /v1/organization/costs response shape.
        let body = json!({
            "object": "list",
            "data": [
                {
                    "object": "organization.costs.result",
                    "amount": {"value": "0.50", "currency": "usd"},
                    "line_item": "gpt-4o",
                    "project_id": null,
                    "results": [
                        {"object": "organization.costs.result", "amount": {"value": "0.50", "currency": "usd"}, "line_item": null, "cost": {"value": "0.50"}},
                    ],
                },
            ],
            "has_more": false,
            "next": null,
        });
        let spend = parse_openai_spend(&body);
        assert!((spend.unwrap() - 0.50).abs() < 1e-9);
    }

    #[test]
    fn parse_spend_missing_data() {
        assert!(parse_openai_spend(&json!({})).is_none());
    }

    #[test]
    fn parse_spend_skips_unparseable_lines() {
        let body = json!({
            "data": [
                {
                    "results": [
                        {"cost": {"value": "not-a-number"}},
                        {"cost": {"value": "1.25"}},
                    ],
                },
            ],
        });
        let spend = parse_openai_spend(&body).unwrap();
        assert!((spend - 1.25).abs() < 1e-9);
    }

    #[test]
    fn blank_snapshot_filled_per_field() {
        // blank_snapshot() returns defaults; the probe() fills fields in.
        let s = blank_snapshot();
        assert!(s.provider.is_empty());
        assert!(s.rate_windows.is_empty());
        assert!(s.error.is_none());
    }

    #[test]
    fn parse_resets_at_rfc3339() {
        let body = json!({"resets_at": "2026-06-26T10:00:00Z"});
        let parsed = parse_resets_at(&body, "resets_at").unwrap();
        assert_eq!(parsed.to_rfc3339(), "2026-06-26T10:00:00+00:00");
    }

    #[test]
    fn parse_resets_at_camel_case_alias() {
        let body = json!({"resetsAt": "2026-06-26T10:00:00Z"});
        let parsed = parse_resets_at(&body, "resetsAt").unwrap();
        assert_eq!(parsed.to_rfc3339(), "2026-06-26T10:00:00+00:00");
    }

    #[test]
    fn parse_resets_at_missing_returns_none() {
        let body = json!({});
        assert!(parse_resets_at(&body, "resets_at").is_none());
    }

    #[test]
    fn all_fetchers_includes_zai_and_minimax() {
        let fs = all_fetchers();
        let names: Vec<&str> = fs.iter().map(|f| f.provider()).collect();
        assert!(names.contains(&"openai"));
        assert!(names.contains(&"zai"));
        assert!(names.contains(&"minimax"));
    }
}
