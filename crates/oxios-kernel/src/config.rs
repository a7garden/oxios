//! Configuration loading from TOML files.
//!
//! Configuration is stored at `~/.oxios/config.toml` and controls
//! kernel, gateway, and execution settings.

use cron::Schedule;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::scheduler::Priority;

/// Cron scheduler configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CronConfig {
    /// Enable the cron scheduler.
    #[serde(default)]
    pub enabled: bool,
    /// Tick interval in seconds.
    #[serde(default = "default_tick_interval")]
    pub tick_interval_secs: u64,
    /// Inline job definitions from config.toml.
    #[serde(default)]
    pub jobs: std::collections::HashMap<String, InlineCronJob>,
}

impl Default for CronConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tick_interval_secs: default_tick_interval(),
            jobs: std::collections::HashMap::new(),
        }
    }
}

fn default_tick_interval() -> u64 {
    60
}

/// Inline cron job definition in config.toml.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InlineCronJob {
    /// Cron expression (e.g. "0 */6 * * *").
    pub schedule: String,
    /// Goal description for the agent.
    pub goal: String,
    /// Constraints on agent behavior.
    #[serde(default)]
    pub constraints: Vec<String>,
    /// Criteria that must be met for the job to be considered successful.
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    /// Toolchain preset name.
    #[serde(default = "default_toolchain_inline")]
    pub toolchain: String,
    /// Job priority.
    #[serde(default)]
    pub priority: Priority,
    /// Whether the job is active.
    #[serde(default = "default_true_inline")]
    pub enabled: bool,
}

fn default_toolchain_inline() -> String {
    "default".into()
}

fn default_true_inline() -> bool {
    true
}

/// Memory system configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Enable the memory system.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum memories returned by recall.
    #[serde(default = "default_max_recall")]
    pub max_recall: usize,
    /// Auto-summarize sessions on completion.
    #[serde(default = "default_true")]
    pub auto_summarize: bool,
    /// Capture compaction summaries as conversation memory.
    #[serde(default = "default_true")]
    pub capture_compaction: bool,
    /// Memory retention in days (0 = unlimited).
    #[serde(default)]
    pub retention_days: u32,
    /// Enable embedding cache.
    #[serde(default = "default_true")]
    pub cache_enabled: bool,
    /// Embedding cache TTL in seconds.
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_secs: u64,
    /// Maximum embedding cache entries.
    #[serde(default = "default_cache_max_entries")]
    pub cache_max_entries: usize,
    /// Consolidation configuration (RFC-008).
    #[serde(default)]
    pub consolidation: ConsolidationConfig,
    /// SQLite memory storage configuration (RFC-012).
    #[serde(default)]
    pub sqlite: SqliteMemoryConfig,
    /// Embedding provider configuration (RFC-012).
    #[serde(default)]
    pub embedding: EmbeddingConfig,
}

fn default_true() -> bool {
    true
}

fn default_max_recall() -> usize {
    10
}

fn default_cache_ttl() -> u64 {
    3600 // 1 hour
}

fn default_cache_max_entries() -> usize {
    10000
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_recall: 10,
            auto_summarize: true,
            capture_compaction: true,
            retention_days: 0,
            cache_enabled: true,
            cache_ttl_secs: 3600,
            cache_max_entries: 10000,
            consolidation: ConsolidationConfig::default(),
            sqlite: SqliteMemoryConfig::default(),
            embedding: EmbeddingConfig::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// SqliteMemoryConfig (RFC-012: SQLite Memory Storage)
// ---------------------------------------------------------------------------

/// SQLite-backed memory storage configuration (RFC-012).
///
/// When enabled, memories are stored in a single `memory.db` file with
/// FTS5 BM25 + sqlite-vec KNN search. Falls back to the existing JSON
/// + TF-IDF approach when disabled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteMemoryConfig {
    /// Enable SQLite-backed memory storage.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Path to the SQLite database file.
    /// Empty string means default: `~/.oxios/workspace/memory.db`
    #[serde(default)]
    pub path: String,
    /// Embedding vector dimension.
    /// Controls the `vec0` virtual table dimension.
    /// Common values: 128 (fast), 256 (balanced), 768 (full Gemma).
    #[serde(default = "default_embedding_dim")]
    pub embedding_dim: usize,
    /// Enable WAL mode for concurrent reads.
    #[serde(default = "default_true")]
    pub wal_mode: bool,
}

fn default_embedding_dim() -> usize {
    256
}

impl Default for SqliteMemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: String::new(),
            embedding_dim: 256,
            wal_mode: true,
        }
    }
}

// ---------------------------------------------------------------------------
// EmbeddingConfig (RFC-012: Embedding Provider)
// ---------------------------------------------------------------------------

