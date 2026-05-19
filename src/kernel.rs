//! Kernel assembly — Builder pattern for wiring all Oxios components.
//!
//! This module lives in the binary crate (not oxios-kernel) because
//! it's responsible for *assembling* kernel components, not providing them.
//! The kernel library provides parts; the binary puts them together.

use anyhow::{Context, Result};
use oxios_gateway::Gateway;
use oxios_kernel::{
    access_manager::AccessManager, auth::AuthManager, config::load_config, A2AProtocol,
    AgentRuntime, AgentScheduler, AuditTrail, BasicSupervisor, BudgetManager, CronScheduler,
    EngineProvider, EventBus, GitLayer, HostToolValidator, McpBridge, McpServer, MemoryManager,
    Orchestrator, OxiosConfig, PersonaManager, ProgramManager, ResourceMonitor, SkillStore,
    SpaceManager, Supervisor,
};

use oxios_ouroboros::{OuroborosEngine, OuroborosProtocol, Seed};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

/// Fully assembled Oxios kernel with all components wired together.
///
/// Created via [`Kernel::builder()`]. Fields are private — access
/// through typed methods or [`Kernel::handle()`] for the KernelHandle facade.
pub struct Kernel {
    orchestrator: Arc<Orchestrator>,
    gateway: Arc<Gateway>,
    event_bus: EventBus,
    state_store: Arc<oxios_kernel::state_store::StateStore>,
    config: OxiosConfig,
    skill_store: SkillStore,
    supervisor: Arc<dyn Supervisor>,
    scheduler: Arc<AgentScheduler>,
    access_manager: Arc<parking_lot::Mutex<AccessManager>>,
    program_manager: Arc<ProgramManager>,
    host_tool_validator: HostToolValidator,
    persona_manager: PersonaManager,
    mcp_bridge: Arc<McpBridge>,
    #[allow(dead_code)]
    memory_manager: Arc<MemoryManager>,
    auth_manager: Arc<parking_lot::Mutex<AuthManager>>,
    cron_scheduler: Arc<CronScheduler>,
    git_layer: Arc<GitLayer>,
    audit_trail: Arc<AuditTrail>,
    budget_manager: Arc<BudgetManager>,
    resource_monitor: Arc<ResourceMonitor>,
    space_manager: Arc<SpaceManager>,
    start_time: std::time::Instant,
    /// Cached KernelHandle — created once, reused forever.
    handle_cache: OnceLock<Arc<oxios_kernel::KernelHandle>>,
    /// A2A protocol for inter-agent communication.
    a2a_protocol: Arc<A2AProtocol>,
}

impl Kernel {
    /// Create a new kernel builder with sensible defaults.
    pub fn builder() -> KernelBuilder {
        KernelBuilder {
            config_path: oxios_kernel::config::expand_home("~/.oxios/config.toml"),
        }
    }

    // ── Public accessors ────────────────────────────────────────────────

    /// KernelHandle facade — the primary API for subcommands and plugins.
    ///
    /// Cached after first call. Use this for all kernel operations.
    pub fn handle(&self) -> Arc<oxios_kernel::KernelHandle> {
        self.handle_cache
            .get_or_init(|| {
                Arc::new(oxios_kernel::KernelHandle::new(
                    oxios_kernel::StateApi::new(self.state_store.clone()),
                    oxios_kernel::AgentApi::new(
                        self.supervisor.clone(),
                        self.budget_manager.clone(),
                        self.memory_manager.clone(),
                        Some(self.event_bus.clone()),
                    ),
                    oxios_kernel::SecurityApi::new(
                        self.auth_manager.clone(),
                        self.audit_trail.clone(),
                        self.access_manager.clone(),
                        self.state_store.clone(),
                    ),
                    oxios_kernel::PersonaApi::new(Arc::new(self.persona_manager.clone())),
                    oxios_kernel::ExtensionApi::new(
                        self.program_manager.clone(),
                        Arc::new(self.skill_store.clone()),
                        Arc::new(self.host_tool_validator.clone()),
                    ),
                    oxios_kernel::McpApi::new(self.mcp_bridge.clone()),
                    oxios_kernel::InfraApi::new(
                        self.git_layer.clone(),
                        self.scheduler.clone(),
                        self.cron_scheduler.clone(),
                        self.resource_monitor.clone(),
                        self.event_bus.clone(),
                        self.config.clone(),
                        self.start_time,
                    ),
                    oxios_kernel::SpaceApi::new(self.space_manager.clone(), self.event_bus.clone()),
                    oxios_kernel::ExecApi::new(
                        Arc::new(self.config.exec.clone()),
                        self.access_manager.clone(),
                    ),
                    self.build_browser_api(),
                    oxios_kernel::A2aApi::new(self.a2a_protocol.clone()),
                    oxios_kernel::KnowledgeApi::new(
                        std::path::PathBuf::from(&self.config.kernel.workspace).join("knowledge"),
                        self.memory_manager.clone(),
                    ),
                ))
            })
            .clone()
    }

