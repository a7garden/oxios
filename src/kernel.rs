//! Kernel assembly — Builder pattern for wiring all Oxios components.
//!
//! This module lives in the binary crate (not oxios-kernel) because
//! it's responsible for *assembling* kernel components, not providing them.
//! The kernel library provides parts; the binary puts them together.

use anyhow::{Context, Result};
use oxios_gateway::Gateway;
use oxios_kernel::{
    access_manager::AccessManager, auth::AuthManager, config::load_config, A2AProtocol,
    AgentRuntime, AgentScheduler, AuditTrail, BasicSupervisor, BudgetManager, ClawHubClient,
    ClawHubInstaller, CronScheduler, EventBus, GitLayer,
    McpBridge, McpServer, MarketplaceApi, MemoryManager,
    Orchestrator, OxiosConfig, OxiosEngine, PersonaManager, ResourceMonitor,
    SkillManager, SpaceManager, Supervisor,
    TfIdfEmbeddingProvider,
};
use oxios_markdown::knowledge::FileChange;
use oxios_markdown::KnowledgeBase;

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
    skill_manager: Arc<SkillManager>,
    supervisor: Arc<dyn Supervisor>,
    scheduler: Arc<AgentScheduler>,
    access_manager: Arc<parking_lot::Mutex<AccessManager>>,
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
    /// Path to config.toml (for persistence).
    config_path: PathBuf,
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
                // KnowledgeBase — single source of truth (RFC-003)
                // Shared between KernelHandle.knowledge and KnowledgeLens.
                let knowledge = Arc::new(
                    KnowledgeBase::new(
                        std::path::PathBuf::from(&self.config.kernel.workspace)
                            .join("knowledge"),
                    )
                    .expect("KnowledgeBase init failed"),
                );
                let knowledge_lens = Arc::new(
                    oxios_kernel::KnowledgeLens::new(
                        knowledge.clone(),
                        self.memory_manager.clone(),
                    )
                    .expect("KnowledgeLens init failed"),
                );

                // Git auto-commit for knowledge files (async channel pattern)
                // Same pattern as KnowledgeLens — non-blocking to avoid delaying HTTP responses.
                {
                    let git = self.git_layer.clone();
                    let kb_root = knowledge.root();
                    let git_root = git.root().to_path_buf();
                    let prefix = kb_root
                        .strip_prefix(&git_root)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| "knowledge".to_string());

                    let (git_tx, mut git_rx) =
                        tokio::sync::mpsc::channel::<(String, FileChange)>(64);

                    // Register callback — spawns a task to avoid blocking note_write()
                    knowledge.on_file_change(move |path: &str, change: FileChange| {
                        let tx = git_tx.clone();
                        let path = path.to_string();
                        tokio::spawn(async move {
                            let _ = tx.send((path, change)).await;
                        });
                    });

                    // Background consumer — commits knowledge changes to git
                    tokio::spawn(async move {
                        while let Some((path, change)) = git_rx.recv().await {
                            if !git.is_enabled() {
                                continue;
                            }
                            let rel = format!("{}/{}", prefix, path);
                            let msg = match &change {
                                FileChange::Created(p) => format!("knowledge: create {}", p),
                                FileChange::Updated(p) => format!("knowledge: update {}", p),
                                FileChange::Deleted(p) => format!("knowledge: delete {}", p),
                                FileChange::Moved { old, new } => {
                                    format!("knowledge: rename {} → {}", old, new)
                                }
                            };
                            match change {
                                FileChange::Deleted(_) => {
                                    if let Err(e) = git.remove_file(&rel, &msg) {
                                        tracing::warn!(error = %e, "knowledge git delete failed");
                                    }
                                }
                                FileChange::Moved { old, .. } => {
                                    let old_rel = format!("{}/{}", prefix, old);
                                    let _ = git.remove_file(&old_rel, &msg);
                                    let _ = git.commit_file(&rel, &msg);
                                }
                                _ => {
                                    if let Err(e) = git.commit_file(&rel, &msg) {
                                        tracing::warn!(error = %e, "knowledge git commit failed");
                                    }
                                }
                            }
                        }
                    });
                }

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
                        Arc::clone(&self.skill_manager),
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
                    // EngineApi — LLM providers, models, config
                    oxios_kernel::EngineApi::new(
                        Arc::new(parking_lot::RwLock::new(self.config.clone())),
                        self.config_path.clone(),
                    ),
                    knowledge,
                    knowledge_lens,
                    self.build_marketplace_api(),
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

    /// Build a MarketplaceApi (ClawHub) from config.
    fn build_marketplace_api(&self) -> MarketplaceApi {
        let workspace = PathBuf::from(&self.config.kernel.workspace);
        let skills_dir = workspace.join("skills");
        let config = &self.config.marketplace;

        let client = match ClawHubClient::new(config.base_url.clone()) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "Invalid marketplace.base_url, using default");
                ClawHubClient::new(Some("https://clawhub.ai".to_string())).unwrap()
            }
        };

        let installer = ClawHubInstaller::new(
            skills_dir,
            workspace,
            config.base_url.clone(),
        );

        MarketplaceApi::new(Arc::new(installer), Arc::new(client))
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
        self.skill_manager.init().await?;
        let _ = defaults_dir; // TODO: wire bundled defaults dir
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

        // Daily health check: web UI update, self-update check.
        self.start_daily_health_check();
    }

    /// Start the daily health check loop.
    ///
    /// Runs at 03:00 AM every day (user's local time) via cron expression.
    /// First tick is calculated to land on the next 3 AM, then every 24h after.
    fn start_daily_health_check(&self) {
        tokio::spawn(async move {
            let now = chrono::Local::now();
            let mut next = now.date_naive()
                .and_hms_opt(3, 0, 0)
                .unwrap()
                .and_local_timezone(chrono::Local)
                .unwrap();
            if next <= now {
                next += chrono::Duration::days(1);
            }

            let delay_secs = (next - now).num_seconds().max(0) as u64;
            tracing::info!(
                next_check = %next.format("%Y-%m-%d %H:%M"),
                "Daily health check scheduled at 03:00"
            );

            tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;

            if let Err(e) = daily_health_check().await {
                tracing::warn!(error = %e, "Daily health check failed");
            }

            let mut interval = tokio::time::interval(std::time::Duration::from_secs(86400));
            loop {
                interval.tick().await;
                if let Err(e) = daily_health_check().await {
                    tracing::warn!(error = %e, "Daily health check failed");
                }
            }
        });
    }
}