/// Embedding provider configuration (RFC-012).
///
/// Controls which embedding model is used for semantic search.
/// When `embedding-mlx` feature is enabled and `provider = "mlx"`,
/// uses EmbeddingGemma-300m via MLX on Apple Silicon.
/// Otherwise falls back to TF-IDF.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Embedding provider: "tfidf" (default) or "mlx" (Apple Silicon).
    #[serde(default = "default_embedding_provider")]
    pub provider: String,
    /// Matryoshka dimension: 128, 256, 512, or 768.
    /// Only used when provider = "mlx".
    #[serde(default = "default_embedding_dim")]
    pub dimension: usize,
    /// Model TTL in seconds. Unloaded after this duration of inactivity.
    /// Only used when provider = "mlx".
    #[serde(default = "default_model_ttl")]
    pub model_ttl_secs: u64,
}

fn default_embedding_provider() -> String {
    "gguf".to_string()
}

fn default_model_ttl() -> u64 {
    300 // 5 minutes
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: default_embedding_provider(),
            dimension: default_embedding_dim(),
            model_ttl_secs: default_model_ttl(),
        }
    }
}

// ---------------------------------------------------------------------------
// ConsolidationConfig (RFC-008: Memory Consolidation)
// ---------------------------------------------------------------------------

/// Memory consolidation configuration (RFC-008).
/// All values have sensible defaults — users never need to configure these.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationConfig {
    // ── Dream Process ─────────────────────────────────
    #[serde(default = "default_true")]
    pub dream_enabled: bool,
    #[serde(default = "default_dream_interval")]
    pub dream_interval_hours: u64,
    #[serde(default = "default_dream_min_sessions")]
    pub dream_min_sessions: u32,

    // ── Tier Budgets ──────────────────────────────────
    #[serde(default = "default_hot_max")]
    pub hot_max_entries: usize,
    #[serde(default = "default_warm_max")]
    pub warm_max_entries: usize,
    #[serde(default = "default_cold_max")]
    pub cold_max_entries: usize,
    #[serde(default = "default_hot_token_budget")]
    pub hot_token_budget: usize,

    // ── Decay ─────────────────────────────────────────
    #[serde(default = "default_true")]
    pub decay_enabled: bool,
    #[serde(default = "default_one")]
    pub decay_multiplier: f32,
    #[serde(default = "default_decay_threshold")]
    pub decay_threshold: f32,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,

    // ── Auto-Protection ───────────────────────────────
    #[serde(default = "default_true")]
    pub auto_protection: bool,
    #[serde(default = "default_protection_low_access")]
    pub protection_low_access: u32,
    #[serde(default = "default_protection_medium_access")]
    pub protection_medium_access: u32,
    #[serde(default = "default_protection_high_access")]
    pub protection_high_access: u32,
    #[serde(default = "default_protection_medium_sessions")]
    pub protection_medium_sessions: u32,
    #[serde(default = "default_protection_high_sessions")]
    pub protection_high_sessions: u32,

    // ── Auto-Classification ───────────────────────────
    #[serde(default = "default_true")]
    pub auto_classification: bool,
    #[serde(default = "default_type_promotion_threshold")]
    pub type_promotion_repetitions: u32,

    // ── Compaction ────────────────────────────────────
    #[serde(default = "default_compaction_threshold")]
    pub compaction_line_threshold: usize,
    #[serde(default = "default_true")]
    pub llm_compaction: bool,

    // ── Dream LLM ──────────────────────────────────────
    /// Optional model for Dream LLM operations (None = rule-based fallback).
    #[serde(default)]
    pub dream_model: Option<String>,

    // ── Protection Demotion ────────────────────────────
    #[serde(default = "default_true")]
    pub protection_demotion_enabled: bool,
    #[serde(default = "default_demotion_stale_days")]
    pub protection_demotion_stale_days: u32,
    #[serde(default = "default_demotion_max_step")]
    pub protection_demotion_max_step: u32,

    // ── Proactive Recall ──────────────────────────────
    #[serde(default = "default_true")]
    pub proactive_recall: bool,
    #[serde(default = "default_proactive_limit")]
    pub proactive_recall_limit: usize,
    #[serde(default = "default_proactive_threshold")]
    pub proactive_recall_threshold: f32,
}

fn default_dream_interval() -> u64 { 24 }
fn default_dream_min_sessions() -> u32 { 5 }
fn default_hot_max() -> usize { 50 }
fn default_warm_max() -> usize { 500 }
fn default_cold_max() -> usize { 10_000 }
fn default_hot_token_budget() -> usize { 3_000 }
fn default_one() -> f32 { 1.0 }
fn default_decay_threshold() -> f32 { 0.05 }
fn default_retention_days() -> u32 { 90 }
fn default_protection_low_access() -> u32 { 2 }
fn default_protection_medium_access() -> u32 { 3 }
fn default_protection_high_access() -> u32 { 5 }
fn default_protection_medium_sessions() -> u32 { 2 }
fn default_protection_high_sessions() -> u32 { 3 }
fn default_type_promotion_threshold() -> u32 { 3 }
fn default_compaction_threshold() -> usize { 200 }
fn default_proactive_limit() -> usize { 5 }
fn default_proactive_threshold() -> f32 { 0.6 }
fn default_demotion_stale_days() -> u32 { 30 }
fn default_demotion_max_step() -> u32 { 1 }

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            dream_enabled: true,
            dream_interval_hours: 24,
            dream_min_sessions: 5,
            hot_max_entries: 50,
            warm_max_entries: 500,
            cold_max_entries: 10_000,
            hot_token_budget: 3_000,
            decay_enabled: true,
            decay_multiplier: 1.0,
            decay_threshold: 0.05,
            retention_days: 90,
            auto_protection: true,
            protection_low_access: 2,
            protection_medium_access: 3,
            protection_high_access: 5,
            protection_medium_sessions: 2,
            protection_high_sessions: 3,
            auto_classification: true,
            type_promotion_repetitions: 3,
            compaction_line_threshold: 200,
            llm_compaction: true,
            dream_model: None,
            protection_demotion_enabled: true,
            protection_demotion_stale_days: 30,
            protection_demotion_max_step: 1,
            proactive_recall: true,
            proactive_recall_limit: 5,
            proactive_recall_threshold: 0.6,
        }
    }
}

