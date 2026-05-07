//! Kernel assembly — Builder pattern for wiring all Oxios components.
//!
//! This module lives in the binary crate (not oxios-kernel) because
//! it's responsible for *assembling* kernel components, not providing them.
//! The kernel library provides parts; the binary puts them together.

use anyhow::{Context, Result};
use oxios_gateway::Gateway;
use oxios_kernel::{
    access_manager::AccessManager, auth::AuthManager, config::load_config,
    A2AProtocol, AgentRuntime, BasicSupervisor, ContainerManager,
    EngineProvider, EventBus, HostExecBridge, HostToolValidator,
    McpBridge, McpServer, Orchestrator, OxiosConfig, PersonaManager,
    ProgramManager, SkillStore, AgentScheduler, Supervisor,
};
use oxios_ouroboros::{OuroborosEngine, OuroborosProtocol};
use std::path::PathBuf;
use std::sync::Arc;

/// Fully assembled Oxios kernel with all components wired together.
///
/// Created via [`Kernel::builder()`]. Each field is publicly accessible
/// for use by the main binary's subcommands and web server setup.
pub struct Kernel {
    /// Ouroboros lifecycle orchestrator.
    pub orchestrator: Arc<Orchestrator>,
    /// Channel-agnostic message gateway.
    pub gateway: Gateway,
    /// Kernel event bus.
    pub event_bus: EventBus,
    /// Persistent state store (markdown/JSON).
    pub state_store: Arc<oxios_kernel::state_store::StateStore>,
    /// Container lifecycle manager.
    pub container_manager: Arc<ContainerManager>,
    /// Loaded configuration.
    pub config: OxiosConfig,
    /// Skill instruction store.
    pub skill_store: SkillStore,
    /// Agent supervisor (lifecycle management).
    pub supervisor: Arc<dyn Supervisor>,
    /// Task scheduler.
    pub scheduler: Arc<AgentScheduler>,
    /// Access control manager (RBAC, audit).
    pub access_manager: Arc<parking_lot::Mutex<AccessManager>>,
    /// OS-level program manager.
    pub program_manager: Arc<ProgramManager>,
    /// Host tool validator.
    pub host_tool_validator: HostToolValidator,
    /// Persona manager (multi-persona support).
    pub persona_manager: PersonaManager,
    /// MCP tool bridge.
    pub mcp_bridge: Arc<tokio::sync::Mutex<McpBridge>>,
    /// API key authentication manager.
    pub auth_manager: Arc<parking_lot::Mutex<AuthManager>>,
}

/// Builder for assembling the Oxios kernel.
pub struct KernelBuilder {
    config_path: PathBuf,
    model_id: String,
}

impl KernelBuilder {
    /// Set the config file path.
    pub fn config_path(mut self, path: PathBuf) -> Self {
        self.config_path = path;
        self
    }

    /// Set the LLM model ID (e.g., "anthropic/claude-sonnet-4-20250514").
    pub fn model_id(mut self, model: &str) -> Self {
        self.model_id = model.to_string();
        self
    }