    /// Gateway reference — for channel registration and message routing.
    pub fn gateway(&self) -> Arc<Gateway> {
        self.gateway.clone()
    }

    /// Build a BrowserApi facade based on feature flag and config.
    #[cfg(feature = "browser")]
    fn build_browser_api(&self) -> oxios_kernel::BrowserApi {
        if self.config.browser.enabled {
            oxios_kernel::BrowserApi::from_config(&self.config.browser.engine)
        } else {
            oxios_kernel::BrowserApi::default()
        }
    }

    /// Build a BrowserApi facade (no-op when browser feature is disabled).
    #[cfg(not(feature = "browser"))]
    fn build_browser_api(&self) -> oxios_kernel::BrowserApi {
        oxios_kernel::BrowserApi::default()
    }

    /// Configuration reference.
    pub fn config(&self) -> &OxiosConfig {
        &self.config
    }

    /// Flush audit trail entries to persistent storage.
    /// Call during graceful shutdown to ensure no entries are lost.
    pub fn flush_audit(&self) -> anyhow::Result<()> {
        self.audit_trail
            .flush(&self.state_store)
            .map_err(|e| anyhow::anyhow!("audit flush failed: {}", e))
    }

    /// Execute a prompt with an optional session ID for multi-turn conversations.
    ///
    /// Pass `Some(session_id)` to continue an existing interview;
    /// pass `None` to start a new session.
    pub async fn execute_prompt_with_session(
        &self,
        prompt: &str,
        session_id: Option<&str>,
    ) -> Result<oxios_kernel::OrchestrationResult> {
        self.orchestrator
            .handle_message("cli", prompt, session_id)
            .await
    }

    /// Register a channel with the gateway.
    pub async fn register_channel(&self, channel: Box<dyn oxios_gateway::Channel>) {
        self.gateway.register(channel).await;
    }

    /// Run the gateway event loop (blocking).
    #[allow(dead_code)]
    pub async fn run_gateway(&self) -> Result<()> {
        self.gateway.run().await
    }

    // ── Initialization helpers (used by default mode only) ─────────────

    /// Initialize default skills from the share directory.
    pub async fn init_default_skills(&self, share_dir: &std::path::Path) -> Result<()> {
        let defaults_dir = share_dir.join("default-skills");
        self.skill_store.init_defaults(&defaults_dir).await?;
        Ok(())
    }

    /// Initialize default programs from the share directory.
    pub async fn init_default_programs(&self, share_dir: &std::path::Path) -> Result<()> {
        let programs_dir = share_dir.join("default-programs");
        if programs_dir.exists() {
            for entry in std::fs::read_dir(&programs_dir)? {
                let entry = entry?;
                let name = entry.file_name().to_str().unwrap_or("").to_string();
                if entry.path().is_dir() && self.program_manager.get_program(&name).await.is_none()
                {
                    if let Err(e) = self.program_manager.install(&entry.path()).await {
                        tracing::warn!(error = %e, program = ?entry.file_name(), "Failed to install default program");
                    }
                }
            }
        }
        Ok(())
    }

    /// Initialize MCP servers from config.
    pub async fn init_mcp_servers(&self) -> Result<()> {
        if !self.config.mcp.servers.is_empty() {
            self.mcp_bridge.initialize_all().await?;
            tracing::info!(
                count = self.config.mcp.servers.len(),
                "MCP servers initialized"
            );
        }
        Ok(())
    }

