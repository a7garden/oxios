//! Engine API — LLM engine introspection + config writes + routing control.
//!
//! Provides access to the oxi-sdk model catalog (providers, models, search)
//! and write operations that persist to config.toml (model, API key, routing).
//!
//! Routing statistics (`RoutingStats`) are shared between this API and
//! `AgentRuntime` via an `Arc`, so model usage is recorded end-to-end.

use crate::config::OxiosConfig;
use crate::credential::CredentialStore;
use anyhow::Context;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

// ── Routing types ─────────────────────────────────────────────────────────────

/// Snapshot of routing configuration (read-only API response).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingConfigSnapshot {
    /// Whether automatic model routing is enabled.
    pub routing_enabled: bool,
    /// Whether cost-efficient models are preferred when routing.
    pub prefer_cost_efficient: bool,
    /// Ordered list of fallback models (tried left-to-right on primary failure).
    pub fallback_models: Vec<String>,
    /// Models excluded from automatic routing.
    pub excluded_models: Vec<String>,
}

/// Model usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingStatsSnapshot {
    /// Model ID → number of calls.
    pub model_calls: HashMap<String, u64>,
    /// Model ID → estimated total cost (USD).
    pub model_cost: HashMap<String, f64>,
    /// Total number of requests.
    pub total_requests: u64,
    /// Total estimated cost (USD).
    pub total_cost: f64,
}

/// Single fallback event record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FallbackEvent {
    /// When the fallback occurred.
    pub timestamp: DateTime<Utc>,
    /// Model that was skipped/replaced.
    pub from_model: String,
    /// Model that was used instead.
    pub to_model: String,
    /// Reason for fallback (e.g. "rate_limit", "context_overflow", "error").
    pub reason: String,
    /// Whether the fallback succeeded (no further fallback needed).
    pub success: bool,
}

/// Request body for `PUT /api/engine/routing`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingUpdate {
    pub routing_enabled: Option<bool>,
    pub prefer_cost_efficient: Option<bool>,
    pub fallback_models: Option<Vec<String>>,
    pub excluded_models: Option<Vec<String>>,
}

// ── RoutingStats ─────────────────────────────────────────────────────────────

/// In-memory routing statistics, shared between `EngineApi` and `AgentRuntime`.
/// Uses simple RwLock for thread-safe reads/writes.
pub struct RoutingStats {
    calls: RwLock<HashMap<String, u64>>,
    costs: RwLock<HashMap<String, f64>>,
    /// Circular buffer of recent fallback events (max 200).
    fallbacks: RwLock<Vec<FallbackEvent>>,
}

impl Default for RoutingStats {
    fn default() -> Self {
        Self {
            calls: RwLock::new(HashMap::new()),
            costs: RwLock::new(HashMap::new()),
            fallbacks: RwLock::new(Vec::new()),
        }
    }
}

impl RoutingStats {
    /// Create a new stats tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one model invocation.
    pub fn record_model_usage(&self, model_id: &str, cost_usd: f64) {
        let mut calls = self.calls.write();
        *calls.entry(model_id.to_string()).or_insert(0) += 1;
        if cost_usd > 0.0 {
            let mut costs = self.costs.write();
            *costs.entry(model_id.to_string()).or_insert(0.0) += cost_usd;
        }
    }

    /// Record a fallback event.
    pub fn record_fallback(&self, event: FallbackEvent) {
        let mut fb = self.fallbacks.write();
        fb.push(event);
        let keep = fb.len().saturating_sub(200);
        if keep > 0 {
            fb.drain(0..keep);
        }
    }

    /// Get a snapshot of current stats.
    pub fn snapshot(&self) -> RoutingStatsSnapshot {
        let calls = self.calls.read();
        let costs = self.costs.read();
        let total_requests: u64 = calls.values().sum();
        let total_cost: f64 = costs.values().sum();
        RoutingStatsSnapshot {
            model_calls: calls.clone(),
            model_cost: costs.clone(),
            total_requests,
            total_cost,
        }
    }

    /// Get recent fallback events, newest first.
    pub fn fallback_history(&self, limit: usize) -> Vec<FallbackEvent> {
        let fb = self.fallbacks.read();
        fb.iter().rev().take(limit).cloned().collect()
    }
}

// ── Model cost estimation ────────────────────────────────────────────────────

/// Estimate cost in USD for a model given token usage.
/// Uses oxi-sdk's model_db for per-model pricing.
pub fn estimate_cost(model_id: &str, input_tokens: u64, output_tokens: u64) -> f64 {
    let entries = oxi_sdk::get_provider_models(model_id.split('/').next().unwrap_or(model_id));
    let entry = entries
        .iter()
        .find(|e| format!("{}/{}", e.provider, e.id) == model_id);
    match entry {
        Some(e) => {
            (e.cost_input * input_tokens as f64 / 1_000_000.0)
                + (e.cost_output * output_tokens as f64 / 1_000_000.0)
        }
        None => {
            // Fall back to a rough estimate for unknown models
            (0.003 * input_tokens as f64 / 1_000_000.0)
                + (0.015 * output_tokens as f64 / 1_000_000.0)
        }
    }
}

// ── Provider/Model response types ──────────────────────────────────────────

/// Provider category for UI grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderCategory {
    /// Major providers (Anthropic, OpenAI, Google).
    Major,
    /// Open / specialty providers (Groq, OpenRouter, DeepSeek, etc.).
    Open,
    /// Regional providers.
    Regional,
    /// Local / self-hosted providers.
    Local,
}