    /// Assemble all kernel components and wire them together.
    pub async fn build(self) -> Result<Kernel> {
        let config_path = self.config_path;
        let model_id = &self.model_id;

        // ── Load configuration ──
        let config = if config_path.exists() {
            tracing::info!(path = %config_path.display(), "Loading config");
            load_config(&config_path)?
        } else {
            tracing::info!("No config file found, using defaults");
            OxiosConfig::default()
        };

        // ── Core infrastructure ──
        let event_bus = EventBus::new(config.kernel.event_bus_capacity);
        let state_store = Arc::new(oxios_kernel::state_store::StateStore::new(
            PathBuf::from(&config.kernel.workspace),
        )?);

        // ── Engine provider ──
        let engine_provider = oxios_kernel::OxiEngineProvider::new(model_id);
        let model = engine_provider
            .resolve_model(model_id)
            .context("Failed to resolve model")?;
        let provider = engine_provider
            .create_provider(&model.provider)
            .context("Failed to create provider")?;

        // ── Ouroboros engine ──
        let ouroboros: Arc<dyn OuroborosProtocol> =
            Arc::new(OuroborosEngine::new(Arc::clone(&provider), model));

        // ── Access control & scheduling ──
        let access_manager = Arc::new(parking_lot::Mutex::new(AccessManager::new()));
        let scheduler = Arc::new(AgentScheduler::new(
            config.scheduler.max_concurrent,
            config.scheduler.rate_limit_per_minute,
            config.scheduler.zombie_timeout_secs,
        ));

        // ── Persona (created once, used everywhere) ──
        let persona_manager = PersonaManager::new();
        if let Some(p) = persona_manager.first_enabled() {
            ouroboros.set_persona_prompt(Some(p.system_prompt));
            tracing::info!(persona = %p.name, "Active persona set on OuroborosEngine");
        }

        // ── A2A protocol (created once) ──
        let a2a_protocol = Arc::new(A2AProtocol::new(event_bus.clone()));

        // ── Container infrastructure ──
        let containers_base = PathBuf::from(&config.container.container_path);
        let host_exec = Arc::new(
            HostExecBridge::new(
                containers_base.clone(),
                config.container.allowed_host_commands.clone(),
            )
            .context("HostExecBridge requires non-empty allowlist")?,
        );
        let state_store_for_containers =
            oxios_kernel::state_store::StateStore::new(PathBuf::from(&config.kernel.workspace))?;
        let container_manager = Arc::new(ContainerManager::with_apple_backend(
            host_exec.clone(),
            Arc::new(state_store_for_containers),
            containers_base,
        ));

        // ── Skills & programs ──
        let skills_dir = PathBuf::from(&config.kernel.workspace).join("skills");
        let skill_store = SkillStore::new(skills_dir)?;
        let programs_dir = PathBuf::from(&config.kernel.workspace).join("programs");
        let program_manager = Arc::new(ProgramManager::new(programs_dir));
        program_manager.init().await?;

        // ── Agent runtime ──
        let agent_runtime = AgentRuntime::new(provider, model_id)
            .with_container(Arc::clone(&container_manager))
            .with_host_bridge(host_exec)
            .with_program_manager(Arc::clone(&program_manager))
            .with_oxios_config(config.clone())
            .with_persona_manager(Arc::new(persona_manager.clone()));

        // ── Supervisor ──
        let supervisor: Arc<dyn Supervisor> = Arc::new(BasicSupervisor::new(
            event_bus.clone(),
            agent_runtime,
        ));

        // ── Orchestrator ──
        let orchestrator = Arc::new(Orchestrator::new(
            ouroboros,
            supervisor.clone(),
            event_bus.clone(),
            state_store.clone(),
            scheduler.clone(),
            access_manager.clone(),
            Arc::new(persona_manager.clone()),
            a2a_protocol.clone(),
        ));

        // ── Gateway ──
        let gateway = Gateway::new(orchestrator.clone());

        // ── Host tool validator ──
        let host_tool_validator = HostToolValidator::new(
            config.container.required_host_tools.clone(),
            config.container.optional_host_tools.clone(),
        );

        // ── Auth manager ──
        let mut auth_manager = AuthManager::new();
        let api_keys_path = PathBuf::from(&config.security.api_keys_path);
        if let Err(e) = auth_manager.load_from_file(&api_keys_path) {
            tracing::debug!(error = %e, "No API keys file loaded (this is normal on first run)");
        }
        let auth_manager = Arc::new(parking_lot::Mutex::new(auth_manager));

        // ── MCP bridge (register program MCP servers before Arc wrapping) ──
        let mut mcp_bridge = init_mcp_bridge(&config).await?;

        // Register MCP servers from installed programs
        for program in program_manager.list_enabled().await {
            for server_config in &program.meta.mcp_servers {
                if server_config.enabled {
                    mcp_bridge.register_server(McpServer {
                        name: server_config.name.clone(),
                        command: server_config.command.clone(),
                        args: server_config.args.clone(),
                        env: server_config.env.clone(),
                        enabled: server_config.enabled,
                    });
                }
            }
        }
        let mcp_bridge = Arc::new(tokio::sync::Mutex::new(mcp_bridge));

        Ok(Kernel {
            orchestrator,
            gateway,
            event_bus: event_bus.clone(),
            state_store,
            container_manager,
            config,
            skill_store,
            supervisor,
            scheduler,
            access_manager,
            program_manager,
            host_tool_validator,
            persona_manager,
            mcp_bridge,
            auth_manager,
        })
    }
}

impl Kernel {
    /// Create a new kernel builder with sensible defaults.
    pub fn builder() -> KernelBuilder {
        KernelBuilder {
            config_path: expand_path("~/.oxios/config.toml"),
            model_id: "anthropic/claude-sonnet-4-20250514".to_string(),
        }
    }
}

/// Expand tilde in paths.
fn expand_path(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(path.replacen("~/", &format!("{home}/"), 1));
        }
    }
    PathBuf::from(path)
}

/// Initialize the MCP bridge from config and environment variables.
async fn init_mcp_bridge(config: &OxiosConfig) -> Result<McpBridge> {
    let mut bridge = McpBridge::new();

    for (name, def) in &config.mcp.servers {
        let mut server = McpServer::new(name, &def.command);
        server.args = def.args.clone();
        server.env = def.env.clone();
        server.enabled = def.enabled;
        bridge.register_server(server);
        tracing::debug!(server = %name, command = %def.command, "Registered MCP server from config");
    }

    // Load from environment: OXIOS_MCP_<NAME>_COMMAND=...
    for (key, value) in std::env::vars() {
        if let Some(name) = key.strip_prefix("OXIOS_MCP_") {
            let name = name.trim_end_matches("_COMMAND");
            if name.is_empty() || config.mcp.servers.contains_key(name) {
                continue;
            }
            let mut server = McpServer::new(name, &value);
            if let Ok(args_str) = std::env::var(format!("OXIOS_MCP_{}_ARGS", name)) {
                server.args = args_str.split_whitespace().map(String::from).collect();
            }
            if let Ok(env_str) = std::env::var(format!("OXIOS_MCP_{}_ENV", name)) {
                for pair in env_str.split(',') {
                    if let Some((k, v)) = pair.split_once('=') {
                        server.env.insert(k.trim().to_string(), v.trim().to_string());
                    }
                }
            }
            bridge.register_server(server);
            tracing::debug!(server = %name, "Registered MCP server from environment");
        }
    }

    Ok(bridge)
}