    /// Start the guardian daemon (background integrity checks).
    pub fn start_guardian(&self) {
        use oxios_kernel::audit_trail::AuditAction;
        let handle = self.handle();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;

                if let Ok(valid) = handle.security.verify_chain() {
                    if !valid {
                        handle.security.audit(
                            "guardian",
                            AuditAction::Other {
                                detail: "AUDIT CHAIN BROKEN".into(),
                            },
                            "guardian",
                        );
                    }
                }

                if handle.infra.is_overloaded() {
                    let snap = handle.infra.resource_snapshot();
                    handle.security.audit(
                        "guardian",
                        AuditAction::Other {
                            detail: format!("OVERLOADED: cpu={:.1}%", snap.cpu_percent),
                        },
                        "guardian",
                    );
                }

                if let Ok(valid) = handle.infra.git_verify() {
                    if !valid {
                        handle.security.audit(
                            "guardian",
                            AuditAction::Other {
                                detail: "GIT REPOSITORY CORRUPTED".into(),
                            },
                            "guardian",
                        );
                    }
                }

                let _ = handle.commit_all("guardian: periodic checkpoint");
            }
        });
    }
}

/// Builder for assembling the Oxios kernel.
pub struct KernelBuilder {
    config_path: PathBuf,
}

impl KernelBuilder {
    /// Set the config file path.
    pub fn config_path(mut self, path: PathBuf) -> Self {
        self.config_path = path;
        self
    }