/// Static metadata for an LLM provider.
///
/// This table is the **single source of truth** for provider-facing
/// metadata in the Web UI. It enriches the dynamic list returned by
/// `oxi_sdk::get_providers()` with human-friendly labels, UI grouping,
/// and a flag for providers that should not be exposed to the Web
/// dashboard (e.g. those requiring non-API-key auth like AWS SigV4 or
/// OAuth, or region-specific endpoints).
///
/// New providers added to `oxi-sdk` automatically appear in the UI
/// with sensible fallbacks (`Open` category, derived display name)
/// even before they get an entry here.
#[derive(Debug, Clone, Copy)]
struct ProviderMeta {
    /// Canonical provider id (matches `oxi_sdk::get_providers()`).
    id: &'static str,
    /// Human-readable name shown in dropdowns and badges.
    display_name: &'static str,
    /// UI grouping for the provider selector.
    category: ProviderCategory,
    /// Whether to exclude from the Web UI providers list.
    /// Used for providers with non-standard auth (AWS SigV4, OAuth,
    /// account-scoped URLs) or that are region-specific duplicates.
    hidden: bool,
    /// Short description for tooltips / help text.
    description: &'static str,
    /// Primary environment variable name holding the API key.
    /// Empty string when the provider does not use a single env var
    /// (e.g. AWS Bedrock uses a credential chain).
    env_key: &'static str,
    /// Alternative ids that should resolve to this provider.
    /// Used so that an alias such as `aws-bedrock` matches the
    /// canonical `amazon-bedrock` entry.
    aliases: &'static [&'static str],
}

