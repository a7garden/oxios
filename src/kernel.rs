//! Kernel assembly — Builder pattern for wiring all Oxios components.
//!
//! This module lives in the binary crate (not oxios-kernel) because
//! it's responsible for *assembling* kernel components, not providing them.
//! The kernel library provides parts; the binary puts them together.

use anyhow::{Context, Result};
use oxios_gateway::Gateway;
use oxios_kernel::{
    A2AProtocol, AgentRuntime, AuditPersistence, AuditTrail, BasicSupervisor, BudgetManager,
    ClawHubClient, ClawHubInstaller, CronScheduler, EngineHandle, EventBus, GitLayer,
    HnswMemoryIndex, MarketplaceApi, McpBridge, McpServer, MemoryManager, Orchestrator,
    OxiosConfig, OxiosEngine, PersonaManager, ProjectManager, ResourceMonitor, SkillManager,
    SkillsShClient, SkillsShInstaller, SubsystemState, Supervisor, access_manager::AccessManager,
    auth::AuthManager, config::load_config,
};
use oxios_markdown::KnowledgeBase;
use oxios_markdown::knowledge::FileChange;

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
    /// RFC-031: the shared QuotaTracker (self-tracker + recalibration).
    quota_tracker: Arc<oxios_kernel::QuotaTracker>,
    /// RFC-031: the TokenMaxer orchestrator (drain loop).
    token_maxer: Arc<oxios_kernel::TokenMaxer>,
    resource_monitor: Arc<ResourceMonitor>,
    project_manager: Option<Arc<ProjectManager>>,
    /// Mount manager (RFC-025 path aliases). `None` when SQLite memory is off.
    /// Wired into the lazily-cached handle so `/api/mounts` and the mount tool
    /// see live data — without this the API's handle has `mounts = None` even
    /// though the orchestrator holds a separate `Arc<MountManager>`.
    mount_manager: Option<Arc<oxios_kernel::MountManager>>,
    start_time: std::time::Instant,
    /// Path to config.toml (for persistence).
    config_path: PathBuf,
    /// Cached KernelHandle — created once, reused forever.
    handle_cache: OnceLock<Arc<oxios_kernel::KernelHandle>>,
    /// A2A protocol for inter-agent communication.
    a2a_protocol: Arc<A2AProtocol>,
    /// Hot-swappable engine reference — shared between EngineApi and AgentRuntime.
    engine_handle: Arc<EngineHandle>,
    /// SQLite-backed agent history query index.
    #[cfg(feature = "sqlite-memory")]
    agent_log_db: Option<Arc<oxios_kernel::agent_log_db::AgentLogDb>>,
    /// RFC-025 Phase 5: cancellation sender for the Mount auto-promotion
    /// scanner (Promo-6). Sending `true` breaks the scan loop's `select!`.
    /// `None` when the scanner is disabled. Kept on the `Kernel` so the
    /// sender stays alive (otherwise `watch::Receiver::changed()` resolves
    /// immediately on sender-drop and would abort the loop) and so a future
    /// graceful shutdown can trigger it.
    #[allow(dead_code)] // wired via `shutdown_promotion_scanner` on graceful shutdown
    promo_shutdown_tx: Option<tokio::sync::watch::Sender<bool>>,
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
                        std::path::PathBuf::from(&self.config.kernel.workspace).join("knowledge"),
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
                            let rel = format!("{prefix}/{path}");
                            let msg = match &change {
                                FileChange::Created(p) => format!("knowledge: create {p}"),
                                FileChange::Updated(p) => format!("knowledge: update {p}"),
                                FileChange::Deleted(p) => format!("knowledge: delete {p}"),
                                FileChange::Moved { old, new } => {
                                    format!("knowledge: rename {old} → {new}")
                                }
                            };
                            match change {
                                FileChange::Deleted(_) => {
                                    if let Err(e) = git.remove_file(&rel, &msg) {
                                        tracing::warn!(error = %e, "knowledge git delete failed");
                                    }
                                }
                                FileChange::Moved { old, .. } => {
                                    let old_rel = format!("{prefix}/{old}");
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

                let mut agent_api = oxios_kernel::AgentApi::new(
                    self.supervisor.clone(),
                    self.budget_manager.clone(),
                    self.memory_manager.clone(),
                    Some(self.event_bus.clone()),
                );
                agent_api.set_state_store(self.state_store.clone());

                #[cfg(feature = "sqlite-memory")]
                if let Some(ref db) = self.agent_log_db {
                    agent_api.set_agent_log_db(db.clone());
                }

                let kh = oxios_kernel::KernelHandle::new(
                    oxios_kernel::StateApi::new(self.state_store.clone()),
                    agent_api,
                    oxios_kernel::SecurityApi::new(
                        self.auth_manager.clone(),
                        self.audit_trail.clone(),
                        self.access_manager.clone(),
                        self.state_store.clone(),
                    ),
                    oxios_kernel::PersonaApi::new(Arc::new(self.persona_manager.clone())),
                    oxios_kernel::ExtensionApi::new(Arc::clone(&self.skill_manager)),
                    oxios_kernel::McpApi::new(self.mcp_bridge.clone()),
                    oxios_kernel::InfraApi::new(
                        self.git_layer.clone(),
                        self.cron_scheduler.clone(),
                        self.resource_monitor.clone(),
                        self.event_bus.clone(),
                        self.config.clone(),
                        self.start_time,
                    ),
                    self.project_manager
                        .clone()
                        .map(oxios_kernel::ProjectApi::new),
                    oxios_kernel::ExecApi::new(
                        Arc::new(parking_lot::RwLock::new(self.config.exec.clone())),
                        self.access_manager.clone(),
                    ),
                    oxios_kernel::A2aApi::new(self.a2a_protocol.clone()),
                    // EngineApi — LLM providers, models, config + routing stats + engine hot-swap
                    oxios_kernel::EngineApi::new(
                        Arc::new(parking_lot::RwLock::new(self.config.clone())),
                        self.config_path.clone(),
                        Arc::new(oxios_kernel::RoutingStats::new()),
                        Arc::clone(&self.engine_handle),
                    ),
                    knowledge,
                    knowledge_lens,
                    self.build_marketplace_api(),
                    self.build_calendar_api(),
                    self.build_email_api(),
                    oxios_kernel::PtyApi::new(Arc::new(parking_lot::RwLock::new(
                        self.config.pty.clone(),
                    ))),
                );
                // RFC-025: attach MountApi to the handle the HTTP API and CLI
                // actually use. The orchestrator gets its own Arc directly; this
                // facade is what `/api/mounts` reads (`state.kernel.mounts`).
                let kh = if let Some(mm) = &self.mount_manager {
                    kh.with_mounts(oxios_kernel::MountApi::new(mm.clone()))
                } else {
                    kh
                };
                let kh = kh.with_token_maxing(oxios_kernel::TokenMaxingApi::new(
                    self.quota_tracker.clone(),
                    self.token_maxer.clone(),
                ));
                Arc::new(kh)
            })
            .clone()
    }

    /// Gateway reference — for channel registration and message routing.
    pub fn gateway(&self) -> Arc<Gateway> {
        self.gateway.clone()
    }

    /// Get the ProjectManager reference.
    /// Panics if SQLite is not enabled (project_manager is None).
    pub fn project_manager(&self) -> Arc<oxios_kernel::ProjectManager> {
        self.project_manager
            .clone()
            .expect("ProjectManager not available — SQLite must be enabled")
    }

    /// Get the MountManager reference, if SQLite-backed mounts are enabled.
    /// Returns `None` when the mount system is unavailable (SQLite off).
    pub fn mount_manager(&self) -> Option<Arc<oxios_kernel::MountManager>> {
        self.mount_manager.clone()
    }

    /// Build a MarketplaceApi (ClawHub + Skills.sh) from config.
    fn build_marketplace_api(&self) -> MarketplaceApi {
        let workspace = PathBuf::from(&self.config.kernel.workspace);
        let skills_dir = workspace.join("skills");
        let config = &self.config.marketplace;

        // ClawHub
        let clawhub_client = match ClawHubClient::new(config.base_url.clone()) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "Invalid marketplace.base_url, using default");
                ClawHubClient::new(Some("https://clawhub.ai".to_string())).unwrap()
            }
        };
        let clawhub_installer = ClawHubInstaller::new(
            skills_dir.clone(),
            workspace.clone(),
            config.base_url.clone(),
        );

        // Skills.sh
        let ss_config = &config.skills_sh;
        let skills_sh_client =
            SkillsShClient::new(ss_config.base_url.clone(), ss_config.api_key.clone())
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "Failed to create Skills.sh client, using default");
                    SkillsShClient::new(None, None).unwrap()
                });
        let skills_sh_installer = SkillsShInstaller::new(
            skills_dir,
            ss_config.base_url.clone(),
            ss_config.api_key.clone(),
        );

        MarketplaceApi::new(
            Arc::new(clawhub_installer),
            Arc::new(clawhub_client),
            Arc::new(skills_sh_installer),
            Arc::new(skills_sh_client),
        )
    }

    /// Build the calendar API facade (optional — only if `[calendar] enabled = true`).
    fn build_calendar_api(&self) -> Option<oxios_kernel::CalendarApi> {
        if !self.config.calendar.enabled {
            return None;
        }

        let workspace = PathBuf::from(&self.config.kernel.workspace);
        let calendar_dir = workspace.join("calendar").join("events");

        // CalendarEngine::new only creates the directory and loads index.json — sync-compatible.
        let engine = std::fs::create_dir_all(&calendar_dir)
            .map_err(|e| {
                tracing::warn!(error = %e, "Failed to create calendar directory");
                e
            })
            .ok()
            .and_then(|_| oxios_calendar::CalendarEngine::new_blocking(calendar_dir).ok());

        match engine {
            Some(engine) => {
                tracing::info!("Calendar system initialized");
                Some(oxios_kernel::CalendarApi::with_event_bus(
                    Arc::new(engine),
                    self.event_bus.clone(),
                ))
            }
            None => {
                tracing::warn!("Failed to initialize calendar system");
                None
            }
        }
    }

    /// Build the email API facade (optional — only if `[email] enabled = true`).
    fn build_email_api(&self) -> Option<oxios_kernel::EmailApi> {
        if !self.config.email.enabled {
            return None;
        }

        if self.config.email.my_email.is_empty() {
            tracing::warn!("Email enabled but my_email not set — skipping");
            return None;
        }

        // Resolve SMTP password: env var → credential store
        let password: Option<String> = std::env::var("OXIOS_EMAIL_PASSWORD")
            .ok()
            .filter(|p| !p.is_empty())
            .or_else(|| std::env::var("RESEND_API_KEY").ok())
            .or_else(|| {
                // Try credential store
                oxi_sdk::load_token(&self.config.email.secret_ref)
                    .ok()
                    .flatten()
                    .map(|t| t.access_token)
            });

        let password = match password {
            Some(p) => p,
            None => {
                tracing::warn!(
                    "Email enabled but no SMTP password found. Set OXIOS_EMAIL_PASSWORD env var or run 'oxios email setup'."
                );
                return None;
            }
        };

        match oxios_kernel::SmtpClient::from_config(&self.config.email, &password) {
            Ok(smtp) => {
                let workspace = PathBuf::from(&self.config.kernel.workspace);
                let template_dir = workspace.join("email_templates");
                let _ = std::fs::create_dir_all(&template_dir);

                tracing::info!(
                    from = %smtp.from_addr(),
                    "Email system initialized"
                );
                Some(oxios_kernel::EmailApi::new(
                    smtp,
                    template_dir,
                    self.state_store.clone(),
                    Some(self.event_bus.clone()),
                    self.config.email.rate_limit_per_hour,
                ))
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to initialize email system");
                None
            }
        }
    }

    /// Configuration reference.
    pub fn config(&self) -> &OxiosConfig {
        &self.config
    }

    /// Orchestrator reference — for hot-reload config propagation.
    #[allow(dead_code)]
    pub fn orchestrator(&self) -> &Arc<Orchestrator> {
        &self.orchestrator
    }

    /// Flush audit trail entries to persistent storage.
    /// Call during graceful shutdown to ensure no entries are lost.
    pub fn flush_audit(&self) -> anyhow::Result<()> {
        self.audit_trail
            .flush_to(&*self.state_store)
            .map_err(|e| anyhow::anyhow!("audit flush failed: {e}"))
    }

    /// RFC-025 Phase 5: signal the Mount auto-promotion scanner to stop
    /// (Promo-6). No-op when the scanner is disabled. Safe to call during
    /// graceful shutdown; the spawned task breaks its `select!` loop on the
    /// next iteration.
    #[allow(dead_code)] // part of graceful-shutdown wiring (Promo-6); call site TODO
    pub fn shutdown_promotion_scanner(&self) {
        if let Some(tx) = &self.promo_shutdown_tx {
            let _ = tx.send(true);
        }
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
            .handle_unified(
                "cli",
                prompt,
                session_id,
                None,
                None,
                None,
                None,
                "cli-direct",
            )
            .await
    }

    /// Execute a prompt in chat mode (skips Ouroboros pipeline).
    /// **DEPRECATED (RFC-027):** use `execute_prompt_with_session` which
    /// routes through the unified `IntentEngine` path.
    #[allow(dead_code)]
    pub async fn execute_prompt_chat(
        &self,
        prompt: &str,
        session_id: Option<&str>,
    ) -> Result<oxios_kernel::OrchestrationResult> {
        self.orchestrator
            .handle_unified(
                "cli",
                prompt,
                session_id,
                None,
                None,
                None,
                None,
                "cli-direct",
            )
            .await
    }

    /// Register a channel with the gateway.
    pub async fn register_channel(
        &self,
        channel: Box<dyn oxios_gateway::Channel>,
    ) -> anyhow::Result<()> {
        self.gateway.register(channel).await
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

        if defaults_dir.exists() {
            let count_before = self.skill_manager.list_skills().await.len();
            if let Err(e) = self.skill_manager.load_from_dir(&defaults_dir).await {
                tracing::warn!(
                    path = %defaults_dir.display(),
                    error = %e,
                    "Failed to load default skills directory"
                );
            } else {
                let count_after = self.skill_manager.list_skills().await.len();
                let installed = count_after.saturating_sub(count_before);
                if installed > 0 {
                    tracing::info!(count = installed, "Default skills installed");
                }
            }
        } else {
            tracing::debug!("No default skills directory found");
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
    ///
    /// `web_dist` is the atomic handle to the active web-dist directory. It is
    /// forwarded to the daily health check so auto-updates publish a new
    /// generation atomically (RFC-024 SP3) instead of deleting files that
    /// in-flight requests may be reading.
    pub fn start_guardian(&self, web_dist: oxios_gateway::ActiveWebDist) {
        use oxi_sdk::AuditAction;
        let handle = self.handle();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;

                if let Ok(valid) = handle.security.verify_chain()
                    && !valid
                {
                    handle.security.audit(
                        "guardian",
                        AuditAction::Other {
                            detail: "AUDIT CHAIN BROKEN".into(),
                        },
                        "guardian",
                    );
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

                if let Ok(valid) = handle.infra.git_verify()
                    && !valid
                {
                    handle.security.audit(
                        "guardian",
                        AuditAction::Other {
                            detail: "GIT REPOSITORY CORRUPTED".into(),
                        },
                        "guardian",
                    );
                }

                let _ = handle.commit_all("guardian: periodic checkpoint");
            }
        });

        // Daily health check: web UI update, self-update check.
        self.start_daily_health_check(web_dist);
    }

    /// Start the daily health check loop.
    ///
    /// Runs at 03:00 AM every day (user's local time) via cron expression.
    /// First tick is calculated to land on the next 3 AM, then every 24h after.
    fn start_daily_health_check(&self, web_dist: oxios_gateway::ActiveWebDist) {
        tokio::spawn(async move {
            let now = chrono::Local::now();
            let mut next = now
                .date_naive()
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

            if let Err(e) = daily_health_check(web_dist.clone()).await {
                tracing::warn!(error = %e, "Daily health check failed");
            }

            let mut interval = tokio::time::interval(std::time::Duration::from_secs(86400));
            loop {
                interval.tick().await;
                if let Err(e) = daily_health_check(web_dist.clone()).await {
                    tracing::warn!(error = %e, "Daily health check failed");
                }
            }
        });
    }
}