    /// Assemble all kernel components and wire them together.
    pub async fn build(self) -> Result<Kernel> {
        let config_path = self.config_path;

        let config = if config_path.exists() {
            tracing::info!(path = %config_path.display(), "Loading config");
            load_config(&config_path)?
        } else {
            tracing::info!("No config file found, using defaults");
            OxiosConfig::default()
        };

        let event_bus = EventBus::new(config.kernel.event_bus_capacity);
        let state_store = Arc::new(oxios_kernel::state_store::StateStore::new(PathBuf::from(
            &config.kernel.workspace,
        ))?);

        // Model comes from config, not hardcoded default
        let model_id = &config.engine.default_model;
        let engine_provider = oxios_kernel::OxiEngineProvider::new(model_id);
        let model = engine_provider
            .resolve_model(model_id)
            .context(format!("Failed to resolve model: {}", model_id))?;
        let provider = engine_provider
            .create_provider(&model.provider)
            .context(format!("Failed to create provider: {}", model.provider))?;

        let ouroboros: Arc<dyn OuroborosProtocol> =
            Arc::new(OuroborosEngine::new(Arc::clone(&provider), model));

        let mut access_manager = AccessManager::new();
        if let Some(ref audit_path) = config.security.audit_log_path {
            let expanded = oxios_kernel::config::expand_home(audit_path);
            access_manager = access_manager.with_audit_log_path(expanded.clone());
            tracing::info!(path = %expanded.display(), "Audit log file persistence enabled");
        }
        let access_manager = Arc::new(parking_lot::Mutex::new(access_manager));
        let scheduler = Arc::new(AgentScheduler::new(
            config.scheduler.max_concurrent,
            config.scheduler.rate_limit_per_minute,
            config.scheduler.zombie_timeout_secs,
        ));

        let persona_manager = PersonaManager::new();
        if let Some(p) = persona_manager.first_enabled() {
            ouroboros.set_persona_prompt(Some(p.system_prompt));
            tracing::info!(persona = %p.name, "Active persona set on OuroborosEngine");
        }

        let a2a_protocol = Arc::new(A2AProtocol::new(event_bus.clone()));

        let git_layer = Arc::new(GitLayer::new(
            PathBuf::from(&config.kernel.workspace),
            config.git.auto_commit,
        )?);

        let skills_dir = PathBuf::from(&config.kernel.workspace).join("skills");
        let skill_store = SkillStore::new(skills_dir)?;
        let programs_dir = PathBuf::from(&config.kernel.workspace).join("programs");
        let program_manager = Arc::new(ProgramManager::new(programs_dir));
        program_manager.init().await?;

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

        // ── Pre-create all kernel service objects ──
        // These are needed before KernelHandle creation (for AgentRuntime).
        // Order doesn't matter — they're independent.

        let mut memory_manager = MemoryManager::new(state_store.clone());
        memory_manager.set_git_layer(git_layer.clone());
        let memory_manager = Arc::new(memory_manager);

        let budget_manager = Arc::new(BudgetManager::new());

        let host_tool_validator = HostToolValidator::new(
            config.exec.required_host_tools.clone(),
            config.exec.optional_host_tools.clone(),
        );

        let auth_manager = AuthManager::new();
        // API key auth is now via engine.api_key or ~/.oxi/auth.json
        // No more security.api_keys_path
        let auth_manager = Arc::new(parking_lot::Mutex::new(auth_manager));

        let audit_trail = Arc::new(AuditTrail::new(config.audit.max_entries));

        let mut cron_scheduler =
            CronScheduler::new(state_store.clone(), config.cron.tick_interval_secs);
        cron_scheduler.set_git_layer(git_layer.clone());
        let cron_scheduler = Arc::new(cron_scheduler);

        let resource_monitor = Arc::new(ResourceMonitor::new(
            config.resource_monitor.interval_secs,
            config.resource_monitor.history_max,
        ));

        event_bus.attach_audit_trail(audit_trail.clone());

        // Restore persisted audit entries.
        if let Ok(entries) = state_store.load_audit_entries() {
            if !entries.is_empty() {
                tracing::info!(count = entries.len(), "Restored audit trail entries");
                audit_trail.restore_from(entries);
            }
        }

        // ── Space Management (early — needed for KernelHandle) ──
        let space_manager = SpaceManager::new(state_store.clone(), event_bus.clone()).await?;
        let space_manager = Arc::new(space_manager);

        // ── KernelHandle — the syscall table for agent OS control ──
        // Created inline here because AgentRuntime needs it.
        // Will be cached again in the Kernel instance.
        let kernel_handle: Arc<oxios_kernel::KernelHandle> =
            Arc::new(oxios_kernel::KernelHandle::new(
                oxios_kernel::StateApi::new(state_store.clone()),
                oxios_kernel::AgentApi::new(
                    // Placeholder supervisor — the real one needs AgentRuntime which needs this handle.
                    // AgentApi.supervisor is only used for list/kill, not during tool registration.
                    Arc::new(oxios_kernel::supervisor::NoOpSupervisor),
                    budget_manager.clone(),
                    memory_manager.clone(),
                    None,
                ),
                oxios_kernel::SecurityApi::new(
                    auth_manager.clone(),
                    audit_trail.clone(),
                    access_manager.clone(),
                    state_store.clone(),
                ),
                oxios_kernel::PersonaApi::new(Arc::new(persona_manager.clone())),
                oxios_kernel::ExtensionApi::new(
                    program_manager.clone(),
                    Arc::new(skill_store.clone()),
                    Arc::new(host_tool_validator.clone()),
                ),
                oxios_kernel::McpApi::new(mcp_bridge.clone()),
                oxios_kernel::InfraApi::new(
                    git_layer.clone(),
                    scheduler.clone(),
                    cron_scheduler.clone(),
                    resource_monitor.clone(),
                    event_bus.clone(),
                    config.clone(),
                    std::time::Instant::now(),
                ),
                oxios_kernel::SpaceApi::new(space_manager.clone(), event_bus.clone()),
                oxios_kernel::ExecApi::new(Arc::new(config.exec.clone()), access_manager.clone()),
                build_browser_api_value(&config),
                oxios_kernel::A2aApi::new(a2a_protocol.clone()),
                oxios_kernel::KnowledgeApi::new(
                    PathBuf::from(&config.kernel.workspace).join("knowledge"),
                    memory_manager.clone(),
                ),
            ));

        // Build ToolRetriever for semantic capability discovery.
        let tool_retriever = build_tool_retriever(&program_manager).await;

        let agent_runtime = AgentRuntime::new(provider, model_id, kernel_handle)
            .with_persona_manager(Arc::new(persona_manager.clone()))
            .with_tool_retriever(Arc::new(tool_retriever));

        let supervisor: Arc<dyn Supervisor> =
            Arc::new(BasicSupervisor::new(event_bus.clone(), agent_runtime));

        let lifecycle = oxios_kernel::AgentLifecycleManager::new(
            supervisor.clone(),
            scheduler.clone(),
            access_manager.clone(),
            a2a_protocol.clone(),
            event_bus.clone(),
            config.security.max_execution_time_secs,
        );

        // Register the A2A dispatch handler.
        // When a TaskDelegation arrives, the handler spawns an agent via
        // the lifecycle manager and returns the execution result.
        let dispatch_lifecycle = lifecycle.clone();
        a2a_protocol
            .set_delegation_handler(Arc::new(move |_from, _to, task| {
                let lc = dispatch_lifecycle.clone();
                Box::pin(async move {
                    let seed = Seed {
                        id: task.task_id,
                        goal: task.description.clone(),
                        constraints: vec![],
                        acceptance_criteria: vec!["Task completes successfully".into()],
                        ontology: vec![],
                        created_at: chrono::Utc::now(),
                        generation: 0,
                        parent_seed_id: None,
                        cspace_hint: None,
                    };
                    match lc
                        .spawn_and_run(&seed, oxios_kernel::scheduler::Priority::Normal)
                        .await
                    {
                        Ok(result) => Ok(serde_json::json!({
                            "output": result.output,
                            "success": result.success,
                            "steps": result.steps_completed,
                        })),
                        Err(e) => Ok(serde_json::json!({
                            "error": e.to_string(),
                            "success": false,
                        })),
                    }
                })
            }))
            .await;

        let mut orchestrator = Orchestrator::with_config(
            ouroboros,
            event_bus.clone(),
            state_store.clone(),
            lifecycle,
            config.orchestrator.clone(),
        );
        orchestrator.set_git_layer(git_layer.clone());
        orchestrator.set_a2a(a2a_protocol.clone());
        orchestrator.set_space_manager(space_manager.clone());
        let orchestrator = Arc::new(orchestrator);

        let gateway = Arc::new(Gateway::new(orchestrator.clone()));

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
            space_manager,
            start_time: std::time::Instant::now(),
            handle_cache: OnceLock::new(),
            a2a_protocol,
        })
    }
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
                        server
                            .env
                            .insert(k.trim().to_string(), v.trim().to_string());
                    }
                }
            }
            bridge.register_server(server);
            tracing::debug!(server = %name, "Registered MCP server from environment");
        }
    }

    Ok(bridge)
}