/// Channel activation configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelsConfig {
    /// List of channel names to activate on startup.
    /// Default: ["web"]
    #[serde(default = "default_channels_enabled")]
    pub enabled: Vec<String>,

    /// Telegram-specific configuration.
    #[serde(default)]
    pub telegram: TelegramChannelConfig,
}

fn default_channels_enabled() -> Vec<String> {
    vec!["web".to_string()]
}

impl Default for ChannelsConfig {
    fn default() -> Self {
        Self {
            enabled: default_channels_enabled(),
            telegram: TelegramChannelConfig::default(),
        }
    }
}

/// Telegram channel configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TelegramChannelConfig {
    /// Environment variable name holding the bot token.
    #[serde(default = "default_telegram_token_env")]
    pub bot_token_env: String,
    /// List of allowed Telegram user IDs (empty = allow all).
    #[serde(default)]
    pub allowed_users: Vec<i64>,
    /// Telegram session management settings.
    #[serde(default)]
    pub session: TelegramSessionConfig,
}

fn default_telegram_token_env() -> String {
    "TELEGRAM_BOT_TOKEN".to_string()
}

impl Default for TelegramChannelConfig {
    fn default() -> Self {
        Self {
            bot_token_env: default_telegram_token_env(),
            allowed_users: Vec::new(),
            session: TelegramSessionConfig::default(),
        }
    }
}

/// LLM engine configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[allow(clippy::derivable_impls)]
pub struct EngineConfig {
    /// Default model in "provider/model" format.
    /// Empty string means no model configured — onboarding required.
    #[serde(default)]
    pub default_model: String,
    /// Explicit API key override (highest priority).
    /// If empty/None, falls back to oxi auth store, then env vars.
    /// Masked when serialized to API responses.
    #[serde(default, skip_serializing)]
    pub api_key: Option<String>,
    /// Per-provider options for fine-grained control (thinking mode, etc.).
    /// Passed through to `AgentLoopConfig::provider_options`.
    #[serde(default)]
    pub provider_options: Option<oxi_sdk::ProviderOptions>,
}

#[allow(clippy::derivable_impls)]
impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            default_model: String::new(),
            api_key: None,
            provider_options: None,
        }
    }
}

/// Daemon mode configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DaemonConfig {
    /// PID file path.
    #[serde(default = "default_pid_file")]
    pub pid_file: String,
    /// Log directory.
    #[serde(default = "default_daemon_log_dir")]
    pub log_dir: String,
}

fn default_pid_file() -> String {
    dirs::home_dir()
        .map(|h| format!("{}/.oxios/oxios.pid", h.display()))
        .unwrap_or_else(|| "./oxios.pid".into())
}

fn default_daemon_log_dir() -> String {
    dirs::home_dir()
        .map(|h| format!("{}/.oxios/logs", h.display()))
        .unwrap_or_else(|| "./logs".into())
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            pid_file: default_pid_file(),
            log_dir: default_daemon_log_dir(),
        }
    }
}

/// Session management configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionConfig {
    /// Maximum number of sessions to retain.
    /// When exceeded, oldest sessions (by `updated_at`) are pruned.
    /// Set to 0 for unlimited.
    #[serde(default = "default_max_sessions")]
    pub max_sessions: usize,

    /// Time-to-live for sessions in hours.
    /// Sessions older than this are automatically pruned.
    /// Set to 0 for unlimited (no TTL-based pruning).
    #[serde(default = "default_session_ttl_hours")]
    pub ttl_hours: u64,

    /// Enable automatic session pruning on every session save.
    #[serde(default = "default_true")]
    pub auto_prune: bool,
}

fn default_max_sessions() -> usize {
    100
}

fn default_session_ttl_hours() -> u64 {
    168 // 7 days
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_sessions: default_max_sessions(),
            ttl_hours: default_session_ttl_hours(),
            auto_prune: true,
        }
    }
}

/// Telegram session management configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TelegramSessionConfig {
    /// Automatically rotate to a new session after this many hours of inactivity.
    /// Set to 0 to disable time-based rotation.
    #[serde(default = "default_telegram_session_rotation_hours")]
    pub rotation_hours: u64,

    /// Maximum number of messages per session before auto-rotating.
    /// Set to 0 for unlimited.
    #[serde(default = "default_telegram_session_max_messages")]
    pub max_messages: usize,
}

