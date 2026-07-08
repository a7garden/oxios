//! Kernel facade — domain Facades composing the System Call API.

pub mod a2a_api;
pub mod agent_api;
pub mod calendar_api;
pub mod email_api;
pub mod engine_api;
pub mod exec_api;
pub mod extension_api;
pub mod pty_api;
pub mod infra_api;
pub mod knowledge_lens;
pub mod marketplace_api;
pub mod mcp_api;
pub mod memory_api;
pub mod mount_api;
pub mod persona_api;
pub mod project_api;
pub mod security_api;
pub mod state_api;
pub mod token_maxing_api;

pub use a2a_api::A2aApi;
pub use agent_api::AgentApi;
pub use calendar_api::CalendarApi;
pub use email_api::EmailApi;
pub use engine_api::{
    EngineApi, EngineConfigResponse, FallbackEvent, InputModality, ModelInfo, ProviderCategory,
    ProviderInfo, RoutingConfigSnapshot, RoutingStats, RoutingStatsSnapshot, RoutingUpdate,
    ValidateKeyResult,
};
pub use exec_api::ExecApi;
pub use exec_api::SharedExecConfig;
pub use pty_api::{PtyApi, SharedPtyConfig};
pub use extension_api::ExtensionApi;
pub use infra_api::InfraApi;
pub use knowledge_lens::{
    CopilotResponse, KnowledgeContext, KnowledgeLens, KnowledgeNote, MemoryNote,
};
pub use marketplace_api::MarketplaceApi;
pub use mcp_api::McpApi;
pub use memory_api::MemoryApi;
pub use mount_api::{MountApi, MountInfo};
pub use persona_api::PersonaApi;
pub use project_api::{ProjectApi, ProjectInfo};
pub use security_api::SecurityApi;
pub use state_api::StateApi;
pub use token_maxing_api::TokenMaxingApi;

use crate::git_layer::CommitInfo;
use crate::readiness::ReadinessGate;
use serde::Serialize;
use std::sync::Arc;

/// Oxios kernel System Call API — composed of domain Facades.
///
/// Each Facade groups related system calls:
/// - [`StateApi`]     — data persistence, sessions
/// - [`AgentApi`]     — agent lifecycle, budgets, memory
/// - [`SecurityApi`]  — auth, audit trail, RBAC, approvals
/// - [`PersonaApi`]   — multi-persona management
/// - [`ExtensionApi`] — programs, skills, host tools
/// - [`McpApi`]       — MCP server bridge
/// - [`MountApi`]      — Mount (path alias) management (RFC-025)
/// - [`ProjectApi`]    — Project management, memory linking
/// - [`ExecApi`]      — execution config, access management
/// - [`A2aApi`]       — agent-to-agent communication
/// - [`EngineApi`]    — LLM engine providers, models, config
/// - [`KnowledgeBase`] — markdown note management (kernel-free, via oxios-markdown)
pub struct KernelHandle {
    /// State management: save/load/sessions.
    pub state: StateApi,
    /// Agent management: lifecycle/budgets/memory.
    pub agents: AgentApi,
    /// Security: auth/audit/RBAC/approvals.
    pub security: SecurityApi,
    /// Persona management.
    pub persona: PersonaApi,
    /// Extensions: programs/skills/host tools.
    pub extensions: ExtensionApi,
    /// MCP server bridge.
    pub mcp: McpApi,
    /// Infrastructure: Git/scheduler/cron/resources/events/system.
    pub infra: InfraApi,
    /// Project management: work context (RFC-011).
    pub projects: Option<ProjectApi>,
    /// Mount management: path aliases (RFC-025).
    pub mounts: Option<MountApi>,
    /// Execution: config + access management.
    pub exec: ExecApi,
    /// RFC-038: Interactive terminal (PTY-bridged WebSocket).
    pub pty: PtyApi,
    /// Agent-to-agent communication.
    pub a2a: A2aApi,
    /// Engine: LLM providers, models, config.
    pub engine: EngineApi,
    /// Knowledge base: markdown notes (direct access, no kernel dependency).
    pub knowledge: Arc<oxios_markdown::KnowledgeBase>,
    /// Semantic knowledge overlay (HNSW index + agent recall).
    pub knowledge_lens: Arc<KnowledgeLens>,
    /// Marketplace API — ClawHub search, install, update.
    pub marketplace_api: MarketplaceApi,
    /// Calendar events — create, update, delete, list, search, freebusy.
    pub calendar: Option<CalendarApi>,
    /// Email — send HTML emails via SMTP, template management.
    pub email: Option<EmailApi>,
    /// Token-maxing (RFC-031): the shared QuotaTracker facade. `None` only on
    /// the incomplete preliminary handle; the cached handle attaches it.
    pub token_maxing: Option<TokenMaxingApi>,
    /// RFC-024 SP4: subsystem readiness gate.
    pub readiness: Arc<ReadinessGate>,
    /// Per-session streaming sink registry (P1 chat transparency).
    ///
    /// The agent runtime callback looks up the sink by `session_id` (which
    /// it already has via `transparency_session`) and pushes live text
    /// deltas. The gateway registers a strong sender before invoking
    /// the orchestrator and drops it after the collector completes; the
    /// `Weak` entries auto-clean.
    pub streaming_sinks: Arc<crate::streaming_sink::StreamingSinkRegistry>,
}