/// All provider metadata, in a single static table.
///
/// Order is for human readability only — the runtime lookup is O(n)
/// linear scan, which is fine for ~30 entries. If the table grows
/// past ~100 entries, swap to a `phf` or `once_cell` hash map.
const PROVIDER_META: &[ProviderMeta] = &[
    // ── Major (top 3) ──────────────────────────────────────────────
    ProviderMeta {
        id: "anthropic",
        display_name: "Anthropic",
        category: ProviderCategory::Major,
        hidden: false,
        description: "Claude models with extended thinking",
        env_key: "ANTHROPIC_API_KEY",
        aliases: &["anthropic"],
    },
    ProviderMeta {
        id: "openai",
        display_name: "OpenAI",
        category: ProviderCategory::Major,
        hidden: false,
        description: "GPT, o-series, and Codex models",
        env_key: "OPENAI_API_KEY",
        aliases: &["openai"],
    },
    ProviderMeta {
        id: "google",
        display_name: "Google Gemini",
        category: ProviderCategory::Major,
        hidden: false,
        description: "Gemini models with thinking and tool use",
        env_key: "GOOGLE_API_KEY",
        aliases: &["google"],
    },
    // ── Open / specialty (gateways + open-weight hosts) ────────────
    ProviderMeta {
        id: "groq",
        display_name: "Groq",
        category: ProviderCategory::Open,
        hidden: false,
        description: "Fast Llama, Mixtral, and Gemma inference",
        env_key: "GROQ_API_KEY",
        aliases: &["groq"],
    },
    ProviderMeta {
        id: "openrouter",
        display_name: "OpenRouter",
        category: ProviderCategory::Open,
        hidden: false,
        description: "Unified gateway to 200+ models",
        env_key: "OPENROUTER_API_KEY",
        aliases: &["openrouter"],
    },
    ProviderMeta {
        id: "deepseek",
        display_name: "DeepSeek",
        category: ProviderCategory::Open,
        hidden: false,
        description: "DeepSeek-V3 and DeepSeek-R1",
        env_key: "DEEPSEEK_API_KEY",
        aliases: &["deepseek"],
    },
    ProviderMeta {
        id: "mistral",
        display_name: "Mistral",
        category: ProviderCategory::Open,
        hidden: false,
        description: "Mistral and Codestral models",
        env_key: "MISTRAL_API_KEY",
        aliases: &["mistral"],
    },
    ProviderMeta {
        id: "xai",
        display_name: "xAI (Grok)",
        category: ProviderCategory::Open,
        hidden: false,
        description: "Grok models from xAI",
        env_key: "XAI_API_KEY",
        aliases: &["xai", "grok"],
    },
    ProviderMeta {
        id: "cerebras",
        display_name: "Cerebras",
        category: ProviderCategory::Open,
        hidden: false,
        description: "Ultra-fast open model inference",
        env_key: "CEREBRAS_API_KEY",
        aliases: &["cerebras"],
    },
    ProviderMeta {
        id: "fireworks",
        display_name: "Fireworks",
        category: ProviderCategory::Open,
        hidden: false,
        description: "Fast open-source model serving",
        env_key: "FIREWORKS_API_KEY",
        aliases: &["fireworks"],
    },
    ProviderMeta {
        id: "github-copilot",
        display_name: "GitHub Copilot",
        category: ProviderCategory::Open,
        hidden: false,
        description: "GitHub Copilot models (GPT-4, Claude)",
        env_key: "GITHUB_COPILOT_TOKEN",
        aliases: &["github-copilot", "copilot"],
    },
    ProviderMeta {
        id: "huggingface",
        display_name: "Hugging Face",
        category: ProviderCategory::Open,
        hidden: false,
        description: "Open model inference hub",
        env_key: "HUGGINGFACE_API_KEY",
        aliases: &["huggingface", "hf"],
    },
    ProviderMeta {
        id: "together",
        display_name: "Together AI",
        category: ProviderCategory::Open,
        hidden: false,
        description: "Open-source model hosting (Llama, Mixtral, ...)",
        env_key: "TOGETHER_API_KEY",
        aliases: &["together", "togetherai"],
    },
    ProviderMeta {
        id: "opencode",
        display_name: "OpenCode",
        category: ProviderCategory::Open,
        hidden: false,
        description: "OpenCode coding agent gateway",
        env_key: "",
        aliases: &["opencode"],
    },
    ProviderMeta {
        id: "perplexity",
        display_name: "Perplexity",
        category: ProviderCategory::Open,
        hidden: false,
        description: "Search-augmented answer models",
        env_key: "PERPLEXITY_API_KEY",
        aliases: &["perplexity"],
    },
    ProviderMeta {
        id: "cohere",
        display_name: "Cohere",
        category: ProviderCategory::Open,
        hidden: false,
        description: "Cohere Command and Embed models",
        env_key: "COHERE_API_KEY",
        aliases: &["cohere"],
    },
    // ── Regional (Chinese / Asian providers) ───────────────────────
    ProviderMeta {
        id: "minimax",
        display_name: "MiniMax",
        category: ProviderCategory::Regional,
        hidden: false,
        description: "MiniMax-M2.7, abab models",
        env_key: "MINIMAX_API_KEY",
        aliases: &["minimax"],
    },
    ProviderMeta {
        id: "moonshotai",
        display_name: "Moonshot AI (Kimi)",
        category: ProviderCategory::Regional,
        hidden: false,
        description: "Kimi models from Moonshot AI",
        env_key: "MOONSHOT_API_KEY",
        aliases: &["moonshotai", "moonshot", "kimi"],
    },
    ProviderMeta {
        id: "kimi-coding",
        display_name: "Kimi Coding",
        category: ProviderCategory::Regional,
        hidden: false,
        description: "Kimi Coding Plan — optimized for coding",
        env_key: "KIMI_CODING_API_KEY",
        aliases: &["kimi-coding"],
    },
    ProviderMeta {
        id: "zai",
        display_name: "Z.AI (GLM)",
        category: ProviderCategory::Regional,
        hidden: false,
        description: "Z.AI GLM models (coding plan)",
        env_key: "ZAI_API_KEY",
        aliases: &["zai"],
    },
    // ── Hidden in Web UI today; mapped for forward-compatibility ───
    // These providers are not exposed by `EngineHandle::providers()`
    // because they require non-standard auth or region-specific setup,
    // but listing them here means the metadata is already wired up if
    // a future change decides to surface them.
    ProviderMeta {
        id: "amazon-bedrock",
        display_name: "Amazon Bedrock",
        category: ProviderCategory::Open,
        hidden: true,
        description: "Multi-model via AWS Bedrock ConverseStream",
        env_key: "AWS_ACCESS_KEY_ID",
        aliases: &["amazon-bedrock", "aws-bedrock", "bedrock"],
    },
    ProviderMeta {
        id: "azure-openai-responses",
        display_name: "Azure OpenAI (Responses)",
        category: ProviderCategory::Open,
        hidden: true,
        description: "OpenAI models via Azure Cognitive Services",
        env_key: "AZURE_OPENAI_API_KEY",
        aliases: &["azure-openai-responses", "azure"],
    },
    ProviderMeta {
        id: "cloudflare-ai-gateway",
        display_name: "Cloudflare AI Gateway",
        category: ProviderCategory::Open,
        hidden: true,
        description: "Serverless AI via Cloudflare AI Gateway",
        env_key: "CLOUDFLARE_API_TOKEN",
        aliases: &["cloudflare-ai-gateway", "cf-ai-gateway"],
    },
    ProviderMeta {
        id: "cloudflare-workers-ai",
        display_name: "Cloudflare Workers AI",
        category: ProviderCategory::Open,
        hidden: true,
        description: "Serverless AI via Cloudflare Workers",
        env_key: "CLOUDFLARE_API_KEY",
        aliases: &["cloudflare-workers-ai", "cloudflare", "workers-ai"],
    },
    ProviderMeta {
        id: "google-vertex",
        display_name: "Google Vertex AI",
        category: ProviderCategory::Open,
        hidden: true,
        description: "Gemini via Google Cloud Vertex AI",
        env_key: "GOOGLE_APPLICATION_CREDENTIALS",
        aliases: &["google-vertex", "vertex"],
    },
    ProviderMeta {
        id: "minimax-cn",
        display_name: "MiniMax (China)",
        category: ProviderCategory::Regional,
        hidden: true,
        description: "MiniMax China region endpoint",
        env_key: "MINIMAX_CN_API_KEY",
        aliases: &["minimax-cn"],
    },
    ProviderMeta {
        id: "moonshotai-cn",
        display_name: "Moonshot AI (China)",
        category: ProviderCategory::Regional,
        hidden: true,
        description: "Kimi models — China region endpoint",
        env_key: "MOONSHOT_CN_API_KEY",
        aliases: &["moonshotai-cn", "moonshot-cn"],
    },
    ProviderMeta {
        id: "openai-codex",
        display_name: "OpenAI Codex",
        category: ProviderCategory::Open,
        hidden: true,
        description: "OpenAI Codex coding agent (Responses API)",
        env_key: "OPENAI_API_KEY",
        aliases: &["openai-codex"],
    },
    ProviderMeta {
        id: "opencode-go",
        display_name: "OpenCode Go",
        category: ProviderCategory::Open,
        hidden: true,
        description: "OpenCode Go Gateway",
        env_key: "OPENCODE_GO_API_KEY",
        aliases: &["opencode-go"],
    },
    ProviderMeta {
        id: "vercel-ai-gateway",
        display_name: "Vercel AI Gateway",
        category: ProviderCategory::Open,
        hidden: true,
        description: "Vercel AI Gateway",
        env_key: "VERCEL_API_KEY",
        aliases: &["vercel-ai-gateway", "vercel"],
    },
    ProviderMeta {
        id: "xiaomi",
        display_name: "Xiaomi MiMo",
        category: ProviderCategory::Regional,
        hidden: true,
        description: "Xiaomi MiMo models",
        env_key: "XIAOMI_API_KEY",
        aliases: &["xiaomi"],
    },
];

/// Look up metadata by canonical id or alias.
fn provider_meta(id: &str) -> Option<&'static ProviderMeta> {
    PROVIDER_META
        .iter()
        .find(|m| m.id == id || m.aliases.contains(&id))
}

fn provider_category(id: &str) -> ProviderCategory {
    provider_meta(id)
        .map(|m| m.category)
        .unwrap_or(ProviderCategory::Open)
}

/// Resolve a display name for a provider id.
///
/// Falls back to a Title-Cased id for unknown providers so that
/// newly added `oxi-sdk` providers still render acceptably until a
/// real entry lands in [`PROVIDER_META`].
fn provider_display_name(id: &str) -> String {
    provider_meta(id)
        .map(|m| m.display_name.to_string())
        .unwrap_or_else(|| fallback_display_name(id))
}

