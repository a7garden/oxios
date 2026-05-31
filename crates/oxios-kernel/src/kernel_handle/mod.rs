//! Kernel facade — 13 domain Facades composing the System Call API.

pub mod a2a_api;
pub mod agent_api;
pub mod browser_api;
pub mod engine_api;
pub mod exec_api;
pub mod extension_api;
pub mod infra_api;
pub mod knowledge_lens;
pub mod marketplace_api;
pub mod mcp_api;
pub mod persona_api;
pub mod project_api;
pub mod security_api;
pub mod state_api;

pub use a2a_api::A2aApi;
pub use agent_api::AgentApi;
pub use browser_api::BrowserApi;
pub use engine_api::{
    EngineApi, EngineConfigResponse, FallbackEvent, ModelInfo, ProviderInfo, RoutingConfigSnapshot,
    RoutingStats, RoutingStatsSnapshot, RoutingUpdate, ValidateKeyResult,
};
pub use exec_api::ExecApi;
pub use extension_api::ExtensionApi;
pub use infra_api::InfraApi;
pub use knowledge_lens::{
    CopilotResponse, KnowledgeContext, KnowledgeLens, KnowledgeNote, MemoryNote,
};
pub use marketplace_api::MarketplaceApi;
pub use mcp_api::McpApi;
pub use persona_api::PersonaApi;
pub use project_api::{ProjectApi, ProjectInfo};
pub use security_api::SecurityApi;
pub use state_api::StateApi;

use crate::a2a::A2AProtocol;
use crate::access_manager::AccessManager;
use crate::audit_trail::AuditTrail;
use crate::auth::AuthManager;
use crate::budget::BudgetManager;
use crate::clawhub::{ClawHubClient, ClawHubInstaller};
use crate::config::OxiosConfig;
use crate::cron::CronScheduler;
use crate::event_bus::EventBus;
use crate::git_layer::CommitInfo;
use crate::git_layer::GitLayer;
use crate::mcp::McpBridge;
use crate::memory::MemoryManager;
use crate::persona_manager::PersonaManager;
use crate::resource_monitor::ResourceMonitor;
use crate::scheduler::AgentScheduler;
use crate::skill::SkillManager;
use crate::state_store::StateStore;
use crate::supervisor::Supervisor;
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;

/// Oxios kernel System Call API — composed of 13 domain Facades.
///
/// Each Facade groups related system calls:
/// - [`StateApi`]     — data persistence, sessions
/// - [`AgentApi`]     — agent lifecycle, budgets, memory
/// - [`SecurityApi`]  — auth, audit trail, RBAC, approvals
/// - [`PersonaApi`]   — multi-persona management
/// - [`ExtensionApi`] — programs, skills, host tools
/// - [`McpApi`]       — MCP server bridge
/// - [`ProjectApi`]    — Project management, memory linking
/// - [`ExecApi`]      — execution config, access management
/// - [`BrowserApi`]   — browser backend
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
    /// Execution: config + access management.
    pub exec: ExecApi,
    /// Browser backend (zero-sized when `browser` feature is disabled).
    pub browser: BrowserApi,
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
}