impl KernelHandle {
    /// Create a new KernelHandle from 14 domain Facades.
    ///
    /// Each Facade is assembled independently in `kernel.rs` and passed here.
    /// This enables testing individual Facades without the full kernel.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: StateApi,
        agents: AgentApi,
        security: SecurityApi,
        persona: PersonaApi,
        extensions: ExtensionApi,
        projects: Option<ProjectApi>,
        exec: ExecApi,
        a2a: A2aApi,
        engine: EngineApi,
        knowledge: Arc<oxios_markdown::KnowledgeBase>,
        knowledge_lens: Arc<KnowledgeLens>,
        marketplace_api: MarketplaceApi,
        calendar: Option<CalendarApi>,
        email: Option<EmailApi>,
        pty: PtyApi,
    ) -> Self {
        Self {
            state,
            agents,
            security,
            persona,
            extensions,
            mcp,
            infra,
            projects,
            mounts: None,
            exec,
            pty,
            a2a,
            engine,
            knowledge,
            knowledge_lens,
            marketplace_api,
            calendar,
            email,
            token_maxing: None,
            // RFC-024 SP4: default Warming/no-deadline. The Kernel
            // (src/kernel.rs) sets the actual state and deadline during
            // startup via `readiness.set_*` / a background task.
            readiness: Arc::new(ReadinessGate::new(0)),
            streaming_sinks: Arc::new(crate::streaming_sink::StreamingSinkRegistry::new()),
        }
    }

    /// Attach a MountManager-backed API (RFC-025).
    ///
    /// Called by the kernel assembler after SQLite initializes the
    /// `MountManager`. Leaves the [`Self::projects`] facade untouched so
    /// RFC-011 Projects continue to work during the migration.
    pub fn with_mounts(mut self, mounts: MountApi) -> Self {
        self.mounts = Some(mounts);
        self
    }

    /// Set the Mounts facade in place (post-construction wiring).
    pub fn set_mounts(&mut self, mounts: MountApi) {
        self.mounts = Some(mounts);
    }

    /// Attach the TokenMaxing facade (RFC-031). Called by the kernel
    /// assembler after constructing the shared `QuotaTracker`.
    pub fn with_token_maxing(mut self, api: TokenMaxingApi) -> Self {
        self.token_maxing = Some(api);
        self
    }

    /// Set the TokenMaxing facade in place (post-construction wiring).
    pub fn set_token_maxing(&mut self, api: TokenMaxingApi) {
        self.token_maxing = Some(api);
    }

    /// Attach the shared streaming-sink registry. Called by the kernel
    /// assembler to make the runtime callback's per-session `TextChunk`
    /// lookup find the gateway's collector sender. The same `Arc` must be
    /// passed to the gateway via `Gateway::with_streaming_sinks`.
    pub fn with_streaming_sinks(
        mut self,
        registry: Arc<crate::streaming_sink::StreamingSinkRegistry>,
    ) -> Self {
        self.streaming_sinks = registry;
        self
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Convenience methods (cross-Facades orchestration)
    // ═══════════════════════════════════════════════════════════════════════

    /// Save data and commit to git (State + Infra).
    ///
    /// The state save is the source of truth and is fully propagated. The git
    /// commit is best-effort observability: if it fails (full disk, lock
    /// contention, missing committer identity) we log a warning rather than
    /// failing the save — the data is already persisted on disk and failing
    /// here would mislead callers into thinking the save itself failed.
    pub async fn save_and_commit<T: Serialize>(
        &self,
        category: &str,
        name: &str,
        data: &T,
    ) -> anyhow::Result<()> {
        self.state.save(category, name, data).await?;
        let git = self.infra.git();
        if git.is_enabled() {
            let rel_path = format!("{category}/{name}.json");
            if let Err(e) = git.commit_file(&rel_path, &format!("save {category}/{name}")) {
                tracing::warn!(
                    error = %e, rel_path = %rel_path,
                    "save_and_commit: git commit failed (data was still saved)"
                );
            }
        }
        Ok(())
    }

    /// Save markdown and commit to git (State + Infra).
    ///
    /// See [`Self::save_and_commit`] for the git-failure policy.
    pub async fn save_markdown_and_commit(
        &self,
        category: &str,
        name: &str,
        content: &str,
    ) -> anyhow::Result<()> {
        self.state.save_markdown(category, name, content).await?;
        let git = self.infra.git();
        if git.is_enabled() {
            let rel_path = format!("{category}/{name}.md");
            if let Err(e) = git.commit_file(&rel_path, &format!("save {category}/{name}")) {
                tracing::warn!(
                    error = %e, rel_path = %rel_path,
                    "save_markdown_and_commit: git commit failed (data was still saved)"
                );
            }
        }
        Ok(())
    }

    /// Delete a file and commit the removal to git (State + Infra).
    ///
    /// See [`Self::save_and_commit`] for the git-failure policy.
    pub async fn delete_and_commit(&self, category: &str, name: &str) -> anyhow::Result<bool> {
        let deleted = self.state.delete(category, name).await?;
        if deleted {
            let git = self.infra.git();
            if git.is_enabled() {
                let rel_path = format!("{category}/{name}.json");
                if let Err(e) = git.remove_file(&rel_path, &format!("delete {category}/{name}")) {
                    tracing::warn!(
                        error = %e, rel_path = %rel_path,
                        "delete_and_commit: git remove failed (file was still deleted)"
                    );
                }
            }
        }
        Ok(deleted)
    }

    /// Commit all current changes to git.
    pub fn commit_all(&self, message: &str) -> anyhow::Result<Option<CommitInfo>> {
        self.state.commit_all(self.infra.git(), message)
    }

    /// Flush audit trail and commit to git (Security + Infra).
    pub fn flush_audit(&self) -> anyhow::Result<()> {
        self.security.flush(self.infra.git())
    }

    /// Schedule a cron job by expression (convenience wrapper).
    ///
    /// **Note:** the `persona` argument is currently NOT wired into the cron
    /// executor — `CronJob` has no persona field yet. Passing a non-default
    /// value logs a warning so callers are not silently surprised. The
    /// parameter is retained for forward compatibility with multi-persona
    /// scheduling (RFC tracking).
    pub async fn schedule(
        &self,
        cron_expr: &str,
        task: &str,
        persona: Option<&str>,
    ) -> anyhow::Result<String> {
        if let Some(p) = persona
            && !p.is_empty()
            && p != "default"
        {
            tracing::warn!(
                persona = p,
                "schedule: persona argument is not yet honored by the cron executor; job will run with the default persona"
            );
        }
        let job = crate::cron::CronJob::new(
            format!("job_{}", uuid::Uuid::new_v4()),
            cron_expr.to_string(),
            task.to_string(),
        );
        let job_id = self.infra.add_cron(job).await?;
        Ok(job_id.to_string())
    }

    /// Unschedule a cron job by string ID (convenience wrapper).
    ///
    /// Returns `Ok(true)` when the job existed and was removed, `Ok(false)`
    /// when no job with that ID was registered, and `Err(...)` when the
    /// scheduler itself fails (DB corruption, lock poisoning). The previous
    /// implementation collapsed scheduler errors into `Ok(false)`, hiding
    /// real failures from callers.
    pub async fn unschedule(&self, job_id: &str) -> anyhow::Result<bool> {
        let uuid =
            uuid::Uuid::parse_str(job_id).map_err(|e| anyhow::anyhow!("invalid job id: {e}"))?;
        match self.infra.remove_cron(uuid).await {
            Ok(()) => Ok(true),
            Err(e) => {
                let msg = format!("{e}");
                if msg.to_lowercase().contains("not found") {
                    // Legitimate "already removed" case — not an error.
                    Ok(false)
                } else {
                    Err(anyhow::anyhow!("failed to remove cron job {job_id}: {e}"))
                }
            }
        }
    }
    pub fn list_schedules(&self) -> Vec<crate::cron::CronJob> {
        self.infra.list_crons()
    }

    /// Load JSON from state store.
    pub async fn load_json<T: serde::de::DeserializeOwned>(
        &self,
        category: &str,
        name: &str,
    ) -> anyhow::Result<Option<T>> {
        self.state.load(category, name).await
    }

    /// Get kernel start time.
    pub fn start_time(&self) -> std::time::Instant {
        self.infra.start_time
    }

    /// Marketplace API — ClawHub search, install, update.
    pub fn marketplace_api(&self) -> &MarketplaceApi {
        &self.marketplace_api
    }

    /// Get a [`MemoryApi`] facade for memory operations.
    ///
    /// Returns a fresh `MemoryApi` each call. It shares the same underlying
    /// `Arc<MemoryManager>` and `Arc<HnswMemoryIndex>` (when attached) as
    /// `AgentApi`, so semantic search and index rebuilds route through the
    /// real index rather than the keyword-only fallback.
    pub fn memory(&self) -> MemoryApi {
        let mm = self.agents.memory_manager().clone();
        let hnsw = self.agents.hnsw_index.clone();
        MemoryApi::new(mm, hnsw)
    }
}