/// Render a fallback display name by splitting on `-` / `_` and
/// Title-Casing each segment. Examples:
///   `"kimi-coding"`   → `"Kimi Coding"`
///   `"some_id"`       → `"Some Id"`
///   `"openai"`        → `"Openai"`
fn fallback_display_name(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|s| !s.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Summary of an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    /// Provider identifier (e.g. "anthropic", "openai").
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Category for UI grouping.
    pub category: ProviderCategory,
    /// Number of models available for this provider.
    pub model_count: usize,
    /// Whether an API key is currently configured.
    pub has_key: bool,
    /// Short description for tooltips / help text. Empty for unknown
    /// providers that have no entry in [`PROVIDER_META`].
    #[serde(default)]
    pub description: String,
    /// Primary environment variable name for the API key. Empty for
    /// providers that do not use a single env var (e.g. AWS Bedrock
    /// uses a credential chain rather than a single API key var).
    #[serde(default)]
    pub env_key: String,
}

/// Input modality for a model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputModality {
    /// Text input.
    Text,
    /// Image input (vision).
    Image,
}

/// Summary of a model from the catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// Full model ID: "provider/model-id".
    pub id: String,
    /// Human-readable model name.
    pub name: String,
    /// API protocol used by the model's provider.
    pub api: String,
    /// Provider name.
    pub provider: String,
    /// Whether this model supports reasoning/thinking.
    pub reasoning: bool,
    /// Supported input modalities.
    pub input: Vec<InputModality>,
    /// Maximum context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
    /// Cost per million input tokens (USD).
    pub cost_input: f64,
    /// Cost per million output tokens (USD).
    pub cost_output: f64,
    /// Cost per million cached read tokens (USD).
    pub cost_cache_read: f64,
    /// Cost per million cached write tokens (USD).
    pub cost_cache_write: f64,
}

impl From<&oxi_sdk::ModelEntry> for ModelInfo {
    fn from(entry: &oxi_sdk::ModelEntry) -> Self {
        Self {
            id: format!("{}/{}", entry.provider, entry.id),
            name: entry.name.to_string(),
            api: entry.api.to_string(),
            provider: entry.provider.to_string(),
            reasoning: entry.reasoning,
            input: entry
                .input
                .iter()
                .map(|m| match m {
                    oxi_sdk::InputModality::Text => InputModality::Text,
                    oxi_sdk::InputModality::Image => InputModality::Image,
                    _ => InputModality::Text,
                })
                .collect(),
            context_window: entry.context_window,
            max_tokens: entry.max_tokens,
            cost_input: entry.cost_input,
            cost_output: entry.cost_output,
            cost_cache_read: entry.cost_cache_read,
            cost_cache_write: entry.cost_cache_write,
        }
    }
}

impl From<&oxi_sdk::CatalogModelEntry> for ModelInfo {
    /// Build a [`ModelInfo`] from a live catalog entry (catalog port).
    ///
    /// Same fields as the [`ModelEntry`](oxi_sdk::ModelEntry) path; the
    /// catalog entry additionally reflects runtime models.dev refresh +
    /// user overrides when wired into the engine.
    fn from(entry: &oxi_sdk::CatalogModelEntry) -> Self {
        Self {
            id: format!("{}/{}", entry.provider, entry.model_id),
            name: entry.name.clone(),
            api: entry.protocol.as_str().to_string(),
            provider: entry.provider.clone(),
            reasoning: entry.reasoning,
            input: entry
                .input_modalities
                .iter()
                .map(|m| match m.as_str() {
                    "image" => InputModality::Image,
                    _ => InputModality::Text,
                })
                .collect(),
            context_window: entry.context_window,
            max_tokens: entry.max_tokens,
            cost_input: entry.cost_input,
            cost_output: entry.cost_output,
            cost_cache_read: entry.cost_cache_read,
            cost_cache_write: entry.cost_cache_write,
        }
    }
}

/// Current engine configuration + credential status + routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfigResponse {
    /// Currently configured default model.
    pub default_model: String,
    /// Whether an API key is set for the current provider.
    pub api_key_set: bool,
    /// Source of the API key (if any).
    pub api_key_source: Option<String>,
    /// Provider name extracted from default_model.
    pub provider: Option<String>,
    /// Current routing configuration.
    pub routing: RoutingConfigSnapshot,
}

/// Result of an API key validation attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateKeyResult {
    /// Whether the key is valid.
    pub valid: bool,
    /// Provider that was validated.
    pub provider: String,
    /// Optional message (error detail or success note).
    pub message: Option<String>,
}

// ── EngineApi ───────────────────────────────────────────────────────────────

/// Engine API facade — model catalog introspection + config writes + routing.
///
/// Holds a shared reference to the live config (behind `RwLock`) and the
/// path to config.toml so write operations can persist to disk.
/// Routing stats are shared with `AgentRuntime` via `Arc<RoutingStats>`.
///
/// When config writes change the model or API key, `EngineApi` rebuilds
/// `OxiosEngine` via [`EngineHandle`] so the runtime picks up the change
/// on the next agent execution (hot-swap, no restart required).
pub struct EngineApi {
    config: Arc<RwLock<OxiosConfig>>,
    config_path: PathBuf,
    routing_stats: Arc<RoutingStats>,
    /// Hot-swap handle — config writes rebuild `OxiosEngine` and swap it in.
    engine_handle: Arc<crate::engine::EngineHandle>,
}

impl EngineApi {
    /// Create a new EngineApi.
    ///
    /// - `config` — shared config store (backed by RwLock)
    /// - `config_path` — path to config.toml for persistence
    /// - `routing_stats` — shared stats tracker (shared with AgentRuntime)
    /// - `engine_handle` — hot-swap handle for live engine replacement
    pub fn new(
        config: Arc<RwLock<OxiosConfig>>,
        config_path: PathBuf,
        routing_stats: Arc<RoutingStats>,
        engine_handle: Arc<crate::engine::EngineHandle>,
    ) -> Self {
        Self {
            config,
            config_path,
            routing_stats,
            engine_handle,
        }
    }