impl KernelHandle {
    /// Create a new KernelHandle from 13 domain Facades.
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
        mcp: McpApi,
        infra: InfraApi,
        projects: Option<ProjectApi>,
        exec: ExecApi,
        browser: BrowserApi,
        a2a: A2aApi,
        engine: EngineApi,
        knowledge: Arc<oxios_markdown::KnowledgeBase>,
        knowledge_lens: Arc<KnowledgeLens>,
        marketplace_api: MarketplaceApi,
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
            exec,
            browser,
            a2a,
            engine,
            knowledge,
            knowledge_lens,
            marketplace_api,
        }
    }

    /// Build a KernelHandle from raw subsystem parameters.
    ///
    /// Prefer [`KernelHandle::new()`] which takes pre-built Facades.
    #[deprecated(note = "Use KernelHandle::new() with pre-built Facades instead")]
    #[allow(clippy::too_many_arguments)]
    pub fn from_subsystems(
        state_store: Arc<StateStore>,
        event_bus: EventBus,
        supervisor: Arc<dyn Supervisor>,
        scheduler: Arc<AgentScheduler>,
        memory_manager: Arc<MemoryManager>,
        git_layer: Arc<GitLayer>,
        audit_trail: Arc<AuditTrail>,
        budget_manager: Arc<BudgetManager>,
        resource_monitor: Arc<ResourceMonitor>,
        cron_scheduler: Arc<CronScheduler>,
        skill_manager: Arc<SkillManager>,
        persona_manager: Arc<PersonaManager>,
        mcp_bridge: Arc<McpBridge>,
        auth_manager: Arc<parking_lot::Mutex<AuthManager>>,
        access_manager: Arc<parking_lot::Mutex<AccessManager>>,
        config: OxiosConfig,
        start_time: Instant,
    ) -> Self {
        let knowledge_dir = state_store.base_path.join("knowledge");
        let knowledge = Arc::new(
            oxios_markdown::KnowledgeBase::new(knowledge_dir)
                .expect("Failed to create KnowledgeBase"),
        );
        let knowledge_lens = Arc::new(
            KnowledgeLens::new(knowledge.clone(), memory_manager.clone())
                .expect("Failed to create KnowledgeLens"),
        );
        Self {
            security: SecurityApi::new(
                auth_manager.clone(),
                audit_trail,
                access_manager.clone(),
                state_store.clone(),
            ),
            state: StateApi::new(state_store.clone()),
            agents: AgentApi::new(
                supervisor,
                budget_manager,
                memory_manager,
                Some(event_bus.clone()),
            ),
            persona: PersonaApi::new(persona_manager),
            extensions: ExtensionApi::new(skill_manager),
            mcp: McpApi::new(mcp_bridge),
            infra: InfraApi::new(
                git_layer,
                scheduler,
                cron_scheduler,
                resource_monitor,
                event_bus.clone(),
                config.clone(),
                start_time,
            ),
            projects: None,
            exec: ExecApi::new(Arc::new(config.exec.clone()), access_manager),
            #[allow(clippy::default_trait_access)]
            browser: BrowserApi::default(),
            a2a: A2aApi::new(Arc::new(A2AProtocol::new(crate::EventBus::new(0)))),
            engine: EngineApi::new(
                Arc::new(parking_lot::RwLock::new(config.clone())),
                std::path::PathBuf::from("~/.oxios/config.toml"),
                Arc::new(RoutingStats::new()),
            ),
            knowledge,
            knowledge_lens,
            marketplace_api: MarketplaceApi::new(
                Arc::new(ClawHubInstaller::new(
                    state_store.base_path.join("skills"),
                    state_store.base_path.clone(),
                    config.marketplace.base_url.clone(),
                )),
                Arc::new(
                    ClawHubClient::new(config.marketplace.base_url.clone())
                        .expect("valid ClawHub client"),
                ),
            ),
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Convenience methods (cross-Facades orchestration)
    // ═══════════════════════════════════════════════════════════════════════

    /// Save data and commit to git (State + Infra).
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
            let _ = git.commit_file(&rel_path, &format!("save {category}/{name}"));
        }
        Ok(())
    }

    /// Save markdown and commit to git (State + Infra).
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
            let _ = git.commit_file(&rel_path, &format!("save {category}/{name}"));
        }
        Ok(())
    }

    /// Delete a file and commit the removal to git (State + Infra).
    pub async fn delete_and_commit(&self, category: &str, name: &str) -> anyhow::Result<bool> {
        let deleted = self.state.delete(category, name).await?;
        if deleted {
            let git = self.infra.git();
            if git.is_enabled() {
                let rel_path = format!("{category}/{name}.json");
                let _ = git.remove_file(&rel_path, &format!("delete {category}/{name}"));
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
    pub async fn schedule(
        &self,
        cron_expr: &str,
        task: &str,
        persona: Option<&str>,
    ) -> anyhow::Result<String> {
        let _persona = persona.unwrap_or("default");
        let job = crate::cron::CronJob::new(
            format!("job_{}", uuid::Uuid::new_v4()),
            cron_expr.to_string(),
            task.to_string(),
        );
        let job_id = self.infra.add_cron(job).await?;
        Ok(job_id.to_string())
    }

    /// Unschedule a cron job by string ID (convenience wrapper).
    pub async fn unschedule(&self, job_id: &str) -> anyhow::Result<bool> {
        let uuid =
            uuid::Uuid::parse_str(job_id).map_err(|e| anyhow::anyhow!("invalid job id: {e}"))?;
        match self.infra.remove_cron(uuid).await {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// List cron jobs (convenience wrapper).
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
}