/// Daily health check logic (RFC-024 SP3: atomic publish).
///
/// Downloads the new dist into a **fresh versioned staging directory** and
/// publishes it atomically via the in-memory pointer + persisted marker.
/// The previously-active directory is removed after a grace period by a
/// background task. No request ever observes a half-extracted directory.
async fn daily_health_check(web_dist: oxios_gateway::ActiveWebDist) -> anyhow::Result<()> {
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

    // Current active directory + its version. The version lives inside the
    // dist itself (written by the Vite build as `dist/version.json`).
    // GitHub tags carry a leading `v`; the version file does not, so
    // normalize before comparing.
    let active_path = web_dist.path();
    // A dir is "usable" only when internally self-consistent; a partial or
    // raced extraction missing referenced assets must be replaced.
    let consistent = active_path
        .as_ref()
        .map(|p| oxios_gateway::ActiveWebDist::dist_is_consistent(p))
        .unwrap_or(false);
    let current_version = active_path
        .as_ref()
        .and_then(|p| std::fs::read(p.join("version.json")).ok())
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v["version"].as_str().map(str::to_string))
        .unwrap_or_default();

    let latest_version = latest_tag.trim_start_matches('v');
    // Only (re)download when the dist is missing/inconsistent, OR reports a
    // KNOWN version that differs from latest. A blank version on a
    // CONSISTENT dir (e.g. an unstamped "0.0.0" build — see
    // web/vite.config.ts) does NOT trigger a download: this is what stops a
    // perpetually re-downloading storm when version stamping regresses,
    // while still recovering a genuinely broken or missing dist.
    let needs_download =
        !consistent || (!current_version.is_empty() && current_version != latest_version);

    if !needs_download {
        tracing::debug!(
            current = %current_version,
            latest = %latest_tag,
            consistent,
            "Daily health check: web UI up to date; skipping download"
        );
        return Ok(());
    }

    tracing::info!(
        current = %current_version,
        latest = %latest_tag,
        "Updating web UI..."
    );

    // Download web-dist.zip
    let url =
        format!("https://github.com/a7garden/oxios/releases/download/{latest_tag}/web-dist.zip");
    let bytes = client.get(&url).send().await?.bytes().await?;

    // Extract into a fresh versioned staging dir (never the active dir).
    let staging = crate::web_dist::staging_dir_for(latest_tag)
        .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    crate::web_dist::extract_zip_into(&staging, &bytes)?;

    // Validate before publishing — a corrupt or partial extraction must not
    // become active. Self-consistency (not just index.html presence) catches
    // a dist that mixes two builds, so a broken page is never published.
    if !oxios_gateway::ActiveWebDist::dist_is_consistent(&staging) {
        anyhow::bail!(
            "extracted dist is not self-consistent (index.html references missing assets)"
        );
    }

    // Atomic publish: swap the pointer + persist marker. The previous
    // generation is cleaned up after a grace period so in-flight requests
    // reading from the old inode complete successfully.
    let marker = crate::web_dist::active_marker_path()
        .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    web_dist.publish(staging, &marker);

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
    #[cfg(feature = "sqlite-memory")]
    fn create_embedding_provider(config: &OxiosConfig) -> Arc<dyn oxios_kernel::EmbeddingProvider> {
        use oxios_kernel::TfIdfEmbeddingProvider;
        let emb_config = &config.memory.embedding;

        match emb_config.provider.as_str() {
            "gguf" => {
                #[cfg(feature = "embedding-gguf")]
                {
                    use oxios_kernel::{EmbeddingDimension, GgufEmbeddingProvider};

                    let model_dir =
                        oxios_kernel::embedding::gguf::GgufModelLoader::model_dir_for_workspace(
                            Path::new(&config.kernel.workspace),
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

        let mut config = if config_path.exists() {
            tracing::info!(path = %config_path.display(), "Loading config");
            load_config(&config_path)?
        } else {
            tracing::info!("No config file found, using defaults");
            OxiosConfig::default()
        };

        // RFC-018: Apply consolidation preset if not "custom".
        // This overwrites individual consolidation fields with preset values.
        config.memory.consolidation.apply_preset();

        let event_bus = EventBus::new(config.kernel.event_bus_capacity);

        // RFC-015 P1: shared streaming-sink registry. The gateway registers
        // a strong sender per active chat session, the runtime callback
        // looks it up by session_id to push live text deltas. The SAME Arc
        // is attached to KernelHandle (for runtime lookup) and to the
        // Gateway (for registration).
        let streaming_sinks = Arc::new(oxios_kernel::streaming_sink::StreamingSinkRegistry::new());
        let state_store = Arc::new(oxios_kernel::state_store::StateStore::new(PathBuf::from(
            &config.kernel.workspace,
        ))?);

        // Model comes from config, not hardcoded default
        let model_id = &config.engine.default_model;
        // Initialize the shared model catalog once. This pulls in dynamic
        // models.dev metadata (live prices/limits, user overrides). Failure
        // is non-fatal: engines fall back to the static registry.
        let catalog = match OxiosEngine::init_file_catalog().await {
            Ok(c) => {
                tracing::info!("Model catalog initialized (dynamic models.dev data)");
                Some(c)
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Failed to initialize model catalog; resolving via static registry"
                );
                None
            }
        };
        let engine = if config.engine.routing_enabled {
            let mut engine_builder = OxiosEngine::builder().default_model(model_id);
            if let Some(ref c) = catalog {
                engine_builder = engine_builder.with_catalog(c.clone());
            }
            let (engine, _routing_control) = engine_builder.build_with_routing();
            Arc::new(engine)
        } else {
            match &catalog {
                Some(c) => Arc::new(OxiosEngine::from_config_with_catalog(
                    model_id,
                    config.engine.api_key.as_deref(),
                    c.clone(),
                )),
                None => Arc::new(OxiosEngine::from_config(
                    model_id,
                    config.engine.api_key.as_deref(),
                )),
            }
        };
        // Boot-time validation: resolve the configured model so a broken
        // config fails fast (daemon refuses to start). `model.provider` is
        // reused below to seed the agent API key.
        let model = engine
            .resolve_model(model_id)
            .context(format!("Failed to resolve model: {model_id}"))?;

        // EngineHandle — hot-swappable engine reference. Created here so both
        // the OuroborosEngine (interview/crystallize/review) and the AgentRuntime
        // (execute) resolve the *live* default model through it — the single
        // source of truth that makes the phases agree and honors hot-swaps.
        let engine_handle = Arc::new(EngineHandle::new(engine));

        // Boot-time fail-fast for the provider too: this also warms the
        // EngineHandle provider cache.
        engine_handle
            .resolve_default()
            .context("Boot model/provider resolution failed")?;

        let resolver: Arc<dyn oxios_ouroboros::ModelResolver> = engine_handle.clone();
        let intent_engine: Arc<oxios_ouroboros::IntentEngine> =
            Arc::new(oxios_ouroboros::IntentEngine::new(resolver.clone()));

        let mut access_manager = AccessManager::new();
        if let Some(ref audit_path) = config.security.audit_log_path {
            let expanded = oxios_kernel::config::expand_home(audit_path);
            access_manager = access_manager.with_audit_log_path(expanded.clone());
            tracing::info!(path = %expanded.display(), "Audit log file persistence enabled");
        }
        let access_manager = Arc::new(parking_lot::Mutex::new(access_manager));

        let persona_manager = PersonaManager::new();
        // RFC-039: 디스크에서 페르소나 로드 → config 적용 → 활성 결정 → intent 시드.
        // 손상은 silent fallback 하지 않고 tracing log 에 남김.
        if let Err(e) = persona_manager.load_from_state_store(&state_store).await {
            tracing::warn!(error = %e, "persona load from state store failed; using in-memory defaults");
        }
        persona_manager.apply_config(&config.persona);
        if let Some(p) = persona_manager.first_enabled() {
            intent_engine.set_persona_prompt(Some(p.system_prompt.clone()));
            tracing::info!(persona = %p.name, "Active persona set on engines");
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

        // ProjectManager — initialized after SQLite (RFC-011)
        let mut project_manager: Option<Arc<oxios_kernel::ProjectManager>> = None;
        // MountManager — initialized after SQLite (RFC-025)
        let mut mount_manager: Option<Arc<oxios_kernel::MountManager>> = None;

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

                    let db_clone = db.clone();
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

                    // Initialize ProjectManager using the same SQLite database
                    match oxios_kernel::ProjectManager::new(
                        db_clone.clone(),
                        Some(event_bus.clone()),
                    ) {
                        Ok(pm) => {
                            project_manager = Some(Arc::new(pm));
                            tracing::info!("ProjectManager initialized");
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "ProjectManager init failed (non-fatal)");
                        }
                    }

                    // Initialize MountManager (RFC-025) using the same SQLite database.
                    // Coexists with ProjectManager during the migration window.
                    match oxios_kernel::MountManager::new(db_clone, Some(event_bus.clone())) {
                        Ok(mm) => {
                            // RFC-025: one-time migration — promote legacy
                            // Project paths into Mounts. Idempotent: Projects
                            // that already reference Mounts are skipped.
                            if let Some(ref pm) = project_manager {
                                migrate_projects_to_mounts(&mm, pm);
                            }
                            mount_manager = Some(Arc::new(mm));
                            tracing::info!("MountManager initialized");
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "MountManager init failed (non-fatal)");
                        }
                    }

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
                let dream_config = oxios_kernel::DreamConfig::from(consolidation);
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

        // ── RFC-025 Phase 5: Mount auto-promotion background scanner ──
        // Scans session history on a cadence and promotes paths that cross
        // the frequency threshold into Mounts. Cheap (one filesystem walk
        // per scan) and debounced by the threshold.
        //
        // Promo-6: a `watch` channel provides a cancellation point so a
        // future graceful shutdown can break the loop. The `Sender` is
        // stored on the returned `Kernel` to keep it alive.
        let promo_shutdown_tx = {
            let mounts_cfg = &config.mounts;
            if !mounts_cfg.auto_promote_enabled {
                None
            } else if let Some(ref mm) = mount_manager {
                let mm = mm.clone();
                let ss = state_store.clone();
                // Promo-11: respect the configured toggle instead of a
                // hardcoded `true`.
                let promo_config = oxios_kernel::PromotionConfig {
                    enabled: mounts_cfg.auto_promote_enabled,
                    threshold: mounts_cfg.auto_promote_threshold,
                    window_days: mounts_cfg.auto_promote_window_days,
                };
                let interval_secs = mounts_cfg.auto_promote_interval_secs;
                // Promo-6: cancellation channel.
                let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
                let window_days = mounts_cfg.auto_promote_window_days;
                tokio::spawn(async move {
                    let mut ticker =
                        tokio::time::interval(std::time::Duration::from_secs(interval_secs));
                    // The first tick completes immediately — run a scan right
                    // after startup, then wait the full interval thereafter.
                    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    loop {
                        // Promo-6: break out on shutdown.
                        tokio::select! {
                            _ = ticker.tick() => {}
                            res = shutdown_rx.changed() => {
                                if res.is_err() || *shutdown_rx.borrow() {
                                    tracing::info!(
                                        "Mount auto-promotion scanner shutting down"
                                    );
                                    break;
                                }
                            }
                        }

                        // Promo-1: only load sessions updated within the
                        // promotion window, bounding memory to the ones that
                        // can actually contribute a touch.
                        let cutoff = chrono::Utc::now() - chrono::Duration::days(window_days);
                        let sessions = match ss.load_sessions_for_promotion(cutoff).await {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::warn!(error = %e, "Mount promotion scan failed");
                                continue;
                            }
                        };

                        // Promo-5: `promote_frequent_paths` does blocking
                        // filesystem I/O (canonicalize + marker walks). Run it
                        // on a blocking thread so it never stalls the async
                        // runtime.
                        let mm = mm.clone();
                        let promo_config = promo_config.clone();
                        match tokio::task::spawn_blocking(move || {
                            mm.promote_frequent_paths(&sessions, &promo_config)
                        })
                        .await
                        {
                            Ok(created) if !created.is_empty() => {
                                tracing::info!(
                                    promoted = created.len(),
                                    "RFC-025: auto-promoted frequent paths to Mounts"
                                );
                            }
                            Ok(_) => {}
                            Err(e) => {
                                tracing::warn!(
                                    error = %e,
                                    "Mount promotion scan task panicked"
                                );
                            }
                        }
                    }
                });
                tracing::info!("Mount auto-promotion scanner spawned");
                Some(shutdown_tx)
            } else {
                None
            }
        };

        let budget_manager = Arc::new(BudgetManager::new());
        // RFC-031 v2: the shared QuotaTracker. One live instance per
        // kernel. v2 derives eligibility from live quota snapshots;
        // the v1 `[[token-maxing.providers]]` opt-in block is now
        // optional and no longer required. We keep a forward-looking
        // warning so users with existing v1 configs know they can
        // remove the block safely.
        if !config.token_maxing.providers.is_empty() {
            tracing::warn!(
                count = config.token_maxing.providers.len(),
                "[[token-maxing.providers]] is no longer required by RFC-031 v2; \
                 token-maxing now derives eligibility from live quota API \
                 responses. The block was preserved for back-compat but can be \
                 removed."
            );
        }
        let quota_tracker = Arc::new(oxios_kernel::QuotaTracker::new(config.token_maxing.clone()));

        // RFC-031 Phase 2: recalibration tick. Where a provider exposes a
        // usage/balance endpoint, periodically snap the self-tracked counter
        // to real state, erasing drift from a key shared with another app.
        {
            let interval = config.token_maxing.recalibration_interval_secs;
            let api_key = config.engine.api_key.clone();
            if interval > 0 {
                tokio::spawn(recalibration_tick(
                    Arc::clone(&quota_tracker),
                    interval,
                    api_key,
                ));
            }
        }

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

        oxios_kernel::event_bus::attach_audit_trail(&event_bus, audit_trail.clone());

        // Restore persisted audit entries.
        if let Ok(entries) = state_store.load()
            && !entries.is_empty()
        {
            tracing::info!(count = entries.len(), "Restored audit trail entries");
            audit_trail.restore_from(entries);
        }

        // Routing stats — shared between EngineApi and AgentRuntime
        let routing_stats = Arc::new(oxios_kernel::RoutingStats::new());

        // EngineHandle was created earlier (before OuroborosEngine) so both the
        // Ouroboros phases and AgentRuntime resolve the live default model
        // through the same handle. EngineApi writes (set_model / set_api_key)
        // rebuild and swap it; AgentRuntime reads the latest on each execute().
        // ── Gateway APIs — Arc-wrapped for sharing with Gateway and KernelHandle ──
        let engine_api = Arc::new(oxios_kernel::EngineApi::new(
            Arc::new(parking_lot::RwLock::new(config.clone())),
            config_path.clone(),
            Arc::clone(&routing_stats),
            Arc::clone(&engine_handle),
        ));
        let mut persona_api_unwrapped =
            oxios_kernel::PersonaApi::new(Arc::new(persona_manager.clone()));
        // RFC-039: auto re-seed the intent engine when the active persona
        // changes via HTTP. The binary crate bridges kernel ↔ ouroboros.
        let ie = intent_engine.clone();
        persona_api_unwrapped.set_reseed_callback(Some(Arc::new(move |prompt| {
            ie.set_persona_prompt(prompt);
        })));
        let persona_api = Arc::new(persona_api_unwrapped);

        // Shared KnowledgeBase — single source of truth (RFC-003)
        let knowledge_base = Arc::new(
            KnowledgeBase::new(PathBuf::from(&config.kernel.workspace).join("knowledge"))
                .expect("KnowledgeBase init failed"),
        );

        // HNSW index for fast semantic search
        let hnsw_index = Arc::new(
            HnswMemoryIndex::new(
                config.memory.sqlite.embedding_dim,
                10000,
                Some(PathBuf::from(&config.kernel.workspace).join("memory")),
            )
            .expect("HNSW index init failed"),
        );

        // Build AgentApi with HNSW index attached
        let mut agent_api = oxios_kernel::AgentApi::new(
            // Placeholder supervisor — the real one needs AgentRuntime which needs this handle.
            // AgentApi.supervisor is only used for list/kill, not during tool registration.
            Arc::new(oxios_kernel::supervisor::NoOpSupervisor),
            budget_manager.clone(),
            memory_manager.clone(),
            None,
        );
        agent_api.set_hnsw_index(hnsw_index.clone());

        // ── KernelHandle — the syscall table for agent OS control ──
        // Created inline here because AgentRuntime needs it.
        // Will be cached again in the Kernel instance.
        let kernel_handle: Arc<oxios_kernel::KernelHandle> = {
            let kh = oxios_kernel::KernelHandle::new(
                oxios_kernel::StateApi::new(state_store.clone()),
                agent_api,
                oxios_kernel::SecurityApi::new(
                    auth_manager.clone(),
                    audit_trail.clone(),
                    access_manager.clone(),
                    state_store.clone(),
                ),
                oxios_kernel::PersonaApi::new(Arc::new(persona_manager.clone())),
                oxios_kernel::ExtensionApi::new(Arc::clone(&skill_manager)),
                oxios_kernel::McpApi::new(mcp_bridge.clone()),
                oxios_kernel::InfraApi::new(
                    git_layer.clone(),
                    cron_scheduler.clone(),
                    resource_monitor.clone(),
                    event_bus.clone(),
                    config.clone(),
                    std::time::Instant::now(),
                ),
                project_manager.clone().map(oxios_kernel::ProjectApi::new),
                oxios_kernel::ExecApi::new(
                    Arc::new(parking_lot::RwLock::new(config.exec.clone())),
                    access_manager.clone(),
                ),
                oxios_kernel::A2aApi::new(a2a_protocol.clone()),
                // EngineApi — routing stats shared between EngineApi and AgentRuntime + engine hot-swap
                oxios_kernel::EngineApi::new(
                    Arc::new(parking_lot::RwLock::new(config.clone())),
                    config_path.clone(),
                    Arc::clone(&routing_stats),
                    Arc::clone(&engine_handle),
                ),
                // KnowledgeBase — single source of truth (RFC-003), shared
                knowledge_base.clone(),
                // KnowledgeLens — semantic overlay, shares same KnowledgeBase
                Arc::new(
                    oxios_kernel::KnowledgeLens::new(
                        knowledge_base.clone(),
                        memory_manager.clone(),
                    )
                    .expect("KnowledgeLens init failed"),
                ),
                build_marketplace_api_value(&config),
                None, // calendar (initialized later)
                None, // email (initialized later)
                oxios_kernel::PtyApi::new(Arc::new(parking_lot::RwLock::new(config.pty.clone()))),
            );

            // RFC-015 P1: attach the streaming-sink registry so the runtime
            // callback's per-session `TextChunk` lookup finds the gateway's
            // collector sender. Wired before `Arc::new(kh)` so we can use
            // the consuming builder.
            let kh = kh.with_streaming_sinks(streaming_sinks.clone());
            // Attach the Mount facade (RFC-025). Set before Arc so the handle
            // carries it from construction.
            let kh = if let Some(mm) = mount_manager.clone() {
                kh.with_mounts(oxios_kernel::MountApi::new(mm))
            } else {
                kh
            };
            Arc::new(kh)
        };

        // Knowledge dream (RFC-022)
        if config.memory.knowledge_dream.enabled {
            let kb = kernel_handle.knowledge.clone();
            let kd_config = config.memory.knowledge_dream.clone();
            match oxios_kernel::knowledge_dream::KnowledgeDream::new(
                kb,
                git_layer.clone(),
                engine_handle.clone(),
                kd_config,
            ) {
                Ok(kd) => {
                    Arc::new(kd).spawn();
                    tracing::info!("Knowledge dream spawned for background note curation");
                }
                Err(e) => {
                    // Non-fatal: the dream is a background feature. A bad
                    // curation model disables it with a clear log rather than
                    // crashing the daemon or silently failing every cycle.
                    tracing::error!(
                        error = %e,
                        "Knowledge dream disabled — invalid model config, skipping background curation"
                    );
                }
            }
        }

        // Build ToolRetriever for semantic capability discovery.
        let tool_retriever = build_tool_retriever(&skill_manager).await;

        let agent_runtime = AgentRuntime::new(
            Arc::clone(&engine_handle),
            kernel_handle.clone(),
            Some(Arc::clone(&routing_stats)),
        )
        .with_persona_manager(Arc::new(persona_manager.clone()))
        .with_tool_retriever(Arc::new(tool_retriever))
        .with_config({
            // Resolve API key from CredentialStore based on the model's provider.
            let provider_name = model.provider.as_str();
            let config_api_key = config.engine.api_key.as_deref();
            let api_key = oxios_kernel::CredentialStore::resolve(provider_name, config_api_key)
                .map(|(key, _)| key);

            oxios_kernel::agent_runtime::AgentRuntimeConfig {
                api_key,
                provider_options: config.engine.provider_options.clone(),
                ..Default::default()
            }
        })
        .with_persistence_hook(Arc::new(oxios_kernel::PersistenceHook::new(
            memory_manager.clone(),
            knowledge_base.clone(),
            Arc::clone(&engine_handle),
            state_store.clone(),
            event_bus.clone(),
        )));

        let mut basic_supervisor = BasicSupervisor::new(event_bus.clone(), agent_runtime);

        // Wire agent history persistence
        basic_supervisor.set_state_store(state_store.clone());
        basic_supervisor.set_agent_log_config(config.agent_log.clone());

        // Wire SQLite agent log index if available
        #[cfg(feature = "sqlite-memory")]
        let (agent_log_db,) = {
            let db_path = if config.agent_log.db_path.is_empty() {
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(".oxios/state/agent_log.db")
            } else {
                let p = std::path::PathBuf::from(&config.agent_log.db_path);
                if p.is_absolute() {
                    p
                } else {
                    dirs::home_dir().unwrap_or_default().join(".oxios").join(&p)
                }
            };

            // Ensure parent dir exists
            if let Some(parent) = db_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            match oxios_kernel::agent_log_db::AgentLogDb::open(&db_path) {
                Ok(db) => {
                    let db = Arc::new(db);
                    basic_supervisor.set_agent_log_db(db.clone());
                    tracing::info!(path = %db_path.display(), "Agent history SQLite log initialized");
                    (Some(db),)
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        path = %db_path.display(),
                        "Failed to open agent history SQLite DB, falling back to filesystem-only"
                    );
                    (None,)
                }
            }
        };
        #[cfg(not(feature = "sqlite-memory"))]
        let agent_log_db: Option<Arc<oxios_kernel::agent_log_db::AgentLogDb>> = None;

        let supervisor: Arc<dyn Supervisor> = Arc::new(basic_supervisor);

        let lifecycle = oxios_kernel::AgentLifecycleManager::new(
            supervisor.clone(),
            access_manager.clone(),
            a2a_protocol.clone(),
            event_bus.clone(),
            config.security.max_execution_time_secs,
            config.security.allowed_tools.clone(),
            config.security.network_access,
            config.kernel.workspace.clone(),
        );

        // Register the A2A dispatch handler.
        // When a TaskDelegation arrives, the handler spawns an agent via
        // the lifecycle manager and returns the execution result.
        let dispatch_lifecycle = lifecycle.clone();
        a2a_protocol
            .set_delegation_handler(Arc::new(move |_from, _to, task| {
                let lc = dispatch_lifecycle.clone();
                Box::pin(async move {
                    let directive = oxios_ouroboros::Directive {
                        goal: task.description.clone(),
                        ..Default::default()
                    };
                    let env = oxios_ouroboros::ExecEnv::default();
                    match lc.execute_directive(&directive, &env).await {
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

        // RFC-031 Phase 3: the TokenMaxer orchestrator. Clone the lifecycle
        // manager (it impls Clone) before the orchestrator consumes it; the
        // maxer drains eligible subscription providers over a window.
        let maxer_lifecycle = lifecycle.clone();
        let planner = oxios_kernel::WorkPlanner::new(
            Arc::clone(&skill_manager),
            project_manager.clone(),
            mount_manager.clone(),
        );
        let token_maxer = Arc::new(oxios_kernel::TokenMaxer::new(
            maxer_lifecycle,
            Arc::clone(&quota_tracker),
            planner,
            state_store.clone(),
        ));

        let mut orchestrator = Orchestrator::with_config(
            event_bus.clone(),
            state_store.clone(),
            lifecycle,
            config.orchestrator.clone(),
        );
        orchestrator.set_intent_engine(intent_engine.clone());
        orchestrator.set_git_layer(git_layer.clone());
        orchestrator.set_a2a(a2a_protocol.clone());
        if let Some(pm) = project_manager.clone() {
            orchestrator.set_project_manager(pm);
        }
        if let Some(mm) = mount_manager.clone() {
            orchestrator.set_mount_manager(mm);
        }
        // RFC-029: wire the recovery coordinator (L1 backoff / L2 model
        // swap). Shares RoutingStats with EngineApi/AgentRuntime so
        // fallback events surface in the Web UI. Reads the configured
        // fallback-model list (live-updatable via set_routing).
        {
            let coordinator = Arc::new(oxios_kernel::resilience::RecoveryCoordinator::new(
                Arc::clone(&routing_stats),
                oxios_kernel::resilience::ResilienceConfig::default(),
            ));
            coordinator.set_fallback_models(config.engine.fallback_models.clone());
            orchestrator.set_recovery(coordinator);
        }

        let orchestrator = Arc::new(orchestrator);

        // RFC-015 P1: attach the streaming-sink registry shared with the
        // KernelHandle so the runtime callback can find the gateway's
        // collector sender for live text deltas.
        let gateway = Gateway::with_apis(orchestrator.clone(), engine_api, persona_api)
            .with_streaming_sinks(streaming_sinks);
        let gateway = Arc::new(gateway);

        // Initialize metrics and observability singletons.
        oxios_kernel::register_builtin_metrics();
        oxios_kernel::observability::init();

        let kernel = Kernel {
            orchestrator,
            gateway,
            event_bus: event_bus.clone(),
            state_store: state_store.clone(),
            config,
            skill_manager,
            supervisor,
            access_manager,
            persona_manager,
            mcp_bridge,
            memory_manager,
            auth_manager,
            cron_scheduler,
            git_layer,
            audit_trail,
            budget_manager,
            quota_tracker,
            token_maxer,
            resource_monitor,
            project_manager,
            mount_manager,
            start_time: std::time::Instant::now(),
            config_path,
            // Do NOT pre-seed with the cycle-breaking preliminary handle
            // (`kernel_handle`, built above solely to construct AgentRuntime):
            // its AgentApi is intentionally incomplete (NoOpSupervisor, no
            // agent_log_db / state_store). Caching it made every control-plane
            // agent query silently return empty even though the real
            // supervisor kept persisting rows. The fully-wired handle is
            // assembled lazily by `handle()` and cached just below.
            handle_cache: std::sync::OnceLock::new(),
            a2a_protocol,
            engine_handle,
            #[cfg(feature = "sqlite-memory")]
            agent_log_db,
            promo_shutdown_tx,
        };

        // Eagerly assemble the fully-wired KernelHandle (real supervisor +
        // SQLite agent log + state store) and cache it, so the control plane
        // — HTTP API, CLI — never observes the incomplete preliminary handle.
        // Runs `handle()`'s `get_or_init` exactly once.
        let handle = kernel.handle();

        // RFC-024 SP4: mark state store ready and start the 30 s readiness
        // deadline on the *cached* handle's gate. The engine state is
        // finalized by the caller (main.rs cmd_serve) once it knows whether
        // the configured model has an API key — at which point the gate is
        // set to `Ready` or `Degraded`. The deadline forcibly promotes any
        // still-Warming subsystem to `Degraded` so a missing API key cannot
        // lock the gate forever.
        handle.readiness.set_state_store(SubsystemState::Ready);
        let deadline_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() + 30)
            .unwrap_or(0);
        handle.readiness.set_deadline_secs(deadline_secs);

        Ok(kernel)
    }
}

/// RFC-031 v2: background recalibration loop. Periodically fetches
/// real provider quota and caches the live snapshot in
/// [`QuotaTracker`]. v2 departure from v1: probes **every provider
/// for which a [`QuotaFetcher`] is registered** (via
/// [`crate::api::quota::all_fetchers`]) AND has credentials
/// configured. This is what makes the user's bug fix work: zai is
/// registered in the engine (so the credential store has a key)
/// but missing from `[[token-maxing.providers]]`. The v1 tick
/// filtered on config eligibility and never fetched zai.
async fn recalibration_tick(
    tracker: Arc<oxios_kernel::QuotaTracker>,
    interval_secs: u64,
    api_key: Option<String>,
) {
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
    // The first tick fires immediately on construction — skip it so we don't
    // fan out HTTP on boot, then settle into the configured cadence.
    ticker.tick().await;
    loop {
        ticker.tick().await;
        let cfg = tracker.config();
        if !cfg.enabled {
            continue;
        }
        drop(cfg);

        // v2: probe every registered fetcher. `all_fetchers()` is the
        // canonical list of providers with a known quota endpoint
        // (zai, openai, minimax today).
        let fetchers = crate::api::quota::all_fetchers();
        let mut creds: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for f in &fetchers {
            let provider = f.provider();
            if let Some((key, _)) =
                oxios_kernel::CredentialStore::resolve(provider, api_key.as_deref())
            {
                creds.insert(provider.to_string(), key);
            }
        }
        if creds.is_empty() {
            continue;
        }
        let snaps = crate::api::quota::fetch_all(&creds).await;
        for snap in &snaps {
            // v2: also cache the live snapshot so QuotaTracker::availability
            // can use it for auto-discovery.
            let live = crate::api::quota::to_live_snapshot(snap);
            tracker.update_live_snapshot(live);
            if snap.error.is_some() {
                tracker.apply_recalibration(
                    &snap.provider,
                    None,
                    None,
                    None,
                    oxios_kernel::RecalibrationOutcome::FetchFailed,
                );
                continue;
            }
            let rw = snap
                .rate_windows
                .iter()
                .find(|w| w.remaining_percent.is_some());
            let (rem, resets) = match rw {
                Some(w) => (w.remaining_percent, w.resets_at),
                None => (None, None),
            };
            tracker.apply_recalibration(
                &snap.provider,
                rem,
                resets,
                snap.token_limit
                    .and_then(|l| if l > 0.0 { Some(l as u64) } else { None }),
                oxios_kernel::RecalibrationOutcome::Ok,
            );
        }
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
            if let Ok(args_str) = std::env::var(format!("OXIOS_MCP_{name}_ARGS")) {
                server.args = args_str.split_whitespace().map(String::from).collect();
            }
            if let Ok(env_str) = std::env::var(format!("OXIOS_MCP_{name}_ENV")) {
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
async fn build_tool_retriever(sm: &SkillManager) -> oxios_kernel::tools::retrieval::ToolRetriever {
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
            "knowledge",
            "os-service",
            "Personal markdown vault — save, read, search documents and notes",
        ),
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

/// Build a MarketplaceApi from the Kernel instance (used after Kernel construction).
fn build_marketplace_api_value(config: &OxiosConfig) -> MarketplaceApi {
    let workspace = PathBuf::from(&config.kernel.workspace);
    let skills_dir = workspace.join("skills");

    // ClawHub
    let clawhub_client =
        ClawHubClient::new(config.marketplace.base_url.clone()).unwrap_or_else(|_| {
            tracing::warn!("Invalid marketplace.base_url, using default");
            ClawHubClient::new(Some("https://clawhub.ai".to_string())).unwrap()
        });
    let clawhub_installer = ClawHubInstaller::new(
        skills_dir.clone(),
        workspace.clone(),
        config.marketplace.base_url.clone(),
    );

    // Skills.sh
    let ss = &config.marketplace.skills_sh;
    let skills_sh_client = SkillsShClient::new(ss.base_url.clone(), ss.api_key.clone()).unwrap();
    let skills_sh_installer =
        SkillsShInstaller::new(skills_dir, ss.base_url.clone(), ss.api_key.clone());

    MarketplaceApi::new(
        Arc::new(clawhub_installer),
        Arc::new(clawhub_client),
        Arc::new(skills_sh_installer),
        Arc::new(skills_sh_client),
    )
}

/// RFC-025 one-time migration: promote legacy Project paths into Mounts.
///
/// For each Project that has `paths` but no `mount_ids`, resolve a Mount for
/// every legacy path — reusing an existing Mount that already covers the path
/// (path-prefix match) when one exists, otherwise creating one named after the
/// Project — link them via `mount_ids`, then clear the legacy `paths` field.
///
/// Idempotent: Projects already referencing Mounts are skipped, and the
/// path-coverage check prevents duplicate Mounts for paths a user registered
/// under a different name.
fn migrate_projects_to_mounts(
    mount_manager: &oxios_kernel::MountManager,
    project_manager: &ProjectManager,
) {
    let projects = project_manager.list_projects();
    let mut migrated = 0usize;

    for project in projects {
        // Skip Projects that already reference Mounts (idempotent).
        if !project.mount_ids.is_empty() {
            continue;
        }
        // Skip Projects without paths — nothing to lift into a Mount.
        if project.paths.is_empty() {
            continue;
        }

        // Partition legacy paths: reuse any existing Mount that already covers
        // a path (path-prefix match), collecting only the uncovered ones for a
        // new Mount. This avoids duplicating a Mount the user registered for
        // the same path under a different name.
        let mut mount_ids: Vec<oxios_kernel::MountId> = Vec::new();
        let mut uncovered: Vec<PathBuf> = Vec::new();
        for path in &project.paths {
            match mount_manager.covering_mount_id(path) {
                Some(mid) => {
                    if !mount_ids.contains(&mid) {
                        mount_ids.push(mid);
                    }
                }
                None => uncovered.push(path.clone()),
            }
        }

        // Create one Mount for any uncovered paths, named after the Project
        // (suffixed to avoid colliding with a manually-created Mount).
        if !uncovered.is_empty() {
            let name = unique_mount_name(mount_manager, &project.name);
            match mount_manager.create_mount(
                name,
                uncovered,
                oxios_kernel::MountSource::AutoDetected,
            ) {
                Ok(mount) => mount_ids.push(mount.id),
                Err(e) => {
                    tracing::warn!(
                        project = %project.name,
                        error = %e,
                        "failed to create Mount during migration; leaving Project paths in place"
                    );
                    continue;
                }
            }
        }

        // Link the resolved Mounts and clear the legacy `paths` field so the
        // runtime legacy fallbacks never re-activate for this Project.
        if let Err(e) = project_manager.update_project_bundle(project.id, Some(mount_ids), None) {
            tracing::warn!(
                project = %project.name,
                error = %e,
                "link failed; orphan Mounts may remain"
            );
            continue;
        }
        if let Err(e) = project_manager.clear_legacy_paths(project.id) {
            tracing::warn!(
                project = %project.name,
                error = %e,
                "Mounts linked but failed to clear legacy paths"
            );
        }
        migrated += 1;
    }

    if migrated > 0 {
        tracing::info!(
            migrated = migrated,
            "RFC-025: migrated legacy Project paths into Mounts"
        );
    }
}

/// Pick a Mount name based on `base` that is not already taken, suffixing
/// `-2`, `-3`, … as needed so the migration never fails on a name collision.
fn unique_mount_name(mount_manager: &oxios_kernel::MountManager, base: &str) -> String {
    if mount_manager.get_mount_by_name(base).is_none() {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if mount_manager.get_mount_by_name(&candidate).is_none() {
            return candidate;
        }
        n += 1;
    }
}