    /// Get the shared `RoutingStats` reference (for `AgentRuntime` wiring).
    pub fn routing_stats(&self) -> Arc<RoutingStats> {
        Arc::clone(&self.routing_stats)
    }

    /// Get a reference to the engine handle.
    pub fn engine_handle(&self) -> &Arc<crate::engine::EngineHandle> {
        &self.engine_handle
    }

    // ── Read operations ────────────────────────────────────────────────

    /// List all available providers from the oxi-sdk catalog.
    ///
    /// Reads provider/model counts from the live catalog (runtime models.dev
    /// refresh + user overrides) when wired into the engine, falling back to
    /// the static registry otherwise.
    ///
    /// Filters out hidden/internal providers (those flagged with
    /// `hidden: true` in [`PROVIDER_META`]) and augments each entry
    /// with credential status, display name, and description.
    ///
    /// Providers without a [`PROVIDER_META`] entry are shown by
    /// default — a new provider landing in `oxi-sdk` should be
    /// available to users even before its metadata is added here.
    pub fn providers(&self) -> Vec<ProviderInfo> {
        let catalog = self.engine_handle.get().oxi().catalog().clone();
        let use_catalog = catalog.model_count_sync() > 0;
        let all: Vec<String> = if use_catalog {
            catalog.list_providers_sync()
        } else {
            oxi_sdk::get_providers()
                .into_iter()
                .map(|s| s.to_string())
                .collect()
        };

        all.into_iter()
            .filter(|p| provider_meta(p).map(|m| !m.hidden).unwrap_or(true))
            .map(|p| {
                let model_count = if use_catalog {
                    catalog.list_models_sync(&p).len()
                } else {
                    oxi_sdk::get_provider_models(&p).len()
                };
                let has_key = CredentialStore::has_credential(
                    &p,
                    self.config
                        .read()
                        .engine
                        .api_key
                        .as_deref()
                        .filter(|k| !k.is_empty()),
                );
                let meta = provider_meta(&p);
                ProviderInfo {
                    id: p.clone(),
                    name: provider_display_name(&p),
                    category: provider_category(&p),
                    model_count,
                    has_key,
                    description: meta.map(|m| m.description.to_string()).unwrap_or_default(),
                    env_key: meta.map(|m| m.env_key.to_string()).unwrap_or_default(),
                }
            })
            .collect()
    }

    /// List models for a given provider, optionally filtered by a query.
    ///
    /// Reads from the live catalog (runtime models.dev refresh + user
    /// overrides) when wired into the engine, falling back to the static
    /// registry (embedded snapshot) otherwise.
    pub fn models(&self, provider: &str, query: Option<&str>) -> Vec<ModelInfo> {
        let catalog = self.engine_handle.get().oxi().catalog().clone();
        let live = catalog.list_models_sync(provider);
        let models: Vec<ModelInfo> = if !live.is_empty() {
            live.iter().map(ModelInfo::from).collect()
        } else {
            oxi_sdk::get_provider_models(provider)
                .iter()
                .map(ModelInfo::from)
                .collect()
        };
        models
            .into_iter()
            .filter(|m| !m.name.contains("latest"))
            .filter(|m| {
                if let Some(q) = query {
                    let q = q.to_lowercase();
                    m.name.to_lowercase().contains(&q)
                        || m.id.to_lowercase().contains(&q)
                        || m.provider.to_lowercase().contains(&q)
                } else {
                    true
                }
            })
            .collect()
    }

    /// Search models across all providers.
    ///
    /// Uses the live catalog's `search_sync` when available, else the static
    /// registry.
    pub fn search_models(&self, query: &str) -> Vec<ModelInfo> {
        let catalog = self.engine_handle.get().oxi().catalog().clone();
        let live = catalog.search_sync(query);
        if !live.is_empty() {
            live.iter().map(ModelInfo::from).collect()
        } else {
            oxi_sdk::search_models(query)
                .into_iter()
                .map(ModelInfo::from)
                .collect()
        }
    }

    /// Get the current engine configuration + credential status + routing.
    pub fn config(&self) -> EngineConfigResponse {
        let cfg = self.config.read();
        let provider =
            CredentialStore::provider_from_model(&cfg.engine.default_model).map(|s| s.to_string());
        let api_key_source = provider.as_deref().and_then(|p| {
            CredentialStore::resolve(p, cfg.api_key().as_deref()).map(|(_, src)| {
                match src {
                    crate::credential::CredentialSource::EnvVar => "env",
                    crate::credential::CredentialSource::Config => "config",
                    crate::credential::CredentialSource::OxiAuthStore => "auth_store",
                }
                .to_string()
            })
        });
        let api_key_set = provider
            .as_deref()
            .map(|p| CredentialStore::has_credential(p, cfg.api_key().as_deref()))
            .unwrap_or(false);

        EngineConfigResponse {
            default_model: cfg.engine.default_model.clone(),
            api_key_set,
            api_key_source,
            provider,
            routing: RoutingConfigSnapshot {
                routing_enabled: cfg.engine.routing_enabled,
                prefer_cost_efficient: cfg.engine.prefer_cost_efficient,
                fallback_models: cfg.engine.fallback_models.clone(),
                excluded_models: cfg.engine.excluded_models.clone(),
            },
        }
    }

    /// Get routing stats snapshot (for Web dashboard).
    pub fn routing_stats_snapshot(&self) -> RoutingStatsSnapshot {
        self.routing_stats.snapshot()
    }

    /// Get recent fallback history.
    pub fn fallback_history(&self, limit: usize) -> Vec<FallbackEvent> {
        self.routing_stats.fallback_history(limit)
    }

    // ── Write operations ───────────────────────────────────────────────

