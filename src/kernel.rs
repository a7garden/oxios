//! Kernel assembly — Builder pattern for wiring all Oxios components.
//!
//! This module lives in the binary crate (not oxios-kernel) because
//! it's responsible for *assembling* kernel components, not providing them.
//! The kernel library provides parts; the binary puts them together.

use anyhow::{Context, Result};
use oxios_gateway::Gateway;
use oxios_kernel::{
    access_manager::AccessManager, auth::AuthManager, config::load_config,
    A2AProtocol, AgentRuntime, BasicSupervisor,
    CronScheduler, EngineProvider, EventBus, GitLayer, HostToolValidator,
    McpBridge, McpServer, MemoryManager, Orchestrator, OxiosConfig, PersonaManager,
    ProgramManager, SkillStore, AgentScheduler, Supervisor,
    AuditTrail, BudgetManager, ResourceMonitor,
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
    pub mcp_bridge: Arc<McpBridge>,
    /// Memory manager for cross-session agent memory.
    #[allow(dead_code)] // Used via AgentRuntime
    pub memory_manager: Arc<MemoryManager>,
    /// API key authentication manager.
    pub auth_manager: Arc<parking_lot::Mutex<AuthManager>>,
    /// Cron job scheduler for time-based task execution.
    pub cron_scheduler: Arc<CronScheduler>,
    /// Git-based version control layer for state persistence.
    pub git_layer: Arc<GitLayer>,
    /// Audit trail for tamper-evident event logging.
    pub audit_trail: Arc<AuditTrail>,
    /// Budget manager for agent-level token/call budgets.
    pub budget_manager: Arc<BudgetManager>,
    /// Resource monitor for system metrics.
    pub resource_monitor: Arc<ResourceMonitor>,
    /// Kernel start time for uptime tracking.
    pub start_time: std::time::Instant,
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
        let mut access_manager = AccessManager::new();
        if let Some(ref audit_path) = config.security.audit_log_path {
            let expanded = expand_path(audit_path);
            access_manager = access_manager.with_audit_log_path(expanded.clone());
            tracing::info!(path = %expanded.display(), "Audit log file persistence enabled");
        }
        let access_manager = Arc::new(parking_lot::Mutex::new(access_manager));
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

        // ── Git version control layer (created early for wiring to other components) ──
        let git_layer = Arc::new(GitLayer::new(
            PathBuf::from(&config.kernel.workspace),
            config.git.auto_commit,
        )?);

        // ── Skills & programs ──
        let skills_dir = PathBuf::from(&config.kernel.workspace).join("skills");
        let skill_store = SkillStore::new(skills_dir)?;
        let programs_dir = PathBuf::from(&config.kernel.workspace).join("programs");
        let program_manager = Arc::new(ProgramManager::new(programs_dir));
        program_manager.init().await?;

        // ── MCP bridge (register program MCP servers before Arc wrapping) ──
        let mcp_bridge = init_mcp_bridge(&config).await?;
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
        let mcp_bridge = Arc::new(mcp_bridge);

        // ── Agent runtime ──
        let agent_runtime = AgentRuntime::new(provider, model_id)
            .with_program_manager(Arc::clone(&program_manager))
            .with_oxios_config(config.clone())
            .with_persona_manager(Arc::new(persona_manager.clone()))
            .with_mcp_bridge(mcp_bridge.clone());

        // ── Memory manager ──
        let mut memory_manager = MemoryManager::new(state_store.clone());
        memory_manager.set_git_layer(git_layer.clone());
        let memory_manager = Arc::new(memory_manager);

        // ── Agent runtime with memory ──
        let agent_runtime = agent_runtime
            .with_memory_manager(memory_manager.clone());

        // ── Supervisor ──
        let supervisor: Arc<dyn Supervisor> = Arc::new(BasicSupervisor::new(
            event_bus.clone(),
            agent_runtime,
        ));

        // ── Agent lifecycle manager ──
        let lifecycle = oxios_kernel::AgentLifecycleManager::new(
            supervisor.clone(),
            scheduler.clone(),
            access_manager.clone(),
            a2a_protocol.clone(),
            event_bus.clone(),
            config.security.max_execution_time_secs,
        );

        // ── Orchestrator ──
        let mut orchestrator = Orchestrator::new(
            ouroboros,
            event_bus.clone(),
            state_store.clone(),
            lifecycle,
        );
        orchestrator.set_git_layer(git_layer.clone());
        let orchestrator = Arc::new(orchestrator);

        // ── Gateway ──
        let gateway = Gateway::new(orchestrator.clone());

        // ── Host tool validator ──
        let host_tool_validator = HostToolValidator::new(
            config.exec.required_host_tools.clone(),
            config.exec.optional_host_tools.clone(),
        );

        // ── Auth manager ──
        let mut auth_manager = AuthManager::new();
        let api_keys_path = PathBuf::from(&config.security.api_keys_path);
        if let Err(e) = auth_manager.load_from_file(&api_keys_path) {
            tracing::debug!(error = %e, "No API keys file loaded (this is normal on first run)");
        }
        let auth_manager = Arc::new(parking_lot::Mutex::new(auth_manager));

        // ── Cron scheduler ──
        let mut cron_scheduler = CronScheduler::new(
            state_store.clone(),
            config.cron.tick_interval_secs,
        );
        cron_scheduler.set_git_layer(git_layer.clone());
        let cron_scheduler = Arc::new(cron_scheduler);

        // ── Audit trail ──
        let audit_trail = Arc::new(AuditTrail::new(config.audit.max_entries));

        // ── Budget manager ──
        let budget_manager = Arc::new(BudgetManager::new());

        // ── Resource monitor ──
        let resource_monitor = Arc::new(ResourceMonitor::new(
            config.resource_monitor.interval_secs,
            config.resource_monitor.history_max,
        ));

        // Wire audit trail to event bus
        event_bus.attach_audit_trail(audit_trail.clone());

        Ok(Kernel {
            orchestrator,
            gateway,
            event_bus: event_bus.clone(),
            state_store,
            config,
            skill_store,
            supervisor,
            scheduler,
            access_manager,
            program_manager,
            host_tool_validator,
            persona_manager,
            mcp_bridge,
            memory_manager,
            auth_manager,
            cron_scheduler,
            git_layer,
            audit_trail,
            budget_manager,
            resource_monitor,
            start_time: std::time::Instant::now(),
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

    pub fn start_guardian(&self) {
        use oxios_kernel::audit_trail::AuditAction;
        let handle = self.handle();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;

                // Audit chain integrity
                if let Ok(valid) = handle.security.verify_chain() {
                    if !valid {
                        handle.security.audit("guardian", AuditAction::Other { detail: "AUDIT CHAIN BROKEN".into() }, "guardian");
                    }
                }

                // Resource check
                if handle.infra.is_overloaded() {
                    let snap = handle.infra.resource_snapshot();
                    handle.security.audit("guardian", AuditAction::Other { detail: format!("OVERLOADED: cpu={:.1}%", snap.cpu_percent) }, "guardian");
                }

                // Git integrity
                if let Ok(valid) = handle.infra.git_verify() {
                    if !valid {
                        handle.security.audit("guardian", AuditAction::Other { detail: "GIT REPOSITORY CORRUPTED".into() }, "guardian");
                    }
                }

                // Periodic checkpoint
                let _ = handle.commit_all("guardian: periodic checkpoint");
            }
        });
    }

    /// Create a KernelHandle facade for use by other crates.
    pub fn handle(&self) -> Arc<oxios_kernel::KernelHandle> {
        Arc::new(oxios_kernel::KernelHandle::from_subsystems(
            self.state_store.clone(),
            self.event_bus.clone(),
            self.supervisor.clone(),
            self.scheduler.clone(),
            self.memory_manager.clone(),
            self.git_layer.clone(),
            self.audit_trail.clone(),
            self.budget_manager.clone(),
            self.resource_monitor.clone(),
            self.cron_scheduler.clone(),
            self.program_manager.clone(),
            Arc::new(self.skill_store.clone()),
            Arc::new(self.persona_manager.clone()),
            self.mcp_bridge.clone(),
            self.auth_manager.clone(),
            self.access_manager.clone(),
            Arc::new(self.host_tool_validator.clone()),
            self.config.clone(),
            self.start_time,
        ))
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
    let bridge = McpBridge::new();

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
