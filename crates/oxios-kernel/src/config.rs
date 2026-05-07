//! Configuration loading from TOML files.
//!
//! Configuration is stored at `~/.oxios/config.toml` and controls
//! kernel, gateway, and container settings.

use crate::scheduler::Priority;
use serde::{Deserialize, Serialize};

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
    pub schedule: String,
    pub goal: String,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(default = "default_toolchain_inline")]
    pub toolchain: String,
    #[serde(default)]
    pub priority: Priority,
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
}

/// Execution mode for agent command execution.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    /// Execute inside a container (production).
    Container,
    /// Execute directly on host (development).
    #[default]
    Auto,
}

fn default_true() -> bool { true }

fn default_max_recall() -> usize { 10 }

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_recall: 10,
            auto_summarize: true,
            capture_compaction: true,
            retention_days: 0,
        }
    }
}

/// Top-level Oxios configuration.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct OxiosConfig {
    /// Kernel settings.
    pub kernel: KernelConfig,
    /// Gateway settings.
    #[serde(default)]
    pub gateway: GatewayConfig,
    /// Container settings.
    #[serde(default)]
    pub container: ContainerConfig,
    /// Scheduler settings (AIOS-inspired task scheduling).
    #[serde(default)]
    pub scheduler: SchedulerConfig,
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
    std::env::var("HOME").ok().map(|h| format!("{h}/.oxios/workspace"))
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

/// Container configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContainerConfig {
    /// Base directory for containers.
    #[serde(default = "default_container_path")]
    pub container_path: String,
    /// Default image tag for new containers.
    #[serde(default = "default_image_tag")]
    pub image_tag: String,
    /// Allowed host commands (for the Host Exec Bridge).
    /// If empty, all bare-name commands are allowed (development mode).
    #[serde(default)]
    pub allowed_host_commands: Vec<String>,
    /// Default memory limit for containers.
    #[serde(default = "default_memory_limit")]
    pub memory_limit: String,
    /// Default CPU limit for containers.
    #[serde(default = "default_cpu_limit")]
    pub cpu_limit: u64,
    /// Minimal container tools (pre-installed in the minimal container image).
    #[serde(default = "default_minimal_tools")]
    pub minimal_tools: Vec<String>,
    /// Host tools that MUST be on host (checked on startup).
    #[serde(default = "default_required_host_tools")]
    pub required_host_tools: Vec<String>,
    /// Optional host tools: checked when program needs them.
    #[serde(default = "default_optional_host_tools")]
    pub optional_host_tools: Vec<String>,
    /// Execution mode for command execution.
    #[serde(default)]
    pub execution_mode: ExecutionMode,
}

fn default_container_path() -> String {
    std::env::var("HOME")
        .map(|h| format!("{h}/.oxios/containers"))
        .unwrap_or_else(|_| "./containers".into())
}

fn default_image_tag() -> String {
    "oxios:latest".into()
}

fn default_memory_limit() -> String {
    "4g".into()
}

fn default_cpu_limit() -> u64 {
    4
}

fn default_minimal_tools() -> Vec<String> {
    vec![
        "curl".to_string(),
        "git".to_string(),
        "ripgrep".to_string(),
        "jq".to_string(),
        "sqlite3".to_string(),
        "bash".to_string(),
        "python3".to_string(),
    ]
}

fn default_required_host_tools() -> Vec<String> {
    vec!["git".to_string()]
}

fn default_optional_host_tools() -> Vec<String> {
    vec![
        "gh".to_string(),
        "remindctl".to_string(),
        "shortcuts".to_string(),
        "osascript".to_string(),
        "open".to_string(),
    ]
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            container_path: default_container_path(),
            image_tag: default_image_tag(),
            allowed_host_commands: Vec::new(),
            memory_limit: default_memory_limit(),
            cpu_limit: default_cpu_limit(),
            minimal_tools: default_minimal_tools(),
            required_host_tools: default_required_host_tools(),
            optional_host_tools: default_optional_host_tools(),
            execution_mode: ExecutionMode::Auto,
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
    /// Path to API keys file.
    #[serde(default = "default_api_keys_path")]
    pub api_keys_path: String,
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

fn default_api_keys_path() -> String {
    std::env::var("HOME")
        .map(|h| format!("{h}/.oxios/api-keys.json"))
        .unwrap_or_else(|_| "./api-keys.json".into())
}

fn default_cors_origins() -> Vec<String> {
    vec!["http://localhost:4200".to_string()]
}

fn default_rate_limit_per_minute() -> u32 {
    120
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
            api_keys_path: default_api_keys_path(),
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

/// Loads configuration from a TOML file.
pub fn load_config(path: &std::path::Path) -> anyhow::Result<OxiosConfig> {
    let content = std::fs::read_to_string(path)?;
    let config: OxiosConfig = toml::from_str(&content)?;
    Ok(config)
}