fn default_telegram_session_rotation_hours() -> u64 {
    2 // 2 hours
}

fn default_telegram_session_max_messages() -> usize {
    0 // unlimited by default
}

impl Default for TelegramSessionConfig {
    fn default() -> Self {
        Self {
            rotation_hours: default_telegram_session_rotation_hours(),
            max_messages: default_telegram_session_max_messages(),
        }
    }
}

/// Top-level Oxios configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct OxiosConfig {
    /// Kernel settings.
    pub kernel: KernelConfig,
    /// LLM engine settings.
    #[serde(default)]
    pub engine: EngineConfig,
    /// Daemon mode settings.
    #[serde(default)]
    pub daemon: DaemonConfig,
    /// Gateway settings.
    #[serde(default)]
    pub gateway: GatewayConfig,
    /// Scheduler settings (AIOS-inspired task scheduling).
    #[serde(default)]
    pub scheduler: SchedulerConfig,
    /// Orchestrator settings (Ouroboros protocol execution).
    #[serde(default)]
    pub orchestrator: OrchestratorConfig,
    /// Context manager settings (LLM context window management).
    #[serde(default)]
    pub context: ContextConfig,
    /// Security/access control settings.
    #[serde(default)]
    pub security: SecurityConfig,
    /// Persona system settings.
    #[serde(default)]
    pub persona: PersonaConfig,
    /// Memory system settings.
    #[serde(default)]
    pub memory: MemoryConfig,
    /// Cron scheduler settings.
    #[serde(default)]
    pub cron: CronConfig,
    /// MCP server configurations.
    #[serde(default)]
    pub mcp: McpConfig,
    /// Git version control settings.
    #[serde(default)]
    pub git: GitConfig,
    /// Audit trail configuration.
    #[serde(default)]
    pub audit: AuditConfig,
    /// Budget enforcement configuration.
    #[serde(default)]
    pub budget: BudgetConfig,
    /// Exec configuration (host command execution bridge).
    #[serde(default)]
    pub exec: ExecConfig,
    /// Resource monitor configuration.
    #[serde(default)]
    pub resource_monitor: ResourceMonitorConfig,
    /// OpenTelemetry tracing configuration.
    #[serde(default)]
    pub otel: OtelConfig,
    /// Logging configuration.
    #[serde(default)]
    pub logging: LoggingConfig,
    /// Channel activation configuration.
    #[serde(default)]
    pub channels: ChannelsConfig,
    /// Headless browser configuration.
    #[serde(default)]
    pub browser: BrowserConfig,
    /// Session management configuration.
    #[serde(default)]
    pub session: SessionConfig,
    /// ClawHub marketplace configuration.
    #[serde(default)]
    pub marketplace: MarketplaceConfig,
}

/// Kernel configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KernelConfig {
    /// Path to the workspace directory.
    #[serde(default = "default_workspace")]
    pub workspace: String,
    /// Broadcast capacity for the event bus.
    #[serde(default = "default_event_bus_capacity")]
    pub event_bus_capacity: usize,
    /// Maximum number of concurrent agents.
    #[serde(default = "default_max_agents")]
    pub max_agents: usize,
}

fn default_workspace() -> String {
    dirs_home().unwrap_or_else(|| ".".into())
}

fn dirs_home() -> Option<String> {
    dirs::home_dir().map(|h| format!("{}/.oxios/workspace", h.display()))
}

fn default_event_bus_capacity() -> usize {
    256
}

fn default_max_agents() -> usize {
    16
}

impl Default for KernelConfig {
    fn default() -> Self {
        Self {
            workspace: default_workspace(),
            event_bus_capacity: default_event_bus_capacity(),
            max_agents: default_max_agents(),
        }
    }
}

/// Gateway configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GatewayConfig {
    /// Host to bind the gateway to.
    #[serde(default = "default_gateway_host")]
    pub host: String,
    /// Port for the gateway server.
    #[serde(default = "default_gateway_port")]
    pub port: u16,
}

/// ClawHub marketplace configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarketplaceConfig {
    /// Base URL for the ClawHub registry.
    /// Defaults to `https://clawhub.ai`.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Whether the marketplace is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for MarketplaceConfig {
    fn default() -> Self {
        Self {
            base_url: Some("https://clawhub.ai".to_string()),
            enabled: true,
        }
    }
}

fn default_gateway_host() -> String {
    "127.0.0.1".into()
}

fn default_gateway_port() -> u16 {
    4200
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            host: default_gateway_host(),
            port: default_gateway_port(),
        }
    }
}

/// Execution mode for commands.
///
/// - `Structured`: Binary allowlist + metacharacter blocking (recommended)
/// - `Shell`: Raw bash execution (dangerous, requires `allow_shell_mode=true`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExecMode {
    /// Structured binary execution with allowlist and metacharacter blocking.
    #[default]
    Structured,
    /// Shell execution via `bash -c`. DANGEROUS — requires explicit enable.
    Shell,
}