/// Build a ToolRetriever with all OS tools and installed programs indexed.
async fn build_tool_retriever(
    pm: &Arc<ProgramManager>,
) -> oxios_kernel::tools::retrieval::ToolRetriever {
    use oxios_kernel::embedding::TfIdfEmbeddingProvider;
    use oxios_kernel::tools::retrieval::ToolEntry;

    let embedder = Arc::new(TfIdfEmbeddingProvider);
    let mut retriever = oxios_kernel::tools::retrieval::ToolRetriever::new(embedder);

    // Index built-in OS tools.
    let builtin_tools = vec![
        (
            "exec",
            "os-tool",
            "Execute shell commands or structured binaries in workspace",
        ),
        ("read", "os-tool", "Read file contents"),
        ("write", "os-tool", "Write content to files"),
        ("edit", "os-tool", "Make precise text edits in files"),
        ("grep", "os-tool", "Search file contents with regex"),
        ("find", "os-tool", "Find files by name or pattern"),
        ("ls", "os-tool", "List directory contents"),
        ("web_search", "os-tool", "Search the web for information"),
        ("memory_read", "os-tool", "Recall persistent memories"),
        ("memory_write", "os-tool", "Store persistent memories"),
        ("memory_search", "os-tool", "Semantic search over memories"),
        (
            "browser",
            "os-tool",
            "Headless browser for web automation and scraping",
        ),
    ];

    for (name, category, desc) in builtin_tools {
        retriever
            .index_tool(ToolEntry {
                name: name.to_string(),
                category: category.to_string(),
                description: desc.to_string(),
                skill_path: None,
                command: None,
            })
            .await;
    }

    // Index installed programs.
    let programs = pm.list_enabled().await;
    for program in &programs {
        let desc = program.meta.description.clone();
        retriever
            .index_tool(ToolEntry {
                name: format!("program:{}", program.meta.name),
                category: "program".to_string(),
                description: desc,
                skill_path: Some(format!("programs/{}/SKILL.md", program.meta.name)),
                command: Some(program.meta.name.clone()),
            })
            .await;

        // Index individual program tools.
        for tool_def in &program.meta.tools {
            retriever
                .index_tool(ToolEntry {
                    name: format!("{}:{}", program.meta.name, tool_def.name),
                    category: "program-tool".to_string(),
                    description: tool_def.description.clone(),
                    skill_path: Some(format!("programs/{}/SKILL.md", program.meta.name)),
                    command: Some(tool_def.command.clone()),
                })
                .await;
        }
    }

    tracing::info!(count = retriever.len(), "ToolRetriever indexed");
    retriever
}

/// Build a BrowserApi from config (standalone, for use during KernelBuilder::build).
#[cfg(feature = "browser")]
fn build_browser_api_value(config: &OxiosConfig) -> oxios_kernel::BrowserApi {
    if config.browser.enabled {
        oxios_kernel::BrowserApi::from_config(&config.browser.engine)
    } else {
        oxios_kernel::BrowserApi::default()
    }
}

#[cfg(not(feature = "browser"))]
fn build_browser_api_value(_config: &OxiosConfig) -> oxios_kernel::BrowserApi {
    oxios_kernel::BrowserApi::default()
}