/// Daily health check logic.
async fn daily_health_check() -> anyhow::Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home dir"))?;
    let web_dist = home.join(".oxios").join("web").join("dist");
    let version_file = home.join(".oxios").join("web").join("version");

    // Fetch latest release tag from GitHub
    let client = reqwest::Client::builder()
        .user_agent("oxios-health")
        .build()?;

    let resp: serde_json::Value = client
        .get("https://api.github.com/repos/a7garden/oxios/releases/latest")
        .send()
        .await?
        .json()
        .await?;

    let latest_tag = resp["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no tag_name in response"))?;

    let current_version = std::fs::read_to_string(&version_file)
        .unwrap_or_default()
        .trim()
        .to_string();

    let needs_download = !web_dist.join("index.html").is_file() || current_version != latest_tag;

    if !needs_download {
        tracing::debug!("Daily health check: web UI up to date ({})", latest_tag);
        return Ok(());
    }

    tracing::info!(
        current = %current_version,
        latest = %latest_tag,
        "Updating web UI..."
    );

    // Download web-dist.zip
    let url = format!(
        "https://github.com/a7garden/oxios/releases/download/{}/web-dist.zip",
        latest_tag
    );
    let bytes = client.get(&url).send().await?.bytes().await?;

    // Clear and extract
    if web_dist.exists() {
        std::fs::remove_dir_all(&web_dist)?;
    }
    std::fs::create_dir_all(&web_dist)?;

    let reader = std::io::Cursor::new(bytes.as_ref());
    let mut archive = zip::ZipArchive::new(reader)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => web_dist.join(path),
            None => continue,
        };
        if file.is_dir() {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }

    // Write version file
    if let Some(parent) = version_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&version_file, latest_tag)?;

    tracing::info!(version = %latest_tag, "Daily health check: web UI updated");
    Ok(())
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

    /// Create the appropriate embedding provider based on config.
    ///
    /// - `"tfidf"` → TfIdfEmbeddingProvider (default, zero-dependency)
    /// - `"gguf"` → GGUF-based embedding (requires `embedding-gguf` feature)
    fn create_embedding_provider(config: &OxiosConfig) -> Arc<dyn oxios_kernel::EmbeddingProvider> {
        let emb_config = &config.memory.embedding;

        match emb_config.provider.as_str() {
            "gguf" => {
                #[cfg(feature = "embedding-gguf")]
                {
                    use oxios_kernel::{EmbeddingDimension, GgufEmbeddingProvider};

                    let model_dir = oxios_kernel::embedding::gguf::GgufModelLoader
                        ::model_dir_for_workspace(
                            Path::new(&config.kernel.workspace)
                        );
                    let dim = match emb_config.dimension {
                        128 => EmbeddingDimension::Dim128,
                        512 => EmbeddingDimension::Dim512,
                        768 => EmbeddingDimension::Dim768,
                        _ => EmbeddingDimension::Dim256,
                    };
                    tracing::info!(
                        dir = %model_dir.display(),
                        dim = emb_config.dimension,
                        "Using GGUF EmbeddingGemma provider"
                    );
                    Arc::new(GgufEmbeddingProvider::new(
                        model_dir,
                        dim,
                        emb_config.model_ttl_secs,
                    ))
                }

                #[cfg(not(feature = "embedding-gguf"))]
                {
                    tracing::warn!(
                        "GGUF embedding requested but embedding-gguf feature not enabled. \
                         Falling back to TF-IDF."
                    );
                    Arc::new(TfIdfEmbeddingProvider)
                }
            }
            _ => {
                tracing::debug!("Using TF-IDF embedding provider");
                Arc::new(TfIdfEmbeddingProvider)
            }
        }
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
        let engine = Arc::new(OxiosEngine::new(model_id));
        let model = engine
            .resolve_model(model_id)
            .context(format!("Failed to resolve model: {}", model_id))?;
        let provider = engine
            .create_provider(&model.provider)
            .context(format!("Failed to create provider: {}", model.provider))?;

        let ouroboros: Arc<dyn OuroborosProtocol> =
            Arc::new(OuroborosEngine::new(Arc::clone(&provider), model.clone()));

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
        let bundled_dir = PathBuf::from(&config.kernel.workspace).join("share/skills");
        let skill_manager = Arc::new(SkillManager::new(skills_dir, bundled_dir));

        let mcp_bridge = Arc::new(init_mcp_bridge(&config).await?);

        // ── Pre-create all kernel service objects ──
        // These are needed before KernelHandle creation (for AgentRuntime).
        // Order doesn't matter — they're independent.

        let mut memory_manager = MemoryManager::new(state_store.clone());
        memory_manager.set_git_layer(git_layer.clone());

        // ── RFC-012: SQLite memory backend ──
        // When enabled, initialize the SQLite database and attach it to the memory manager.
        #[cfg(feature = "sqlite-memory")]
        if config.memory.sqlite.enabled {
            use oxios_kernel::{MemoryDatabase, SqliteMemoryStore};

            let sqlite_config = &config.memory.sqlite;
            let db_path = if sqlite_config.path.is_empty() {
                PathBuf::from(&config.kernel.workspace).join("memory.db")
            } else {
                oxios_kernel::config::expand_home(&sqlite_config.path)
            };

            match MemoryDatabase::open(&db_path, sqlite_config.embedding_dim) {
                Ok(db) => {
                    let db = Arc::new(db);

                    // Select embedding provider based on config
                    let embedding: Arc<dyn oxios_kernel::EmbeddingProvider> =
                        Self::create_embedding_provider(&config);

                    let sqlite_store = SqliteMemoryStore::new(db, embedding);

                    // Run JSON → SQLite migration (one-time, best effort)
                    let workspace_dir = PathBuf::from(&config.kernel.workspace);
                    if let Err(e) = sqlite_store.migrate_if_needed(&workspace_dir) {
                        tracing::warn!(error = %e, "Memory migration failed (non-fatal)");
                    }

                    memory_manager.set_sqlite_store(Arc::new(sqlite_store));
                    tracing::info!(
                        path = %db_path.display(),
                        dim = sqlite_config.embedding_dim,
                        "SQLite memory backend initialized"
                    );

                    // Prefetch the embedding model in the background so it's ready
                    // before the first search. Non-blocking — errors are logged.
                    if config.memory.embedding.provider == "gguf" {
                        #[cfg(feature = "embedding-gguf")]
                        oxios_kernel::embedding::gguf::GgufModelLoader::spawn_prefetch(
                            oxios_kernel::embedding::gguf::GgufModelLoader::model_dir_for_workspace(
                                Path::new(&config.kernel.workspace),
                            ),
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "Failed to open SQLite memory database, falling back to JSON"
                    );
                }
            }
        }

        let memory_manager = Arc::new(memory_manager);

        // ── RFC-008: Dream process for background memory consolidation ──
        {
            let consolidation = &config.memory.consolidation;
            if consolidation.dream_enabled {
                let dream_config = oxios_kernel::memory::dream::DreamConfig::from_consolidation(consolidation);
                let space_dir = PathBuf::from(&config.kernel.workspace);
                let dream = Arc::new(oxios_kernel::DreamProcess::new(
                    memory_manager.clone(),
                    dream_config,
                    space_dir,
                ));
                dream.spawn_dream_task();
                tracing::info!("Dream process spawned for background memory consolidation");
            }
        }

        let budget_manager = Arc::new(BudgetManager::new());

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
                    Arc::clone(&skill_manager),
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
                // EngineApi — LLM providers, models, config
                oxios_kernel::EngineApi::new(
                    Arc::new(parking_lot::RwLock::new(config.clone())),
                    config_path.clone(),
                ),
                // KnowledgeBase — single source of truth (RFC-003)
                Arc::new(
                    KnowledgeBase::new(PathBuf::from(&config.kernel.workspace).join("knowledge"))
                        .expect("KnowledgeBase init failed"),
                ),
                // KnowledgeLens — semantic overlay, shares same KnowledgeBase
                Arc::new(
                    oxios_kernel::KnowledgeLens::new(
                        Arc::new(
                            KnowledgeBase::new(
                                PathBuf::from(&config.kernel.workspace).join("knowledge"),
                            )
                            .expect("KnowledgeBase init failed"),
                        ),
                        memory_manager.clone(),
                    )
                    .expect("KnowledgeLens init failed"),
                ),
                build_marketplace_api_value(&config),
            ));

        // Build ToolRetriever for semantic capability discovery.
        let tool_retriever = build_tool_retriever(&*skill_manager).await;

        let agent_runtime = AgentRuntime::new(Arc::clone(&engine), model_id, kernel_handle)
            .with_persona_manager(Arc::new(persona_manager.clone()))
            .with_tool_retriever(Arc::new(tool_retriever))
            .with_config({
                // Resolve API key from CredentialStore based on the model's provider.
                let provider_name = model.provider.as_str();
                let config_api_key = config.engine.api_key.as_deref();
                let api_key =
                    oxios_kernel::CredentialStore::resolve(provider_name, config_api_key)
                        .map(|(key, _)| key);

                oxios_kernel::agent_runtime::AgentRuntimeConfig {
                    model_id: model_id.clone(),
                    api_key,
                    provider_options: config.engine.provider_options.clone(),
                    ..Default::default()
                }
            });

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
                        original_request: String::new(),
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
            skill_manager,
            supervisor,
            scheduler,
            access_manager,
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
            config_path,
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