/// Exec configuration.
///
/// Governs how the kernel dispatches commands for execution.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecConfig {
    /// Default execution mode.
    #[serde(default)]
    pub default_mode: ExecMode,
    /// Allow shell mode. DANGEROUS — should be false in production.
    #[serde(default = "default_false")]
    pub allow_shell_mode: bool,
    /// Commands allowed to run on the host.
    /// If empty, *all* bare-name commands are permitted (development mode).
    #[serde(default)]
    pub allowed_commands: Vec<String>,
    /// Default timeout for an exec call in seconds.
    #[serde(default = "default_exec_timeout")]
    pub default_timeout_secs: u64,
    /// Maximum allowed timeout for an exec call in seconds.
    #[serde(default = "default_exec_max_timeout")]
    pub max_timeout_secs: u64,
}

fn default_false() -> bool {
    false
}

fn default_exec_timeout() -> u64 {
    120
}

fn default_exec_max_timeout() -> u64 {
    600
}

impl ExecConfig {
    /// Check whether a binary / command name is allowed to execute.
    ///
    /// Returns `true` when `allowed_commands` is empty (permissive dev mode)
    /// **or** when the name is present in the allow-list.
    pub fn is_binary_allowed(&self, name: &str) -> bool {
        self.allowed_commands.is_empty() || self.allowed_commands.iter().any(|c| c == name)
    }
}

impl Default for ExecConfig {
    fn default() -> Self {
        Self {
            default_mode: ExecMode::default(),
            allow_shell_mode: default_false(),
            allowed_commands: Vec::new(),
            default_timeout_secs: default_exec_timeout(),
            max_timeout_secs: default_exec_max_timeout(),
        }
    }
}

/// Scheduler configuration (inspired by AIOS / AgentRM).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SchedulerConfig {
    /// Maximum number of concurrent agent tasks.
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
    /// Maximum LLM API calls per minute (rate limiting).
    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_minute: u32,
    /// Timeout in seconds before a running task is considered a zombie.
    #[serde(default = "default_zombie_timeout")]
    pub zombie_timeout_secs: u64,
}

fn default_max_concurrent() -> usize {
    5
}

fn default_rate_limit() -> u32 {
    60
}

fn default_zombie_timeout() -> u64 {
    300
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent: default_max_concurrent(),
            rate_limit_per_minute: default_rate_limit(),
            zombie_timeout_secs: default_zombie_timeout(),
        }
    }
}

/// Orchestrator configuration (Ouroboros protocol execution).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OrchestratorConfig {}

// (removed manual impl Default — now derived)

/// Context manager configuration (inspired by AIOS).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContextConfig {
    /// Maximum tokens in the active (in-context) tier.
    #[serde(default = "default_active_limit")]
    pub active_limit_tokens: usize,
    /// Maximum entries in the cache tier.
    #[serde(default = "default_cache_limit")]
    pub cache_limit_entries: usize,
}

fn default_active_limit() -> usize {
    100_000
}

fn default_cache_limit() -> usize {
    50
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            active_limit_tokens: default_active_limit(),
            cache_limit_entries: default_cache_limit(),
        }
    }
}

/// Security/access control configuration (inspired by OWASP Agentic AI).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    /// Default allowed tools for agents (least privilege).
    #[serde(default = "default_allowed_tools")]
    pub allowed_tools: Vec<String>,
    /// Whether agents can make network requests by default.
    #[serde(default)]
    pub network_access: bool,
    /// Maximum execution time in seconds for agent tasks.
    #[serde(default = "default_max_exec_time")]
    pub max_execution_time_secs: u64,
    /// Maximum memory in MB for agent tasks.
    #[serde(default = "default_max_memory")]
    pub max_memory_mb: u64,
    /// Whether agents can fork sub-agents by default.
    #[serde(default)]
    pub can_fork: bool,
    /// Maximum audit log entries to retain.
    #[serde(default = "default_max_audit")]
    pub max_audit_entries: usize,
    /// Enable API key authentication.
    #[serde(default)]
    pub auth_enabled: bool,
    /// Allowed CORS origins.
    #[serde(default = "default_cors_origins")]
    pub cors_origins: Vec<String>,
    /// Path for audit log file (optional, enables file-based persistence).
    #[serde(default)]
    pub audit_log_path: Option<String>,
    /// Rate limit for API endpoints (requests per minute).
    #[serde(default = "default_rate_limit_per_minute")]
    pub rate_limit_per_minute: u32,
}

fn default_allowed_tools() -> Vec<String> {
    vec![
        "read".to_string(),
        "write".to_string(),
        "edit".to_string(),
        "bash".to_string(),
        "grep".to_string(),
        "find".to_string(),
    ]
}

fn default_max_exec_time() -> u64 {
    300
}

fn default_max_memory() -> u64 {
    512
}

fn default_max_audit() -> usize {
    10_000
}

fn default_rate_limit_per_minute() -> u32 {
    120
}