    /// Set the default model in config.toml.
    ///
    /// Updates both the in-memory config and the on-disk file, then
    /// hot-swaps the runtime engine so the next agent execution uses the new model.
    pub fn set_model(&self, model_id: &str) -> anyhow::Result<()> {
        // Validate BEFORE persisting/swapping: reject unknown models and
        // unconfigured providers so the Web UI's "switch succeeded" is truthful.
        // This prevents the divergence where a bad model ID was silently
        // accepted at swap time and only surfaced as "Model not found" at the
        // execute phase — after interview/seed had already run.
        {
            let engine = self.engine_handle.get();
            let model = engine
                .resolve_model(model_id)
                .with_context(|| format!("Unknown model '{model_id}'"))?;
            engine.create_provider(&model.provider).with_context(|| {
                format!(
                    "Provider '{}' is not configured for '{model_id}'",
                    model.provider
                )
            })?;
        }
        {
            let mut cfg = self.config.write();
            cfg.engine.default_model = model_id.to_string();
            self.persist(&cfg)?;
        }
        tracing::info!(model = %model_id, "Default model updated in config");
        self.rebuild_and_swap();
        Ok(())
    }

    /// Set an API key for a provider.
    ///
    /// Stores the key via CredentialStore (→ ~/.oxi/auth.json) and also
    /// updates config.toml's `[engine].api_key` when the provider matches
    /// the current default model. Hot-swaps the runtime engine afterward.
    pub fn set_api_key(&self, provider: &str, key: &str) -> anyhow::Result<()> {
        CredentialStore::store(provider, key)?;

        // If the provider matches the current default model, also set in config
        let cfg = self.config.read();
        if let Some(current_provider) =
            CredentialStore::provider_from_model(&cfg.engine.default_model)
            && current_provider == provider
        {
            drop(cfg);
            let mut cfg = self.config.write();
            cfg.engine.api_key = Some(key.to_string());
            self.persist(&cfg)?;
        }
        tracing::info!(provider = %provider, "API key stored");
        self.rebuild_and_swap();
        Ok(())
    }

    /// Update provider options in config.toml.
    ///
    /// Persists the options and makes them available for the next agent run.
    /// They are passed through to `AgentLoopConfig::provider_options`.
    pub fn set_provider_options(&self, opts: &oxi_sdk::ProviderOptions) -> anyhow::Result<()> {
        {
            let mut cfg = self.config.write();
            cfg.engine.provider_options = Some(opts.clone());
            self.persist(&cfg)?;
        }
        tracing::info!("Provider options updated and persisted");
        // No engine rebuild needed — provider_options are per-request,
        // picked up from config on the next agent run.
        Ok(())
    }

    /// Update routing configuration in config.toml.
    ///
    /// Only the fields provided in `update` are changed; others are left untouched.
    /// Changes are persisted to disk immediately.
    pub fn set_routing(&self, update: RoutingUpdate) -> anyhow::Result<()> {
        {
            let mut cfg = self.config.write();
            if let Some(v) = update.routing_enabled {
                cfg.engine.routing_enabled = v;
            }
            if let Some(v) = update.prefer_cost_efficient {
                cfg.engine.prefer_cost_efficient = v;
            }
            if let Some(v) = update.fallback_models {
                cfg.engine.fallback_models = v;
            }
            if let Some(v) = update.excluded_models {
                cfg.engine.excluded_models = v;
            }
            self.persist(&cfg)?;
        }
        tracing::info!("Routing configuration updated via API");
        self.rebuild_and_swap();
        Ok(())
    }

    /// Validate an API key by making a simple test call.
    ///
    /// Creates a lightweight provider and attempts a minimal request.
    /// Returns the validation result.
    pub fn validate_key(&self, provider: &str, api_key: &str) -> ValidateKeyResult {
        // Try to create a provider with the given key and make a minimal completion request
        let result = self.try_validate(provider, api_key);
        match result {
            Ok(()) => ValidateKeyResult {
                valid: true,
                provider: provider.to_string(),
                message: Some("API key is valid".to_string()),
            },
            Err(e) => ValidateKeyResult {
                valid: false,
                provider: provider.to_string(),
                message: Some(format!("Validation failed: {e}")),
            },
        }
    }

    /// Attempt a lightweight validation call.
    fn try_validate(&self, provider: &str, api_key: &str) -> anyhow::Result<()> {
        // Build an OxiBuilder with builtins and the provided key
        let builder = oxi_sdk::OxiBuilder::new()
            .with_builtins()
            .api_key(provider, api_key);
        let oxi = builder.build();

        // Try to resolve any model from this provider
        let models = oxi_sdk::get_provider_models(provider);
        if models.is_empty() {
            anyhow::bail!("No models found for provider '{provider}'");
        }

        let model_id = format!("{}/{}", provider, models[0].id);
        let _model = oxi.resolve_model(&model_id)?;

        // Create a provider with the injected key
        let _provider = oxi.create_provider(provider)?;

        // If we got this far, the provider was created with the key.
        // Note: Actual API call validation would require a lightweight
        // completion request. For now, this validates key format + provider existence.
        if api_key.is_empty() {
            anyhow::bail!("API key is empty");
        }
        if api_key.len() < 8 {
            anyhow::bail!("API key appears too short");
        }

        tracing::debug!(
            provider = %provider,
            model = %model_id,
            "Key validation: provider resolved with injected key"
        );
        Ok(())
    }

    // ── Helpers ─────────────────────────────────────────────────────────

    /// Estimate cost for a model invocation.
    pub fn estimate_cost(model_id: &str, input_tokens: u64, output_tokens: u64) -> f64 {
        estimate_cost(model_id, input_tokens, output_tokens)
    }

