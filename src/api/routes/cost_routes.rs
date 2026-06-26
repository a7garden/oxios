//! Cost & spend API routes — dollar-based views over agent_log_db.
//!
//! Replaces the dead token-based BudgetManager endpoints with real spend
//! data aggregated from the SQLite agent log (the actual source of truth for
//! per-agent `cost_usd` recorded by oxi-sdk's CostTracker at execution time).

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use chrono::{DateTime, Datelike, Duration, Utc};
use crate::api::error::AppError;
use crate::api::server::AppState;
use serde::Deserialize;

/// Query parameter selecting a relative spend window.
#[derive(Debug, Clone, Deserialize)]
pub struct PeriodParams {
    /// `today` | `week` | `month` | `all` (default: `all`).
    #[serde(default)]
    pub period: Option<String>,
}

impl PeriodParams {
    /// Resolve the period into an inclusive lower-bound timestamp.
    /// `None` means all-time (no lower bound).
    fn since(&self) -> Option<DateTime<Utc>> {
        match self.period.as_deref().unwrap_or("all") {
            "today" => Some(Utc::now().date_naive().and_hms_opt(0, 0, 0)?.and_utc()),
            "week" => Some(Utc::now() - Duration::days(7)),
            "month" => Some(Utc::now() - Duration::days(30)),
            _ => None, // "all" or unknown → all-time
        }
    }
}

/// GET /api/costs/summary — aggregate spend (total $, tokens, agent count).
pub(crate) async fn handle_cost_summary(
    state: State<Arc<AppState>>,
    Query(params): Query<PeriodParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let since = params.since();
    let summary = state.kernel.agents.cost_summary(since)?;

    // Include monthly spend limit + month-to-date spend for the spend-limit
    // progress bar. MTD is always the calendar month (1st 00:00 UTC),
    // independent of the `period` filter.
    let month_start = Utc::now()
        .date_naive()
        .with_day(1)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|t| t.and_utc());
    let mtd_summary = state.kernel.agents.cost_summary(month_start)?;
    let limit = state.config.read().budget.monthly_spend_limit_usd;

    Ok(Json(serde_json::json!({
        "total_cost_usd": summary.total_cost_usd,
        "total_tokens": summary.total_tokens,
        "agent_count": summary.agent_count,
        "period": params.period.as_deref().unwrap_or("all"),
        "spend_limit_usd": limit,
        "month_to_date_spend_usd": mtd_summary.total_cost_usd,
        "month_to_date_tokens": mtd_summary.total_tokens,
    })))
}

/// GET /api/costs/by-model — per-model spend breakdown.
pub(crate) async fn handle_cost_by_model(
    state: State<Arc<AppState>>,
    Query(params): Query<PeriodParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let since = params.since();
    let rows = state.kernel.agents.cost_by_model(since)?;
    let items: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "model_id": r.model_id,
                "cost_usd": r.cost_usd,
                "tokens": r.tokens,
                "agent_count": r.agent_count,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({ "items": items })))
}

/// GET /api/costs/by-project — per-project spend breakdown.
pub(crate) async fn handle_cost_by_project(
    state: State<Arc<AppState>>,
    Query(params): Query<PeriodParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let since = params.since();
    let rows = state.kernel.agents.cost_by_project(since)?;
    let items: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "project_id": r.project_id,
                "cost_usd": r.cost_usd,
                "tokens": r.tokens,
                "agent_count": r.agent_count,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({ "items": items })))
}

/// GET /api/costs/daily — daily spend time-series.
#[derive(Debug, Clone, Deserialize)]
pub struct DailyParams {
    /// Number of days to include (default 30, max 365).
    #[serde(default = "default_days")]
    pub days: u32,
}

fn default_days() -> u32 {
    30
}

impl DailyParams {
    fn days(&self) -> u32 {
        self.days.clamp(1, 365)
    }
}

pub(crate) async fn handle_cost_daily(
    state: State<Arc<AppState>>,
    Query(params): Query<DailyParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let days = params.days();
    let rows = state.kernel.agents.cost_daily(days)?;
    let items: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "date": r.date,
                "cost_usd": r.cost_usd,
                "tokens": r.tokens,
                "agent_count": r.agent_count,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({ "items": items })))
}

/// GET /api/costs/providers — account-level quota/balance for configured providers.
///
/// Fetches from each provider's API (where a key is available). This is the
/// "subscription quota" axis: credit balance, period spend, rate-limit windows.
pub(crate) async fn handle_cost_providers(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Resolve credentials for all known providers from config + credential store.
    let config_key = state.config.read().api_key();
    let mut credentials = std::collections::HashMap::new();
    for provider in crate::api::quota::all_fetchers().iter().map(|f| f.provider().to_string()) {
        if let Some((key, _)) = oxios_kernel::CredentialStore::resolve(&provider, config_key.as_deref())
        {
            credentials.insert(provider, key);
        }
    }

    let snapshots = crate::api::quota::fetch_all(&credentials).await;
    let items: Vec<serde_json::Value> = snapshots
        .iter()
        .map(|s| serde_json::to_value(s).unwrap_or_default())
        .collect();
    Ok(Json(serde_json::json!({ "providers": items })))
}

// ── Spend limit ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SpendLimitBody {
    pub monthly_limit_usd: Option<f64>,
}

/// GET /api/costs/spend-limit — current monthly spend limit + MTD spend.
pub(crate) async fn handle_cost_spend_limit_get(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let limit = state.config.read().budget.monthly_spend_limit_usd;

    let month_start = Utc::now()
        .date_naive()
        .with_day(1)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|t| t.and_utc());
    let mtd = state.kernel.agents.cost_summary(month_start)?;

    Ok(Json(serde_json::json!({
        "monthly_limit_usd": limit,
        "month_to_date_spend_usd": mtd.total_cost_usd,
        "month_to_date_tokens": mtd.total_tokens,
    })))
}

/// PUT /api/costs/spend-limit — set (or clear) the monthly spend limit.
///
/// Updates the in-memory config immediately and persists to disk so the
/// limit survives restarts. `null` clears the limit.
pub(crate) async fn handle_cost_spend_limit_set(
    state: State<Arc<AppState>>,
    Json(body): Json<SpendLimitBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Validate: negative limits make no sense.
    if let Some(v) = body.monthly_limit_usd
        && v < 0.0
    {
        return Err(AppError::BadRequest(
            "monthly_limit_usd must be non-negative".into(),
        ));
    }

    // Update in-memory config.
    {
        let mut cfg = state.config.write();
        cfg.budget.monthly_spend_limit_usd = body.monthly_limit_usd;
    }

    // Persist to disk.
    let cfg_snapshot = state.config.read().clone();
    let content = toml::to_string_pretty(&cfg_snapshot)
        .map_err(|e: toml::ser::Error| AppError::Internal(e.to_string()))?;
    tokio::fs::write(&state.config_path, content)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(
        limit = ?body.monthly_limit_usd,
        "Monthly spend limit updated"
    );

    Ok(Json(serde_json::json!({
        "ok": true,
        "monthly_limit_usd": body.monthly_limit_usd,
    })))
}