fn default_cors_origins() -> Vec<String> {
    vec!["http://localhost:4200".to_string()]
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            allowed_tools: default_allowed_tools(),
            network_access: false,
            max_execution_time_secs: default_max_exec_time(),
            max_memory_mb: default_max_memory(),
            can_fork: false,
            max_audit_entries: default_max_audit(),
            auth_enabled: false,
            cors_origins: default_cors_origins(),
            audit_log_path: None,
            rate_limit_per_minute: default_rate_limit_per_minute(),
        }
    }
}

/// Persona system configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PersonaConfig {
    /// Default persona ID to activate on startup.
    #[serde(default)]
    pub default_persona_id: Option<String>,
    /// Maximum concurrent personas.
    #[serde(default = "default_max_concurrent_personas")]
    pub max_concurrent_personas: usize,
}

fn default_max_concurrent_personas() -> usize {
    5
}

impl Default for PersonaConfig {
    fn default() -> Self {
        Self {
            default_persona_id: Some("dev".to_string()),
            max_concurrent_personas: default_max_concurrent_personas(),
        }
    }
}

/// MCP server configuration loaded from config.toml.
///
/// Each key is a server name; the value is a table with:
/// - `command`: executable to run (e.g. "npx", "python")
/// - `args`: arguments array
/// - `env`: optional map of environment variables
/// - `enabled`: whether to start this server on boot (default: true)
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct McpConfig {
    /// Map of server-name → server definition.
    #[serde(default)]
    pub servers: std::collections::HashMap<String, McpServerDef>,
}

/// A single MCP server definition in config.toml.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServerDef {
    /// Command to execute.
    pub command: String,
    /// Arguments passed to the command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    /// Whether this server is enabled (default: true).
    #[serde(default = "default_mcp_enabled")]
    pub enabled: bool,
}

fn default_mcp_enabled() -> bool {
    true
}

/// Git version control configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GitConfig {
    /// Enable automatic commits for state changes.
    #[serde(default = "default_true")]
    pub auto_commit: bool,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self { auto_commit: true }
    }
}

/// Audit trail configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuditConfig {
    /// Maximum audit entries before pruning.
    #[serde(default = "default_audit_max_entries")]
    pub max_entries: usize,
    /// Enable audit trail.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_audit_max_entries() -> usize {
    100_000
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            max_entries: default_audit_max_entries(),
            enabled: true,
        }
    }
}

/// Budget enforcement configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BudgetConfig {
    /// Default token budget per agent (0 = unlimited).
    #[serde(default)]
    pub default_token_budget: u64,
    /// Default call budget per agent (0 = unlimited).
    #[serde(default)]
    pub default_calls_budget: u64,
    /// Default budget window in seconds.
    #[serde(default = "default_budget_window")]
    pub default_window_secs: u64,
    /// Enable budget enforcement.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_budget_window() -> u64 {
    3600
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            default_token_budget: 0,
            default_calls_budget: 0,
            default_window_secs: default_budget_window(),
            enabled: true,
        }
    }
}

/// Resource monitor configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResourceMonitorConfig {
    /// Snapshot interval in seconds.
    #[serde(default = "default_rm_interval")]
    pub interval_secs: u64,
    /// Maximum history entries.
    #[serde(default = "default_rm_history_max")]
    pub history_max: usize,
    /// CPU threshold for overload.
    #[serde(default = "default_rm_cpu_threshold")]
    pub cpu_threshold: f32,
    /// Memory threshold for overload (percentage).
    #[serde(default = "default_rm_mem_threshold")]
    pub memory_threshold: f32,
    /// Load average threshold for overload.
    #[serde(default = "default_rm_load_threshold")]
    pub load_threshold: f32,
}

fn default_rm_interval() -> u64 {
    60
}

fn default_rm_history_max() -> usize {
    60
}

fn default_rm_cpu_threshold() -> f32 {
    90.0
}

fn default_rm_mem_threshold() -> f32 {
    90.0
}

fn default_rm_load_threshold() -> f32 {
    8.0
}

impl Default for ResourceMonitorConfig {
    fn default() -> Self {
        Self {
            interval_secs: default_rm_interval(),
            history_max: default_rm_history_max(),
            cpu_threshold: default_rm_cpu_threshold(),
            memory_threshold: default_rm_mem_threshold(),
            load_threshold: default_rm_load_threshold(),
        }
    }
}

/// OpenTelemetry tracing configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OtelConfig {
    /// Enable OTLP export (default: false).
    #[serde(default)]
    pub enabled: bool,
    /// OTLP gRPC endpoint.
    #[serde(default = "default_otel_endpoint")]
    pub endpoint: String,
    /// Service name for traces.
    #[serde(default = "default_otel_service_name")]
    pub service_name: String,
    /// Sampling ratio (0.0 to 1.0).
    #[serde(default = "default_otel_sampling_ratio")]
    pub sampling_ratio: f64,
}

fn default_otel_endpoint() -> String {
    "http://localhost:4317".into()
}

fn default_otel_service_name() -> String {
    "oxios".into()
}

fn default_otel_sampling_ratio() -> f64 {
    1.0
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: default_otel_endpoint(),
            service_name: default_otel_service_name(),
            sampling_ratio: default_otel_sampling_ratio(),
        }
    }
}