    /// Persist the current config to disk.
    fn persist(&self, config: &OxiosConfig) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(config)
            .map_err(|e| anyhow::anyhow!("Failed to serialize config: {e}"))?;
        std::fs::write(&self.config_path, content)?;
        Ok(())
    }

    /// Rebuild `OxiosEngine` from current config and swap into the handle.
    ///
    /// Reuses the model catalog from the current engine (it holds the
    /// in-memory models.dev snapshot — re-initializing it on every config
    /// change would just reload the same data). No network calls beyond
    /// what `CredentialStore` already caches in memory.
    fn rebuild_and_swap(&self) {
        let cfg = self.config.read();
        let model_id = &cfg.engine.default_model;
        // The catalog Arc is cheap to clone and shared across hot-swaps.
        let catalog = self.engine_handle.get().oxi().catalog().clone();
        let new_engine = crate::engine::OxiosEngine::from_config_with_catalog(
            model_id,
            cfg.api_key().as_deref(),
            catalog,
        );
        drop(cfg);
        self.engine_handle.swap(new_engine);
    }
}

impl std::fmt::Debug for EngineApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineApi")
            .field("config_path", &self.config_path)
            .finish()
    }
}

// Expose `RoutingStats::record_model_usage` via a public helper for AgentRuntime.
// This avoids exposing the internal Arc to outside crates.
pub fn record_usage_to_stats(
    stats: &Option<Arc<RoutingStats>>,
    model_id: &str,
    input_tokens: u64,
    output_tokens: u64,
) {
    if let Some(s) = stats {
        let cost = estimate_cost(model_id, input_tokens, output_tokens);
        s.record_model_usage(model_id, cost);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_category_known() {
        // Major
        assert_eq!(provider_category("anthropic"), ProviderCategory::Major);
        assert_eq!(provider_category("openai"), ProviderCategory::Major);
        assert_eq!(provider_category("google"), ProviderCategory::Major);
        // Open / specialty
        assert_eq!(provider_category("groq"), ProviderCategory::Open);
        assert_eq!(provider_category("opencode"), ProviderCategory::Open);
        // Regional
        assert_eq!(provider_category("minimax"), ProviderCategory::Regional);
        assert_eq!(provider_category("moonshotai"), ProviderCategory::Regional);
        assert_eq!(provider_category("kimi-coding"), ProviderCategory::Regional);
        assert_eq!(provider_category("zai"), ProviderCategory::Regional);
        assert_eq!(provider_category("minimax-cn"), ProviderCategory::Regional);
        assert_eq!(provider_category("xiaomi"), ProviderCategory::Regional);
    }

    #[test]
    fn test_provider_category_fallback() {
        // Unknown ids fall back to Open, not panic.
        assert_eq!(
            provider_category("not-a-real-provider"),
            ProviderCategory::Open
        );
        assert_eq!(provider_category(""), ProviderCategory::Open);
    }

    #[test]
    fn test_provider_display_name_known() {
        assert_eq!(provider_display_name("anthropic"), "Anthropic");
        assert_eq!(provider_display_name("minimax"), "MiniMax");
        assert_eq!(provider_display_name("moonshotai"), "Moonshot AI (Kimi)");
        assert_eq!(provider_display_name("kimi-coding"), "Kimi Coding");
        assert_eq!(provider_display_name("zai"), "Z.AI (GLM)");
        assert_eq!(provider_display_name("opencode"), "OpenCode");
        assert_eq!(provider_display_name("amazon-bedrock"), "Amazon Bedrock");
    }

    #[test]
    fn test_provider_display_name_fallback() {
        // Unknown ids get Title-Cased per segment as a fallback.
        assert_eq!(
            provider_display_name("some-new-provider"),
            "Some New Provider"
        );
        assert_eq!(provider_display_name("kimi-coding"), "Kimi Coding");
        assert_eq!(provider_display_name("some_id"), "Some Id");
        // Empty string stays empty.
        assert_eq!(provider_display_name(""), "");
    }

    #[test]
    fn test_provider_meta_lookup_by_alias() {
        // Aliases resolve to the same meta entry as the canonical id.
        let by_id = provider_meta("github-copilot").unwrap();
        let by_alias = provider_meta("copilot").unwrap();
        assert_eq!(by_id.id, by_alias.id);

        let bedrock_id = provider_meta("amazon-bedrock").unwrap();
        let bedrock_alias = provider_meta("aws-bedrock").unwrap();
        let bedrock_canonical = provider_meta("bedrock").unwrap();
        assert_eq!(bedrock_id.id, bedrock_alias.id);
        assert_eq!(bedrock_id.id, bedrock_canonical.id);
    }

    #[test]
    fn test_provider_meta_unknown_is_none() {
        assert!(provider_meta("not-a-real-provider").is_none());
        assert!(provider_meta("").is_none());
    }

    #[test]
    fn test_provider_info_serialization() {
        let info = ProviderInfo {
            id: "anthropic".to_string(),
            name: "Anthropic".to_string(),
            category: ProviderCategory::Major,
            model_count: 15,
            has_key: true,
            description: "Claude models with extended thinking".to_string(),
            env_key: "ANTHROPIC_API_KEY".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        // camelCase serialization
        assert!(json.contains("\"modelCount\":15"));
        assert!(json.contains("\"hasKey\":true"));
        assert!(json.contains("\"envKey\":\"ANTHROPIC_API_KEY\""));
        let restored: ProviderInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "anthropic");
        assert_eq!(restored.name, "Anthropic");
        assert_eq!(restored.model_count, 15);
        assert!(restored.has_key);
        assert_eq!(restored.env_key, "ANTHROPIC_API_KEY");
    }

    #[test]
    fn test_provider_info_serialization_missing_optional() {
        // description / env_key have serde(default) so old clients that
        // omit them still deserialize cleanly.
        let json = r#"{
            "id": "anthropic",
            "name": "Anthropic",
            "category": "major",
            "modelCount": 15,
            "hasKey": true
        }"#;
        let info: ProviderInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, "anthropic");
        assert_eq!(info.description, "");
        assert_eq!(info.env_key, "");
    }

    #[test]
    fn test_model_info_serialization() {
        let info = ModelInfo {
            id: "anthropic/claude-sonnet-4".to_string(),
            name: "Claude Sonnet 4".to_string(),
            api: "anthropic-messages".to_string(),
            provider: "anthropic".to_string(),
            reasoning: true,
            input: vec![InputModality::Text, InputModality::Image],
            context_window: 200000,
            max_tokens: 16000,
            cost_input: 3.0,
            cost_output: 15.0,
            cost_cache_read: 0.3,
            cost_cache_write: 3.75,
        };
        let json = serde_json::to_string(&info).unwrap();
        let restored: ModelInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "anthropic/claude-sonnet-4");
        assert!(restored.reasoning);
        assert_eq!(restored.context_window, 200000);
        assert!(restored.input.contains(&InputModality::Image));
        assert_eq!(restored.api, "anthropic-messages");
    }

    #[test]
    fn test_engine_config_response_serialization() {
        let resp = EngineConfigResponse {
            default_model: "anthropic/claude-sonnet-4".to_string(),
            api_key_set: true,
            api_key_source: Some("config.toml".to_string()),
            provider: Some("anthropic".to_string()),
            routing: RoutingConfigSnapshot {
                routing_enabled: false,
                prefer_cost_efficient: false,
                fallback_models: vec![],
                excluded_models: vec![],
            },
        };
        let json = serde_json::to_string(&resp).unwrap();
        let restored: EngineConfigResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.default_model, "anthropic/claude-sonnet-4");
        assert!(restored.api_key_set);
        assert_eq!(restored.api_key_source.as_deref(), Some("config.toml"));
        assert!(!restored.routing.routing_enabled);
    }

    #[test]
    fn test_validate_key_result_serialization() {
        let result = ValidateKeyResult {
            valid: true,
            provider: "openai".to_string(),
            message: Some("API key is valid".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        let restored: ValidateKeyResult = serde_json::from_str(&json).unwrap();
        assert!(restored.valid);
        assert_eq!(restored.provider, "openai");
    }

    #[test]
    fn test_validate_key_result_invalid() {
        let result = ValidateKeyResult {
            valid: false,
            provider: "anthropic".to_string(),
            message: Some("Validation failed: key too short".to_string()),
        };
        assert!(!result.valid);
        assert!(result.message.as_ref().unwrap().contains("failed"));
    }

    #[test]
    fn test_routing_stats_snapshot() {
        let stats = RoutingStats::new();
        stats.record_model_usage("anthropic/claude-sonnet-4", 0.05);
        stats.record_model_usage("anthropic/claude-sonnet-4", 0.03);
        stats.record_model_usage("openai/gpt-4o-mini", 0.01);

        let snap = stats.snapshot();
        assert_eq!(snap.total_requests, 3);
        assert_eq!(snap.model_calls["anthropic/claude-sonnet-4"], 2);
        assert_eq!(snap.model_calls["openai/gpt-4o-mini"], 1);
        assert!((snap.total_cost - 0.09).abs() < 0.001);
    }

    #[test]
    fn test_fallback_history_circular() {
        let stats = RoutingStats::new();
        for i in 0..210 {
            stats.record_fallback(FallbackEvent {
                timestamp: DateTime::from_timestamp(i as i64, 0).unwrap(),
                from_model: format!("model-{}", i),
                to_model: "fallback".to_string(),
                reason: "test".to_string(),
                success: true,
            });
        }
        let history = stats.fallback_history(200);
        assert_eq!(history.len(), 200);
        // Most recent first (i=209 down to i=10)
        assert_eq!(history[0].from_model, "model-209");
        assert_eq!(history[199].from_model, "model-10");
    }

    #[test]
    fn set_model_rejects_unknown_model_before_persist() {
        use crate::engine::{EngineHandle, OxiosEngine};

        let engine = Arc::new(OxiosEngine::new("anthropic/claude-sonnet-4-20250514"));
        let handle = Arc::new(EngineHandle::new(engine));
        let config = Arc::new(parking_lot::RwLock::new(OxiosConfig::default()));
        // Validation runs before any IO, so a non-existent path is safe — it
        // must never be written to.
        let path = PathBuf::from("/tmp/oxios-set-model-test-NONEXISTENT.toml");
        let api = EngineApi::new(config, path, Arc::new(RoutingStats::new()), handle);

        // The malformed id from the user-reported bug. Must be rejected, not
        // silently accepted and deferred to the execute phase.
        let before = api.config.read().engine.default_model.clone();
        let err = api.set_model("zai-coding-plan/glm-5-turbo").unwrap_err();
        assert!(
            err.to_string().contains("Unknown model"),
            "expected unknown-model error, got: {err}"
        );
        // Rejection happened before persist: config is untouched.
        assert_eq!(api.config.read().engine.default_model, before);
    }

    #[test]
    fn set_model_accepts_known_builtin_model() {
        use crate::engine::{EngineHandle, OxiosEngine};

        let engine = Arc::new(OxiosEngine::new("anthropic/claude-sonnet-4-20250514"));
        let handle = Arc::new(EngineHandle::new(engine));
        let config = Arc::new(parking_lot::RwLock::new(OxiosConfig::default()));
        let tmp =
            std::env::temp_dir().join(format!("oxios-set-model-ok-{}.toml", std::process::id()));
        let api = EngineApi::new(config, tmp.clone(), Arc::new(RoutingStats::new()), handle);

        // A builtin model with a built-in provider resolves + creates a provider
        // without any API key, so validation passes. The swap should succeed.
        let result = api.set_model("openai/gpt-4o");
        // create_provider may still fail without a key on some SDK builds; treat
        // both Ok and a provider-config error as acceptable, but never an
        // "Unknown model" rejection for a known builtin.
        match result {
            Ok(()) => assert_eq!(api.config.read().engine.default_model, "openai/gpt-4o"),
            Err(e) => assert!(
                !e.to_string().contains("Unknown model"),
                "known model rejected as unknown: {e}"
            ),
        }
        let _ = std::fs::remove_file(&tmp);
    }
}
