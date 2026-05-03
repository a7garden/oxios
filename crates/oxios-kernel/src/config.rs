//! Configuration loading from TOML files.
//!
//! Configuration is stored at `~/.oxios/config.toml` and controls
//! kernel, gateway, and container settings.

use serde::Deserialize;

/// Top-level Oxios configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct OxiosConfig {
    /// Kernel settings.
    pub kernel: KernelConfig,
    /// Gateway settings.
    #[serde(default)]
    pub gateway: GatewayConfig,
    /// Container settings.
    #[serde(default)]
    pub container: ContainerConfig,
}

/// Kernel configuration.
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
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

/// Container (garden) configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ContainerConfig {
    /// Base directory for container gardens.
    #[serde(default = "default_garden_path")]
    pub garden_path: String,
    /// Default image tag for new gardens.
    #[serde(default = "default_image_tag")]
    pub image_tag: String,
    /// Allowed host commands (for the Host Exec Bridge).
    /// If empty, all bare-name commands are allowed (development mode).
    #[serde(default)]
    pub allowed_host_commands: Vec<String>,
    /// Default memory limit for garden containers.
    #[serde(default = "default_memory_limit")]
    pub memory_limit: String,
    /// Default CPU limit for garden containers.
    #[serde(default = "default_cpu_limit")]
    pub cpu_limit: u64,
}

fn default_garden_path() -> String {
    std::env::var("HOME")
        .map(|h| format!("{h}/.oxios/gardens"))
        .unwrap_or_else(|_| "./gardens".into())
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

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            garden_path: default_garden_path(),
            image_tag: default_image_tag(),
            allowed_host_commands: Vec::new(),
            memory_limit: default_memory_limit(),
            cpu_limit: default_cpu_limit(),
        }
    }
}

/// Loads configuration from a TOML file.
pub fn load_config(path: &std::path::Path) -> anyhow::Result<OxiosConfig> {
    let content = std::fs::read_to_string(path)?;
    let config: OxiosConfig = toml::from_str(&content)?;
    Ok(config)
}