/// Logging configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// Log format: "pretty", "json", or "compact".
    #[serde(default = "default_log_format")]
    pub format: String,
    /// Log level override (e.g. "info", "debug"). Falls back to RUST_LOG env var.
    #[serde(default)]
    pub level: Option<String>,
}

fn default_log_format() -> String {
    "pretty".into()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            format: default_log_format(),
            level: None,
        }
    }
}

/// Headless browser configuration.
///
/// Wraps `oxibrowser_core::BrowserConfig` (Deserialize/Serialize supported)
/// with an `enabled` toggle. The engine config is passed through directly
/// to the browser — no field-by-field duplication.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BrowserConfig {
    /// Enable the browser integration.
    #[serde(default = "default_browser_enabled")]
    pub enabled: bool,

    /// Engine configuration — passed directly to `oxibrowser_core::Browser::new()`.
    ///
    /// All fields have sensible defaults; override only what you need:
    ///
    /// ```toml
    /// [browser.engine]
    /// user_agent = "MyBot/1.0"
    /// obey_robots = false
    /// js_timeout_ms = 10000
    /// ```
    #[serde(default)]
    pub engine: oxibrowser_core::BrowserConfig,
}

fn default_browser_enabled() -> bool {
    true
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            engine: oxibrowser_core::BrowserConfig::headless(),
        }
    }
}

/// Loads configuration from a TOML file.
pub fn load_config(path: &std::path::Path) -> anyhow::Result<OxiosConfig> {
    let content = std::fs::read_to_string(path)?;
    let config: OxiosConfig = toml::from_str(&content)?;
    let (errors, warnings) = config.validate();
    for w in warnings {
        tracing::warn!("config: {}", w);
    }
    if !errors.is_empty() {
        let msg = errors.join("; ");
        anyhow::bail!("Configuration validation failed: {}", msg);
    }
    Ok(config)
}

impl OxiosConfig {
    /// Returns the effective API key from the engine config.
    pub fn api_key(&self) -> Option<String> {
        self.engine.api_key.clone().filter(|k| !k.is_empty())
    }

    /// Validate configuration values and return a list of warnings.
    /// Returns (errors, warnings). Empty errors = valid config.
    pub fn validate(&self) -> (Vec<String>, Vec<String>) {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Kernel validation
        if self.kernel.max_agents == 0 {
            errors.push("kernel.max_agents must be > 0".into());
        }
        if self.kernel.workspace.is_empty() {
            errors.push("kernel.workspace must not be empty".into());
        }

        // Gateway validation
        if self.gateway.port == 0 {
            errors.push("gateway.port must be > 0".into());
        }
        if self.gateway.port < 1024 && self.gateway.host == "0.0.0.0" {
            warnings.push("Running on port <1024 as 0.0.0.0 may require root".into());
        }

        // Scheduler validation
        if self.scheduler.max_concurrent == 0 {
            warnings.push("scheduler.max_concurrent is 0 — no tasks will run".into());
        }
        if self.scheduler.zombie_timeout_secs == 0 {
            errors.push("scheduler.zombie_timeout_secs must be > 0".into());
        }

        // Cron validation
        for (name, job) in &self.cron.jobs {
            if job.schedule.is_empty() {
                errors.push(format!("cron.jobs.{}: schedule is empty", name));
            } else {
                // Normalize 5-field to 6-field (prepend "0 " for seconds)
                let normalized = {
                    let fields: Vec<&str> = job.schedule.split_whitespace().collect();
                    match fields.len() {
                        5 => format!("0 {}", job.schedule),
                        _ => job.schedule.clone(),
                    }
                };
                if Schedule::from_str(&normalized).is_err() {
                    errors.push(format!(
                        "cron.jobs.{}: invalid cron expression '{}'",
                        name, job.schedule
                    ));
                }
            }
            if job.goal.is_empty() {
                errors.push(format!("cron.jobs.{}: goal is empty", name));
            }
        }

        // Security validation
        if self.security.max_execution_time_secs == 0 {
            warnings.push("security.max_execution_time_secs is 0 — no timeout".into());
        }

        // Audit validation
        if self.audit.max_entries == 0 {
            warnings.push("audit.max_entries is 0 — audit will never prune".into());
        }

        // Budget validation
        if self.budget.default_window_secs == 0 {
            warnings.push("budget.default_window_secs is 0 — no time window".into());
        }

        // Session validation
        if self.session.max_sessions == 0 && self.session.ttl_hours == 0 && self.session.auto_prune {
            warnings.push("session: auto_prune is enabled but both max_sessions and ttl_hours are 0 — nothing will be pruned".into());
        }

        // Exec validation
        if self.exec.default_timeout_secs == 0 {
            errors.push("exec.default_timeout_secs must be > 0".into());
        }
        if self.exec.max_timeout_secs == 0 {
            errors.push("exec.max_timeout_secs must be > 0".into());
        }
        if self.exec.default_timeout_secs > self.exec.max_timeout_secs {
            errors.push(format!(
                "exec.default_timeout_secs ({}) must not exceed max_timeout_secs ({})",
                self.exec.default_timeout_secs, self.exec.max_timeout_secs
            ));
        }

        // Resource monitor validation
        if self.resource_monitor.cpu_threshold > 100.0 {
            errors.push("resource_monitor.cpu_threshold must be <= 100".into());
        }
        if self.resource_monitor.memory_threshold > 100.0 {
            errors.push("resource_monitor.memory_threshold must be <= 100".into());
        }

        // Channels validation
        for name in &self.channels.enabled {
            let valid = ["web", "cli", "telegram"];
            if !valid.contains(&name.as_str()) {
                warnings.push(format!("channels.enabled: unknown channel '{}'", name));
            }
        }
        if self.channels.enabled.iter().any(|c| c == "telegram")
            && std::env::var(&self.channels.telegram.bot_token_env).is_err()
        {
            warnings.push(format!(
                "channels.telegram: {} env var not set — telegram channel will fail",
                self.channels.telegram.bot_token_env
            ));
        }

        (errors, warnings)
    }
}

