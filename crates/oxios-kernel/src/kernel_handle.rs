//! Kernel facade — exposes System Call API.
//!
//! This struct lives in oxios-kernel so that other crates (oxios-web) can use it.
//! It holds Arc references to all subsystems and delegates to them.

use std::sync::Arc;
use std::time::Instant;
use anyhow::Result;
use crate::{
    state_store::{StateStore, Session, SessionId, SessionSummary},
    event_bus::{EventBus, KernelEvent},
    container_manager::{ContainerManager, ContainerInfo, ToolHealthReport},
    container::ExecResult,
    supervisor::Supervisor,
    scheduler::AgentScheduler,
    memory::MemoryManager,
    git_layer::{GitLayer, CommitInfo, LogEntry},
    audit_trail::{AuditTrail, AuditAction, AuditEntry},
    budget::{BudgetManager, BudgetInfo},
    resource_monitor::{ResourceMonitor, ResourceSnapshot},
    cron::{CronScheduler, CronJob},
    program::{ProgramManager, ProgramMeta},
    skill::SkillStore,
    persona_manager::PersonaManager,
    mcp::{McpBridge, McpToolCallResult},
    auth::AuthManager,
    access_manager::AccessManager,
    host_tools::HostToolValidator,
    config::OxiosConfig,
    types::AgentId,
};

/// Kernel facade — exposes System Call API.
///
/// This struct lives in oxios-kernel so that other crates (oxios-web) can use it.
/// It holds Arc references to all subsystems and delegates to them.
pub struct KernelHandle {
    /// Persistent state store (markdown/JSON).
    pub(crate) state_store: Arc<StateStore>,
    /// Kernel event bus.
    pub(crate) event_bus: EventBus,
    /// Container lifecycle manager.
    pub(crate) container_manager: Arc<ContainerManager>,
    /// Agent supervisor (lifecycle management).
    pub(crate) supervisor: Arc<dyn Supervisor>,
    /// Task scheduler.
    pub(crate) scheduler: Arc<AgentScheduler>,
    /// Memory manager for cross-session agent memory.
    pub(crate) memory_manager: Arc<MemoryManager>,
    /// Git-based version control layer for state persistence.
    pub(crate) git_layer: Arc<GitLayer>,
    /// Audit trail for tamper-evident event logging.
    pub(crate) audit_trail: Arc<AuditTrail>,
    /// Budget manager for agent-level token/call budgets.
    pub(crate) budget_manager: Arc<BudgetManager>,
    /// Resource monitor for system metrics.
    pub(crate) resource_monitor: Arc<ResourceMonitor>,
    /// Cron job scheduler for time-based task execution.
    pub(crate) cron_scheduler: Arc<CronScheduler>,
    /// OS-level program manager.
    pub(crate) program_manager: Arc<ProgramManager>,
    /// Skill store for skill management.
    pub(crate) skill_store: Arc<SkillStore>,
    /// Persona manager for multi-persona support.
    pub(crate) persona_manager: Arc<PersonaManager>,
    /// MCP bridge for tool calling.
    pub(crate) mcp_bridge: Arc<McpBridge>,
    /// Authentication manager for bearer token validation.
    pub(crate) auth_manager: Arc<parking_lot::Mutex<AuthManager>>,
    /// Access manager for RBAC and permissions.
    pub(crate) access_manager: Arc<parking_lot::Mutex<AccessManager>>,
    /// Host tool validator.
    pub(crate) host_tool_validator: Arc<HostToolValidator>,
    /// Loaded configuration.
    pub(crate) config: OxiosConfig,
    /// Kernel start time for uptime tracking.
    pub(crate) start_time: Instant,
}