/// Build a ToolRetriever with all OS tools and installed skills indexed.
async fn build_tool_retriever(
    sm: &SkillManager,
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

    // Index installed skills.
    let skills = sm.list_skills().await;
    for entry in &skills {
        let desc = entry.skill.description.clone();
        retriever
            .index_tool(ToolEntry {
                name: format!("skill:{}", entry.skill.name),
                category: "skill".to_string(),
                description: desc,
                skill_path: Some(format!("skills/{}/SKILL.md", entry.skill.name)),
                command: None,
            })
            .await;
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

/// Build a MarketplaceApi from the Kernel instance (used after Kernel construction).
fn build_marketplace_api_value(config: &OxiosConfig) -> MarketplaceApi {
    let workspace = PathBuf::from(&config.kernel.workspace);
    let skills_dir = workspace.join("skills");
    let client = ClawHubClient::new(config.marketplace.base_url.clone()).unwrap_or_else(|_| {
        tracing::warn!("Invalid marketplace.base_url, using default");
        ClawHubClient::new(Some("https://clawhub.ai".to_string())).unwrap()
    });
    let installer = ClawHubInstaller::new(skills_dir, workspace, config.marketplace.base_url.clone());
    MarketplaceApi::new(Arc::new(installer), Arc::new(client))
}
