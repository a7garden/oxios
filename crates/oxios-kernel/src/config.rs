//! Configuration loading from TOML files.
//!
//! Configuration is stored at `~/.oxios/config.toml` and controls
//! kernel, gateway, and container settings.

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
    /// Git version control settings.
    #[serde(default)]
    pub git: GitConfig,
    /// Audit trail configuration.
    #[serde(default)]
    pub audit: AuditConfig,
    /// Budget enforcement configuration.
    #[serde(default)]
    pub budget: BudgetConfig,
    /// Resource monitor configuration.
    #[serde(default)]
    pub resource_monitor: ResourceMonitorConfig,
    /// OpenTelemetry tracing configuration.
    #[serde(default)]
    pub otel: OtelConfig,
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
    /// Path to API keys file (JSON with SHA-256 hashed keys).
    #[serde(default = "default_api_keys_path")]
    pub api_keys_path: String,
    /// Static API key for simple deployments.
    /// Set this OR use the file at `api_keys_path`, not both.
    #[serde(default)]
    pub default_api_key: Option<String>,
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
            default_api_key: None,
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
    /// Returns the effective API key — prefers OXIOS_API_KEY env var,
    /// falls back to the security.default_api_key config field.
    pub fn api_key(&self) -> Option<String> {
        std::env::var("OXIOS_API_KEY")
            .ok()
            .filter(|k| !k.is_empty())
            .or_else(|| self.security.default_api_key.clone())
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

        // Container validation
        if self.container.container_path.is_empty() {
            errors.push("container.container_path must not be empty".into());
        }
        if self.container.memory_limit.is_empty() {
            errors.push("container.memory_limit must not be empty".into());
        }
        // Validate memory format (e.g. "4g", "512m")
        let valid = self.container.memory_limit.ends_with('k')
            || self.container.memory_limit.ends_with('m')
            || self.container.memory_limit.ends_with('g')
            || self.container.memory_limit.ends_with('t');
        if !valid {
            warnings.push(format!(
                "container.memory_limit '{}' may not be valid",
                self.container.memory_limit
            ));
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
        if self.security.auth_enabled {
            let keys_path = std::path::Path::new(&self.security.api_keys_path);
            if !keys_path.exists() {
                warnings.push(format!(
                    "security.api_keys_path '{}' does not exist — auth will fail",
                    self.security.api_keys_path
                ));
            }
        }
        if self.security.max_execution_time_secs == 0 {
            warnings.push("security.max_execution_time_secs is 0 — no timeout".into());
        }

        // Check for API key in environment variable
        if std::env::var("OXIOS_API_KEY").is_ok() {
            warnings.push(
                "OXIOS_API_KEY is set in environment — consider using it instead of config file".into(),
            );
        }

        // Audit validation
        if self.audit.max_entries == 0 {
            warnings.push("audit.max_entries is 0 — audit will never prune".into());
        }

        // Budget validation
        if self.budget.default_window_secs == 0 {
            warnings.push("budget.default_window_secs is 0 — no time window".into());
        }

        // Resource monitor validation
        if self.resource_monitor.cpu_threshold > 100.0 {
            errors.push("resource_monitor.cpu_threshold must be <= 100".into());
        }
        if self.resource_monitor.memory_threshold > 100.0 {
            errors.push("resource_monitor.memory_threshold must be <= 100".into());
        }

        (errors, warnings)
    }
}