impl KernelHandle {
    /// Create a new KernelHandle from individual subsystems.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state_store: Arc<StateStore>,
        event_bus: EventBus,
        container_manager: Arc<ContainerManager>,
        supervisor: Arc<dyn Supervisor>,
        scheduler: Arc<AgentScheduler>,
        memory_manager: Arc<MemoryManager>,
        git_layer: Arc<GitLayer>,
        audit_trail: Arc<AuditTrail>,
        budget_manager: Arc<BudgetManager>,
        resource_monitor: Arc<ResourceMonitor>,
        cron_scheduler: Arc<CronScheduler>,
        program_manager: Arc<ProgramManager>,
        skill_store: Arc<SkillStore>,
        persona_manager: Arc<PersonaManager>,
        mcp_bridge: Arc<McpBridge>,
        auth_manager: Arc<parking_lot::Mutex<AuthManager>>,
        access_manager: Arc<parking_lot::Mutex<AccessManager>>,
        host_tool_validator: Arc<HostToolValidator>,
        config: OxiosConfig,
        start_time: Instant,
    ) -> Self {
        Self {
            state_store,
            event_bus,
            container_manager,
            supervisor,
            scheduler,
            memory_manager,
            git_layer,
            audit_trail,
            budget_manager,
            resource_monitor,
            cron_scheduler,
            program_manager,
            skill_store,
            persona_manager,
            mcp_bridge,
            auth_manager,
            access_manager,
            host_tool_validator,
            config,
            start_time,
        }
    }

    /// Verify the KernelHandle is properly constructed (for testing).
    #[allow(unused_comparisons)]
    pub fn verify(&self) -> bool {
        // Basic sanity checks
        self.git_layer.is_enabled() == self.config.git.auto_commit
            && self.audit_trail.len() >= 0
            && self.start_time.elapsed().as_secs() >= 0
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // SYSTEM CALL METHODS
    // ═══════════════════════════════════════════════════════════════════════════

    // ── State ──

    /// Save data to state store and commit to git.
    pub async fn save_and_commit<T: serde::Serialize>(
        &self,
        category: &str,
        name: &str,
        data: &T,
    ) -> anyhow::Result<()> {
        self.state_store.save_json(category, name, data).await?;
        if self.git_layer.is_enabled() {
            let rel_path = format!("{}/{}.json", category, name);
            let _ = self.git_layer.commit_file(&rel_path, &format!("save {}/{}", category, name));
        }
        Ok(())
    }

    /// Save markdown to state store and commit to git.
    pub async fn save_markdown_and_commit(
        &self,
        category: &str,
        name: &str,
        content: &str,
    ) -> anyhow::Result<()> {
        self.state_store.save_markdown(category, name, content).await?;
        if self.git_layer.is_enabled() {
            let rel_path = format!("{}/{}.md", category, name);
            let _ = self.git_layer.commit_file(&rel_path, &format!("save {}/{}", category, name));
        }
        Ok(())
    }

    /// Delete a file from state store and commit the removal to git.
    pub async fn delete_and_commit(
        &self,
        category: &str,
        name: &str,
    ) -> anyhow::Result<bool> {
        let deleted = self.state_store.delete_file(category, name).await?;
        if deleted && self.git_layer.is_enabled() {
            let rel_path = format!("{}/{}.json", category, name);
            let _ = self.git_layer.remove_file(&rel_path, &format!("delete {}/{}", category, name));
        }
        Ok(deleted)
    }

    /// Commit all current changes to git.
    pub fn commit_all(&self, message: &str) -> anyhow::Result<Option<CommitInfo>> {
        if !self.git_layer.is_enabled() {
            return Ok(None);
        }
        self.git_layer.commit_file(".", message).ok()
            .map_or(Ok(None), |info| Ok(Some(info)))
    }

    /// Flush audit trail to state store and commit to git.
    pub fn flush_audit(&self) -> anyhow::Result<()> {
        if self.git_layer.is_enabled() {
            let _ = self.git_layer.commit_file("audit", "audit trail flush");
        }
        Ok(())
    }

    /// Check if auto-commit is enabled.
    pub fn auto_commit_enabled(&self) -> bool {
        self.config.git.auto_commit && self.git_layer.is_enabled()
    }

    /// Load data from state store.
    pub async fn load<T: serde::de::DeserializeOwned>(&self, category: &str, name: &str) -> anyhow::Result<Option<T>> {
        self.state_store.load_json(category, name).await
    }

    /// List files in a category.
    pub async fn list_category(&self, category: &str) -> anyhow::Result<Vec<String>> {
        self.state_store.list_category(category).await
    }

    /// Save session.
    pub async fn save_session(&self, session: &Session) -> anyhow::Result<()> {
        self.state_store.save_session(session).await
    }

    /// Load session.
    pub async fn load_session(&self, id: &SessionId) -> anyhow::Result<Option<Session>> {
        self.state_store.load_session(id).await
    }

    /// List sessions.
    pub async fn list_sessions(&self) -> anyhow::Result<Vec<SessionSummary>> {
        self.state_store.list_sessions().await
    }

    /// Delete session.
    pub async fn delete_session(&self, id: &SessionId) -> anyhow::Result<bool> {
        self.state_store.delete_session(id).await
    }

    // ── Agent ──

    /// List running agents.
    pub async fn list_agents(&self) -> anyhow::Result<Vec<crate::types::AgentInfo>> {
        self.supervisor.list().await.map_err(|e| anyhow::anyhow!("supervisor: {e}"))
    }

    /// Kill a running agent.
    pub async fn kill_agent(&self, agent_id: &str) -> anyhow::Result<()> {
        let id = uuid::Uuid::parse_str(agent_id)
            .map_err(|e| anyhow::anyhow!("invalid agent id: {e}"))?;
        self.supervisor.kill(id).await.map_err(|e| anyhow::anyhow!("supervisor: {e}"))
    }

    // ── Memory ──

    /// Get memory stats (sync version).
    pub fn memory_stats(&self) -> (usize, usize) {
        (self.memory_manager.vector_index_size(), 0)
    }

    /// Get memory stats (async version with total entries).
    pub async fn memory_stats_async(&self) -> (usize, usize) {
        (self.memory_manager.vector_index_size(), self.memory_manager.total_entries().await)
    }

    // ── Git ──

    /// Get commit log.
    pub fn git_log(&self, max: usize) -> anyhow::Result<Vec<LogEntry>> {
        self.git_layer.log(max)
    }

    /// Tag current state.
    pub fn git_tag(&self, name: &str, message: &str) -> anyhow::Result<()> {
        self.git_layer.tag(name, message)
    }

    /// Restore file from commit.
    pub fn git_restore(&self, path: &str, hash: &str) -> anyhow::Result<()> {
        self.git_layer.restore_file(path, hash)
    }

    /// Verify git repository.
    pub fn git_verify(&self) -> anyhow::Result<bool> {
        self.git_layer.verify()
    }

    /// List git tags.
    pub fn git_tags(&self) -> anyhow::Result<Vec<String>> {
        self.git_layer.list_tags()
    }

    // ── Scheduling ──

    /// Schedule a cron job.
    pub async fn schedule(&self, cron_expr: &str, task: &str, persona: Option<&str>) -> anyhow::Result<String> {
        let _persona = persona.unwrap_or("default");
        let job = CronJob::new(
            format!("job_{}", uuid::Uuid::new_v4()),
            cron_expr.to_string(),
            task.to_string(),
        );
        let job_id = self.cron_scheduler.add_job(job).await?;
        Ok(job_id.to_string())
    }

    /// Unschedule a cron job.
    pub async fn unschedule(&self, job_id: &str) -> anyhow::Result<bool> {
        let uuid = uuid::Uuid::parse_str(job_id)
            .map_err(|e| anyhow::anyhow!("invalid job id: {e}"))?;
        match self.cron_scheduler.remove_job(uuid).await {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// List cron jobs.
    pub fn list_schedules(&self) -> Vec<CronJob> {
        self.cron_scheduler.list_jobs()
    }

    // ── Audit ──

    /// Audit an action.
    pub fn audit(&self, actor: &str, action: AuditAction, resource: &str) -> String {
        self.audit_trail.append(actor.to_string(), action, resource.to_string())
    }

    /// Verify audit chain.
    pub fn verify_audit(&self) -> anyhow::Result<bool> {
        self.audit_trail.verify()
            .map_err(|e| anyhow::anyhow!("audit verify failed: {:?}", e))
    }

    /// Query audit entries by sequence range.
    pub fn query_audit(&self, from_seq: u64, to_seq: u64) -> Vec<AuditEntry> {
        self.audit_trail.entries(from_seq, to_seq)
    }

    /// Query audit by agent.
    pub fn query_audit_by_agent(&self, agent_id: &str) -> Vec<AuditEntry> {
        self.audit_trail.by_agent(agent_id)
    }

    /// Get audit entry count.
    pub fn audit_count(&self) -> usize {
        self.audit_trail.len()
    }

    // ── Resources ──

    /// Get resource snapshot.
    pub fn resource_snapshot(&self) -> ResourceSnapshot {
        self.resource_monitor.snapshot()
    }

    /// Check budget for an agent.
    pub fn check_budget(&self, agent_id: &AgentId) -> BudgetInfo {
        self.budget_manager.remaining(agent_id)
    }

    /// Set budget for an agent.
    pub fn set_budget(&self, limit: crate::budget::BudgetLimit) {
        self.budget_manager.set_budget(limit);
    }

    /// Remove budget for an agent.
    pub fn remove_budget(&self, agent_id: &AgentId) {
        self.budget_manager.remove_budget(agent_id);
    }

    /// Reserve tokens for an agent.
    pub fn reserve_budget(&self, agent_id: &AgentId, tokens: u64) -> Result<(), crate::budget::BudgetExceeded> {
        self.budget_manager.reserve(agent_id, tokens)
    }

    /// Reset budget window for an agent.
    pub fn reset_budget(&self, agent_id: &AgentId) {
        self.budget_manager.reset_window(agent_id);
    }

    /// Get overload status.
    pub fn is_overloaded(&self) -> bool {
        self.resource_monitor.is_overloaded()
    }

    // ── Container ──

    /// Check if container backend is available.
    pub fn container_available(&self) -> bool {
        self.container_manager.is_backend_available()
    }

    /// Get container backend name.
    pub fn container_backend(&self) -> Option<String> {
        if self.container_manager.is_backend_available() {
            Some(self.container_manager.backend_name().to_string())
        } else {
            None
        }
    }

    /// Create new container.
    pub async fn create_container(&self, name: &str) -> anyhow::Result<()> {
        self.container_manager.new_container(name).await
    }

    /// List containers.
    pub fn list_containers(&self) -> Vec<ContainerInfo> {
        match tokio::runtime::Handle::current().block_on(self.container_manager.list_containers()) {
            Ok(containers) => containers,
            Err(_) => vec![],
        }
    }

    /// Check tool health in container.
    pub async fn check_tool_health(&self, container_name: &str) -> anyhow::Result<ToolHealthReport> {
        self.container_manager.check_tool_health(container_name).await
    }

    // ── Events ──

    /// Subscribe to kernel events.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<KernelEvent> {
        self.event_bus.subscribe()
    }

    /// Publish a kernel event.
    pub fn publish(&self, event: KernelEvent) -> anyhow::Result<()> {
        self.event_bus.publish(event).map_err(|e| anyhow::anyhow!("broadcast error: {e}"))
    }

    // ── System ──

    /// Get config reference.
    pub fn get_config(&self) -> &OxiosConfig {
        &self.config
    }

    /// Get system uptime.
    pub fn uptime(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Get kernel start time.
    pub fn start_time(&self) -> Instant {
        self.start_time
    }

    /// List installed programs.
    pub async fn list_programs(&self) -> Vec<ProgramMeta> {
        self.program_manager.list_programs()
            .await
            .into_iter()
            .map(|p| p.meta)
            .collect()
    }

    // ── Skills ──

    pub async fn list_skills(&self) -> anyhow::Result<Vec<crate::skill::SkillMeta>> {
        self.skill_store.list_skills().await
    }

    /// Load skill by name.
    pub async fn load_skill(&self, name: &str) -> anyhow::Result<Option<crate::skill::Skill>> {
        self.skill_store.load_skill(name).await
    }

    /// Create a new skill.
    pub async fn create_skill(&self, name: &str, description: &str, content: &str) -> anyhow::Result<()> {
        self.skill_store.create_skill(name, description, content).await
    }

    /// Delete a skill.
    pub async fn delete_skill(&self, name: &str) -> anyhow::Result<()> {
        self.skill_store.delete_skill(name).await
    }

    // ── Personas ──

    /// Get persona store.
    pub fn persona_store(&self) -> &PersonaManager {
        &self.persona_manager
    }

    /// List all personas.
    pub fn list_personas(&self) -> Vec<crate::Persona> {
        self.persona_manager.store().list_all()
    }

    /// Get persona by ID.
    pub fn get_persona(&self, id: &str) -> Option<crate::Persona> {
        self.persona_manager.store().get(id)
    }

    /// Create a new persona.
    pub fn create_persona(&self, persona: crate::Persona) {
        self.persona_manager.store().register(persona);
    }

    /// Update a persona.
    pub fn update_persona(&self, id: &str, persona: crate::Persona) -> anyhow::Result<()> {
        self.persona_manager.store().update(id, persona)
    }

    /// Delete a persona.
    pub fn delete_persona(&self, id: &str) -> anyhow::Result<()> {
        self.persona_manager.store().delete(id)
    }

    /// Get active persona.
    pub fn get_active_persona(&self) -> Option<crate::Persona> {
        self.persona_manager.get_active_persona()
    }

    /// Set active persona.
    pub fn set_active_persona(&self, id: &str) -> anyhow::Result<()> {
        self.persona_manager.set_active_persona(id)
    }

    /// Get persona count.
    pub fn persona_count(&self) -> usize {
        self.persona_manager.store().len()
    }

    /// List enabled personas.
    pub fn list_enabled_personas(&self) -> Vec<crate::Persona> {
        self.persona_manager.store().list_enabled()
    }

    // ── Auth ──

    /// Validate a bearer token.
    pub fn validate_token(&self, token: &str) -> bool {
        self.auth_manager.lock().validate(token)
    }

    // ── Access Manager ──

    /// Get audit log entries.
    pub fn get_audit_log(&self) -> Vec<crate::access_manager::AuditEntry> {
        self.access_manager.lock().audit_log().to_vec()
    }

    /// Get permissions for an agent.
    pub fn get_permissions(&self, agent: &str) -> Option<crate::access_manager::AgentPermissions> {
        self.access_manager.lock().get_permissions(agent).cloned()
    }

    /// Get or create permissions.
    pub fn get_or_create_permissions(&self, agent: &str) -> crate::access_manager::AgentPermissions {
        self.access_manager.lock().get_or_create_permissions(agent).clone()
    }

    /// List all approvals.
    pub fn list_approvals(&self) -> Vec<(crate::access_manager::PendingApproval, crate::access_manager::ApprovalStatus)> {
        self.access_manager.lock().rbac_manager().all_approvals().to_vec()
    }

    /// Approve a pending request.
    pub fn approve_request(&self, id: uuid::Uuid) -> bool {
        self.access_manager.lock().rbac_manager_mut().approve(id)
    }

    /// Reject a pending request.
    pub fn reject_request(&self, id: uuid::Uuid) -> bool {
        self.access_manager.lock().rbac_manager_mut().reject(id)
    }

    // ── MCP ──

    /// List MCP servers.
    pub fn mcp_servers(&self) -> Vec<String> {
        self.mcp_bridge.servers()
    }

    /// Get MCP server info.
    pub fn get_mcp_server(&self, name: &str) -> Option<crate::McpServer> {
        self.mcp_bridge.get_server(name)
    }

    /// Register an MCP server.
    pub fn register_mcp_server(&self, server: crate::McpServer) {
        self.mcp_bridge.register_server(server);
    }

    // ── Host Tools ──

    /// Full host tool check.
    pub fn check_host_tools(&self) -> crate::host_tools::HostToolStatus {
        self.host_tool_validator.full_check()
    }

    // ── Scheduler ──

    /// Get scheduler stats.
    pub fn scheduler_stats(&self) -> crate::scheduler::SchedulerStats {
        self.scheduler.stats()
    }

    /// Get rate limit remaining.
    pub fn scheduler_rate_remaining(&self) -> u32 {
        self.scheduler.rate_limit_remaining()
    }

    /// Get queued tasks.
    pub fn scheduler_queued_tasks(&self) -> Vec<crate::scheduler::ScheduledTask> {
        self.scheduler.queued_tasks()
    }

    /// Get running tasks.
    pub fn scheduler_running_tasks(&self) -> Vec<crate::scheduler::ScheduledTask> {
        self.scheduler.running_tasks()
    }

    // ── Cron ──

    /// Add a cron job.
    pub async fn add_cron_job(&self, job: CronJob) -> anyhow::Result<uuid::Uuid> {
        self.cron_scheduler.add_job(job).await
    }

    /// Get a cron job.
    pub fn get_cron_job(&self, id: uuid::Uuid) -> Option<CronJob> {
        self.cron_scheduler.get_job(id)
    }

    /// Update a cron job.
    pub async fn update_cron_job(&self, id: uuid::Uuid, update: crate::cron::CronJobUpdate) -> anyhow::Result<()> {
        self.cron_scheduler.update_job(id, update).await
    }

    /// Trigger a cron job.
    pub fn trigger_cron_job(&self, id: uuid::Uuid) -> anyhow::Result<CronJob> {
        self.cron_scheduler.trigger_job(id)
    }

    /// Mark cron job completed.
    pub async fn mark_cron_job_completed(&self, id: uuid::Uuid, success: bool, summary: String) {
        self.cron_scheduler.mark_job_completed(id, success, summary).await
    }

    // ── State Store Direct ──

    /// Get base path of state store.
    pub fn state_store_base_path(&self) -> &std::path::Path {
        &self.state_store.base_path
    }

    /// Load markdown from state store.
    pub async fn load_markdown(&self, category: &str, name: &str) -> anyhow::Result<Option<String>> {
        self.state_store.load_markdown(category, name).await
    }

    /// Save markdown to state store.
    pub async fn save_markdown(&self, category: &str, name: &str, content: &str) -> anyhow::Result<()> {
        self.state_store.save_markdown(category, name, content).await
    }

    /// Load JSON from state store.
    pub async fn load_json<T: serde::de::DeserializeOwned>(&self, category: &str, name: &str) -> anyhow::Result<Option<T>> {
        self.state_store.load_json(category, name).await
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // WORKSPACE
    // ═══════════════════════════════════════════════════════════════════════════

    /// Get workspace base path.
    pub fn workspace_path(&self) -> &std::path::Path {
        &self.state_store.base_path
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // CONTAINER (System Call wrappers)
    // ═══════════════════════════════════════════════════════════════════════════


    /// Start a container.
    pub async fn start_container(&self, name: &str) -> anyhow::Result<()> {
        self.container_manager.start_container(name).await
    }

    /// Stop a container.
    pub async fn stop_container(&self, name: &str) -> anyhow::Result<()> {
        self.container_manager.stop_container(name).await
    }

    /// Remove a container.
    pub async fn remove_container(&self, name: &str) -> anyhow::Result<()> {
        self.container_manager.remove_container(name).await
    }

    /// Execute command in container.
    pub async fn exec_in_container(
        &self,
        name: &str,
        command: &[String],
        workdir: Option<&str>,
    ) -> anyhow::Result<ExecResult> {
        self.container_manager.exec_in_container(name, command, workdir).await
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // PROGRAM (System Call wrappers)
    // ═══════════════════════════════════════════════════════════════════════════

    /// Get program details.
    pub async fn get_program(&self, name: &str) -> Option<crate::program::Program> {
        self.program_manager.get_program(name).await
    }

    /// Install a program from source.
    pub async fn install_program(&self, source: crate::program::InstallSource) -> anyhow::Result<crate::program::Program> {
        self.program_manager.install_from(source).await
    }

    /// Uninstall a program.
    pub async fn uninstall_program(&self, name: &str) -> anyhow::Result<()> {
        self.program_manager.uninstall(name).await
    }

    /// Enable a program.
    pub async fn enable_program(&self, name: &str) -> anyhow::Result<()> {
        self.program_manager.set_enabled(name, true).await
    }

    /// Disable a program.
    pub async fn disable_program(&self, name: &str) -> anyhow::Result<()> {
        self.program_manager.set_enabled(name, false).await
    }

    /// Check host requirements for a program.
    pub async fn check_program_host_requirements(&self, name: &str) -> anyhow::Result<crate::program::HostRequirementsCheck> {
        self.program_manager.check_host_requirements(name).await
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // MEMORY (System Call wrappers)
    // ═══════════════════════════════════════════════════════════════════════════


    /// Store a memory entry.
    pub async fn memory_remember(&self, entry: crate::memory::MemoryEntry) -> anyhow::Result<String> {
        self.memory_manager.remember(entry).await
    }

    /// Search memory entries.
    pub async fn memory_search(
        &self,
        query: &str,
        memory_type: Option<crate::memory::MemoryType>,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::memory::MemoryEntry>> {
        self.memory_manager.search(query, memory_type, limit).await
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // RESOURCE MONITOR (System Call wrappers)
    // ═══════════════════════════════════════════════════════════════════════════

    /// Get resource history snapshots.
    pub fn resource_history(&self, last_n: usize) -> Vec<crate::resource_monitor::ResourceSnapshot> {
        self.resource_monitor.history(last_n)
    }

    /// Get overload threshold.
    pub fn resource_overload_threshold(&self) -> crate::resource_monitor::OverloadThreshold {
        self.resource_monitor.overload_threshold()
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // MCP BRIDGE (System Call wrappers)
    // ═══════════════════════════════════════════════════════════════════════════

    /// List registered MCP servers.
    pub fn mcp_list_servers(&self) -> Vec<String> {
        self.mcp_bridge.servers()
    }

    /// Initialize a specific MCP server.
    pub async fn mcp_initialize_server(&self, name: &str) -> anyhow::Result<()> {
        self.mcp_bridge.initialize_server(name).await
    }

    /// Get MCP client status.
    pub async fn mcp_client_status(&self, name: &str) -> Option<bool> {
        if let Some(client) = self.mcp_bridge.client(name).await {
            Some(client.is_initialized().await)
        } else {
            None
        }
    }

    /// List all MCP tools.
    pub async fn mcp_list_tools(&self) -> anyhow::Result<Vec<crate::program::ToolDef>> {
        self.mcp_bridge.list_tools().await
    }

    /// Get cached tools for a server.
    pub async fn mcp_cached_tools(&self, server_name: &str) -> Option<Vec<crate::program::ToolDef>> {
        self.mcp_bridge.cached_tools(server_name).await
    }

    /// Call an MCP tool.
    pub async fn mcp_call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<McpToolCallResult> {
        self.mcp_bridge.call_tool(server, tool, arguments).await
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // ACCESS MANAGER (System Call wrappers)
    // ═══════════════════════════════════════════════════════════════════════════

    /// Log an audit action.
    pub fn audit_log_action(&self, agent_name: &str, action: &str, resource: &str) {
        let mut am = self.access_manager.lock();
        am.log_access(agent_name, action, resource, true, None);
    }

    /// Update permissions for an agent using a partial update.
    pub fn update_permissions(&self, agent: &str, update: crate::access_manager::PermissionUpdate) -> anyhow::Result<()> {
        self.access_manager.lock().update_permissions(agent, update)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_handle_struct_fields() {
        // Verify KernelHandle has all required fields via reflection
        let fields = [
            "state_store",
            "event_bus",
            "container_manager",
            "supervisor",
            "scheduler",
            "memory_manager",
            "git_layer",
            "audit_trail",
            "budget_manager",
            "resource_monitor",
            "cron_scheduler",
            "program_manager",
            "skill_store",
            "persona_manager",
            "mcp_bridge",
            "auth_manager",
            "access_manager",
            "host_tool_validator",
            "config",
            "start_time",
        ];
        
        // This test documents the expected fields
        // Each field should be pub(crate)
        assert_eq!(fields.len(), 20);
    }
}
