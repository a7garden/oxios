//! Kernel facade — 7 domain Facades composing the System Call API.

pub mod state_api;
pub mod agent_api;
pub mod security_api;
pub mod persona_api;
pub mod extension_api;
pub mod mcp_api;
pub mod infra_api;

pub use state_api::StateApi;
pub use agent_api::AgentApi;
pub use security_api::SecurityApi;
pub use persona_api::PersonaApi;
pub use extension_api::ExtensionApi;
pub use mcp_api::McpApi;
pub use infra_api::InfraApi;

use serde::Serialize;
use crate::git_layer::CommitInfo;

/// Oxios kernel System Call API — composed of 7 domain Facades.
///
/// Each Facade groups related system calls:
/// - [`StateApi`]    — data persistence, sessions
/// - [`AgentApi`]    — agent lifecycle, budgets, memory
/// - [`SecurityApi`] — auth, audit trail, RBAC, approvals
/// - [`PersonaApi`]  — multi-persona management
/// - [`ExtensionApi`] — programs, skills, host tools
/// - [`McpApi`]      — MCP server bridge
/// - [`InfraApi`]    — Git, scheduler, cron, resources, events, system
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
}

impl KernelHandle {
    /// Create a new KernelHandle from 7 domain Facades.
    ///
    /// Each Facade is assembled independently in `kernel.rs` and passed here.
    /// This enables testing individual Facades without the full kernel.
    pub fn new(
        state: StateApi,
        agents: AgentApi,
        security: SecurityApi,
        persona: PersonaApi,
        extensions: ExtensionApi,
        mcp: McpApi,
        infra: InfraApi,
    ) -> Self {
        Self {
            state,
            agents,
            security,
            persona,
            extensions,
            mcp,
            infra,
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
    pub async fn delete_and_commit(
        &self,
        category: &str,
        name: &str,
    ) -> anyhow::Result<bool> {
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
        let uuid = uuid::Uuid::parse_str(job_id)
            .map_err(|e| anyhow::anyhow!("invalid job id: {e}"))?;
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
    pub async fn load_json<T: serde::de::DeserializeOwned>(&self, category: &str, name: &str) -> anyhow::Result<Option<T>> {
        self.state.load(category, name).await
    }
}