/// Expand `~/` in paths to the user's home directory.
///
/// Shared utility for path expansion across the binary and kernel.
pub fn expand_home(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(format!("{home}/{rest}"));
        }
    }
    std::path::PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validates() {
        let config = OxiosConfig::default();
        let (errors, _warnings) = config.validate();
        assert!(
            errors.is_empty(),
            "Default config should have no errors: {:?}",
            errors
        );
    }

    #[test]
    fn test_exec_config_default_allowed_commands() {
        let config = ExecConfig::default();
        // Empty allowed_commands means all commands are permitted.
        assert!(config.allowed_commands.is_empty());
        assert!(config.is_binary_allowed("anything"));
        assert!(config.is_binary_allowed("bash"));
        assert!(config.is_binary_allowed("rm"));
    }

    #[test]
    fn test_is_binary_allowed_with_allowlist() {
        let config = ExecConfig {
            allowed_commands: vec!["git".into(), "echo".into()],
            ..Default::default()
        };
        assert!(config.is_binary_allowed("git"));
        assert!(config.is_binary_allowed("echo"));
        assert!(!config.is_binary_allowed("bash"));
        assert!(!config.is_binary_allowed("rm"));
        assert!(!config.is_binary_allowed("sudo"));
    }

    #[test]
    fn test_expand_home() {
        // With HOME set.
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp/testhome".into());
        let expanded = expand_home("~/projects/test");
        assert_eq!(
            expanded.to_str().unwrap(),
            format!("{}/projects/test", home)
        );

        // Non-tilde path should pass through unchanged.
        let abs = expand_home("/absolute/path");
        assert_eq!(abs, std::path::PathBuf::from("/absolute/path"));

        // Just ~ without slash should not expand.
        let bare = expand_home("~something");
        assert_eq!(bare, std::path::PathBuf::from("~something"));
    }

    #[test]
    fn test_invalid_cron_expression() {
        let mut config = OxiosConfig::default();
        config.cron.enabled = true;
        config.cron.jobs.insert(
            "bad-job".to_string(),
            InlineCronJob {
                schedule: "not a valid cron".to_string(),
                goal: "Test goal".to_string(),
                constraints: vec![],
                acceptance_criteria: vec![],
                toolchain: "default".to_string(),
                priority: Priority::Normal,
                enabled: true,
            },
        );

        let (errors, _warnings) = config.validate();
        assert!(
            !errors.is_empty(),
            "Expected validation error for invalid cron"
        );
        let has_cron_error = errors.iter().any(|e| e.contains("invalid cron expression"));
        assert!(
            has_cron_error,
            "Expected 'invalid cron expression' error, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = OxiosConfig::default();

        // Serialize to TOML string.
        let toml_str = toml::to_string(&config).expect("serialization should succeed");

        // Deserialize back.
        let deserialized: OxiosConfig =
            toml::from_str(&toml_str).expect("deserialization should succeed");

        // Key fields should match.
        assert_eq!(config.kernel.max_agents, deserialized.kernel.max_agents);
        assert_eq!(config.kernel.workspace, deserialized.kernel.workspace);
        assert_eq!(config.gateway.host, deserialized.gateway.host);
        assert_eq!(config.gateway.port, deserialized.gateway.port);
        assert_eq!(
            config.exec.default_timeout_secs,
            deserialized.exec.default_timeout_secs
        );
        assert_eq!(
            config.exec.max_timeout_secs,
            deserialized.exec.max_timeout_secs
        );
    }

    #[test]
    fn test_exec_timeout_validation() {
        let mut config = OxiosConfig::default();
        // default_timeout > max_timeout should be an error.
        config.exec.default_timeout_secs = 999;
        config.exec.max_timeout_secs = 100;
        let (errors, _warnings) = config.validate();
        let has_error = errors.iter().any(|e| e.contains("must not exceed"));
        assert!(
            has_error,
            "Expected timeout ordering error, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_zero_max_agents_error() {
        let mut config = OxiosConfig::default();
        config.kernel.max_agents = 0;
        let (errors, _warnings) = config.validate();
        assert!(errors.iter().any(|e| e.contains("max_agents must be > 0")));
    }
}
