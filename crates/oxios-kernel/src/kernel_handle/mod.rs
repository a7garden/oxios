//! Kernel facade — 12 domain Facades composing the System Call API.

pub mod a2a_api;
pub mod agent_api;
pub mod browser_api;
pub mod exec_api;
pub mod extension_api;
pub mod infra_api;
pub mod knowledge_lens;
pub mod mcp_api;
pub mod persona_api;
pub mod security_api;
pub mod space_api;
pub mod state_api;

pub use a2a_api::A2aApi;
pub use agent_api::AgentApi;
pub use browser_api::BrowserApi;
pub use exec_api::ExecApi;
pub use extension_api::ExtensionApi;
pub use infra_api::InfraApi;
pub use knowledge_lens::{
    CopilotResponse, KnowledgeContext, KnowledgeLens, KnowledgeNote, MemoryNote,
};
pub use mcp_api::McpApi;
pub use persona_api::PersonaApi;
pub use security_api::SecurityApi;
pub use space_api::SpaceApi;
pub use state_api::StateApi;

use crate::a2a::A2AProtocol;
use crate::access_manager::AccessManager;
use crate::audit_trail::AuditTrail;
use crate::auth::AuthManager;
use crate::budget::BudgetManager;
use crate::config::OxiosConfig;
use crate::cron::CronScheduler;
use crate::event_bus::EventBus;
use crate::git_layer::CommitInfo;
use crate::git_layer::GitLayer;
use crate::host_tools::HostToolValidator;
use crate::mcp::McpBridge;
use crate::memory::MemoryManager;
use crate::persona_manager::PersonaManager;
use crate::resource_monitor::ResourceMonitor;
use crate::scheduler::AgentScheduler;
use crate::skill::SkillManager;
use crate::space::SpaceManager;
use crate::state_store::StateStore;
use crate::supervisor::Supervisor;
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;

/// Oxios kernel System Call API — composed of 12 domain Facades.
///
/// Each Facade groups related system calls:
/// - [`StateApi`]     — data persistence, sessions
/// - [`AgentApi`]     — agent lifecycle, budgets, memory
/// - [`SecurityApi`]  — auth, audit trail, RBAC, approvals
/// - [`PersonaApi`]   — multi-persona management
/// - [`ExtensionApi`] — programs, skills, host tools
/// - [`McpApi`]       — MCP server bridge
/// - [`SpaceApi`]     — Space management, knowledge flow
/// - [`ExecApi`]      — execution config, access management
/// - [`BrowserApi`]   — browser backend
/// - [`A2aApi`]       — agent-to-agent communication
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
    /// Space management: context partitioning, knowledge flow.
    pub spaces: SpaceApi,
    /// Execution: config + access management.
    pub exec: ExecApi,
    /// Browser backend (zero-sized when `browser` feature is disabled).
    pub browser: BrowserApi,
    /// Agent-to-agent communication.
    pub a2a: A2aApi,
    /// Knowledge base: markdown notes (direct access, no kernel dependency).
    pub knowledge: Arc<oxios_markdown::KnowledgeBase>,
    /// Semantic knowledge overlay (HNSW index + agent recall).
    pub knowledge_lens: Arc<KnowledgeLens>,
}

impl KernelHandle {
    /// Create a new KernelHandle from 12 domain Facades.
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
        spaces: SpaceApi,
        exec: ExecApi,
        browser: BrowserApi,
        a2a: A2aApi,
        knowledge: Arc<oxios_markdown::KnowledgeBase>,
        knowledge_lens: Arc<KnowledgeLens>,
    ) -> Self {
        Self {
            state,
            agents,
            security,
            persona,
            extensions,
            mcp,
            infra,
            spaces,
            exec,
            browser,
            a2a,
            knowledge,
            knowledge_lens,
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
        program_manager: Arc<crate::program::ProgramManager>,
        skill_store: Arc<crate::skill::SkillStore>,
        persona_manager: Arc<PersonaManager>,
        mcp_bridge: Arc<McpBridge>,
        auth_manager: Arc<parking_lot::Mutex<AuthManager>>,
        access_manager: Arc<parking_lot::Mutex<AccessManager>>,
        host_tool_validator: Arc<HostToolValidator>,
        config: OxiosConfig,
        start_time: Instant,
        space_manager: Arc<SpaceManager>,
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
            state: StateApi::new(state_store),
            agents: AgentApi::new(
                supervisor,
                budget_manager,
                memory_manager,
                Some(event_bus.clone()),
            ),
            persona: PersonaApi::new(persona_manager),
            extensions: ExtensionApi::new(
                Arc::new(SkillManager::new(
                    skill_store.path().clone(),
                    skill_store.path().clone().join("../share/skills"),
                )),
                program_manager,
                host_tool_validator,
            ),
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
            spaces: SpaceApi::new(space_manager, event_bus),
            exec: ExecApi::new(Arc::new(config.exec.clone()), access_manager),
            #[allow(clippy::default_trait_access)]
            browser: BrowserApi::default(),
            a2a: A2aApi::new(Arc::new(A2AProtocol::new(crate::EventBus::new(0)))),
            knowledge,
            knowledge_lens,
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
            let rel_path = format!("{}/{}.json", category, name);
            let _ = git.commit_file(&rel_path, &format!("save {}/{}", category, name));
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
            let rel_path = format!("{}/{}.md", category, name);
            let _ = git.commit_file(&rel_path, &format!("save {}/{}", category, name));
        }
        Ok(())
    }

    /// Delete a file and commit the removal to git (State + Infra).
    pub async fn delete_and_commit(&self, category: &str, name: &str) -> anyhow::Result<bool> {
        let deleted = self.state.delete(category, name).await?;
        if deleted {
            let git = self.infra.git();
            if git.is_enabled() {
                let rel_path = format!("{}/{}.json", category, name);
                let _ = git.remove_file(&rel_path, &format!("delete {}/{}", category, name));
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

    /// Activate a Space.
    ///
    /// Note: Knowledge base is global (`~/.oxios/knowledge/`), not Space-scoped.
    /// Space activation only switches the agent's execution context and memory graph.
    pub async fn activate_space(&self, id: &str) -> anyhow::Result<()> {
        self.spaces.activate(id).await?;
        tracing::info!(space_id = %id, "Space activated (knowledge base is global)");
        Ok(())
    }
}
