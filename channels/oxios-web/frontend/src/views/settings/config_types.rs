//! Frontend mirror types for OxiosConfig.
//! These mirror the Rust config structs from oxios-kernel/src/config.rs
//! but live in the WASM frontend. All structs derive Clone, Debug, Serialize,
//! Deserialize, Default. All fields have #[serde(default)] for partial JSON.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Top-level
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigSnapshot {
    #[serde(default)]
    pub kernel: KernelSnapshot,
    #[serde(default)]
    pub engine: EngineSnapshot,
    #[serde(default)]
    pub daemon: DaemonSnapshot,
    #[serde(default)]
    pub gateway: GatewaySnapshot,
    #[serde(default)]
    pub scheduler: SchedulerSnapshot,
    #[serde(default)]
    pub orchestrator: OrchestratorSnapshot,
    #[serde(default)]
    pub context: ContextSnapshot,
    #[serde(default)]
    pub security: SecuritySnapshot,
    #[serde(default)]
    pub persona: PersonaSnapshot,
    #[serde(default)]
    pub memory: MemorySnapshot,
    #[serde(default)]
    pub cron: CronSnapshot,
    #[serde(default)]
    pub mcp: McpSnapshot,
    #[serde(default)]
    pub git: GitSnapshot,
    #[serde(default)]
    pub audit: AuditSnapshot,
    #[serde(default)]
    pub budget: BudgetSnapshot,
    #[serde(default)]
    pub exec: ExecSnapshot,
    #[serde(default)]
    pub resource_monitor: ResourceMonitorSnapshot,
    #[serde(default)]
    pub otel: OtelSnapshot,
    #[serde(default)]
    pub channels: ChannelsSnapshot,
    #[serde(default)]
    pub browser: BrowserSnapshot,
}

// ---------------------------------------------------------------------------
// Kernel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KernelSnapshot {
    #[serde(default)]
    pub workspace: String,
    #[serde(default)]
    pub event_bus_capacity: usize,
    #[serde(default)]
    pub max_agents: usize,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EngineSnapshot {
    #[serde(default)]
    pub default_model: String,
    /// Only used internally when editing — not sent to backend on save if empty.
    #[serde(default)]
    pub api_key: Option<String>,
    /// Populated by the backend to indicate whether a key is currently set.
    #[serde(default)]
    pub api_key_set: bool,
}

// ---------------------------------------------------------------------------
// Daemon
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DaemonSnapshot {
    #[serde(default)]
    pub pid_file: String,
    #[serde(default)]
    pub log_dir: String,
}

// ---------------------------------------------------------------------------
// Gateway
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GatewaySnapshot {
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: u16,
}

// ---------------------------------------------------------------------------
// Scheduler
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SchedulerSnapshot {
    #[serde(default)]
    pub max_concurrent: usize,
    #[serde(default)]
    pub rate_limit_per_minute: u32,
    #[serde(default)]
    pub zombie_timeout_secs: u64,
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrchestratorSnapshot {
    #[serde(default)]
    pub max_evolution_iterations: usize,
    #[serde(default)]
    pub min_evaluation_score: f64,
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextSnapshot {
    #[serde(default)]
    pub active_limit_tokens: usize,
    #[serde(default)]
    pub cache_limit_entries: usize,
}

// ---------------------------------------------------------------------------
// Security
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecuritySnapshot {
    #[serde(default)]
    pub auth_enabled: bool,
    #[serde(default)]
    pub network_access: bool,
    #[serde(default)]
    pub can_fork: bool,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub max_execution_time_secs: u64,
    #[serde(default)]
    pub max_memory_mb: u64,
    #[serde(default)]
    pub cors_origins: Vec<String>,
    #[serde(default)]
    pub rate_limit_per_minute: u32,
    #[serde(default)]
    pub max_audit_entries: usize,
    #[serde(default)]
    pub audit_log_path: Option<String>,
}

// ---------------------------------------------------------------------------
// Persona
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersonaSnapshot {
    #[serde(default)]
    pub default_persona_id: Option<String>,
    #[serde(default)]
    pub max_concurrent_personas: usize,
}

// ---------------------------------------------------------------------------
// Memory
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemorySnapshot {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub max_recall: usize,
    #[serde(default)]
    pub auto_summarize: bool,
    #[serde(default)]
    pub capture_compaction: bool,
    #[serde(default)]
    pub retention_days: u32,
    #[serde(default)]
    pub cache_enabled: bool,
    #[serde(default)]
    pub cache_ttl_secs: u64,
    #[serde(default)]
    pub cache_max_entries: usize,
}

// ---------------------------------------------------------------------------
// Cron
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CronSnapshot {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub tick_interval_secs: u64,
}

// ---------------------------------------------------------------------------
// MCP
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpSnapshot {
    #[serde(default)]
    pub servers: HashMap<String, McpServerDefSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpServerDefSnapshot {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// Git
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitSnapshot {
    #[serde(default)]
    pub auto_commit: bool,
}

// ---------------------------------------------------------------------------
// Audit
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditSnapshot {
    #[serde(default)]
    pub max_entries: usize,
    #[serde(default)]
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// Budget
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BudgetSnapshot {
    #[serde(default)]
    pub default_token_budget: u64,
    #[serde(default)]
    pub default_calls_budget: u64,
    #[serde(default)]
    pub default_window_secs: u64,
    #[serde(default)]
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// Exec
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecSnapshot {
    /// "structured" or "shell"
    #[serde(default)]
    pub default_mode: String,
    #[serde(default)]
    pub allow_shell_mode: bool,
    #[serde(default)]
    pub allowed_commands: Vec<String>,
    #[serde(default)]
    pub default_timeout_secs: u64,
    #[serde(default)]
    pub max_timeout_secs: u64,
    #[serde(default)]
    pub required_host_tools: Vec<String>,
    #[serde(default)]
    pub optional_host_tools: Vec<String>,
}

// ---------------------------------------------------------------------------
// Resource Monitor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceMonitorSnapshot {
    #[serde(default)]
    pub interval_secs: u64,
    #[serde(default)]
    pub history_max: usize,
    #[serde(default)]
    pub cpu_threshold: f32,
    #[serde(default)]
    pub memory_threshold: f32,
    #[serde(default)]
    pub load_threshold: f32,
}

// ---------------------------------------------------------------------------
// Otel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OtelSnapshot {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub service_name: String,
    #[serde(default)]
    pub sampling_ratio: f64,
}

// ---------------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelsSnapshot {
    #[serde(default)]
    pub enabled: Vec<String>,
    #[serde(default)]
    pub telegram: TelegramChannelSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TelegramChannelSnapshot {
    #[serde(default)]
    pub bot_token_env: String,
    #[serde(default)]
    pub allowed_users: Vec<i64>,
}

// ---------------------------------------------------------------------------
// Browser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrowserSnapshot {
    #[serde(default)]
    pub enabled: bool,
}
