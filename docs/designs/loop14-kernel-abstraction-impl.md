# Loop 14: Kernel Abstraction — Implementation Plan

> ARCHITECTURE.md 기반. 4레이어 구조에 맞게 Kernel 캡슐화.

---

## 1. Goal

```
Before:
  AppState가 25개 Kernel subsystem을 직접 보유
  Routes가 state.state_store.save_json() 직접 호출
  Kernel 필드 21개가 전부 pub

After:
  AppState가 Arc<Kernel> 하나만 보유
  Routes가 kernel.save() 호출
  Kernel 필드 전부 pub(crate), System Call만 pub
```

---

## 2. Phase 1: Kernel System Call 완성

src/kernel.rs `impl Kernel`에 추가:

```rust
// ── State ── (이미 구현됨)
// save_and_commit, save_markdown_and_commit, delete_and_commit

/// Load data from state store.
pub async fn load<T: DeserializeOwned>(&self, category: &str, name: &str) -> Result<Option<T>>

/// List files in a category.
pub async fn list_category(&self, category: &str) -> Result<Vec<String>>

/// Save session.
pub async fn save_session(&self, session: &Session) -> Result<()>

/// Load session.
pub async fn load_session(&self, id: &SessionId) -> Result<Option<Session>>

/// List sessions.
pub async fn list_sessions(&self) -> Result<Vec<SessionSummary>>

/// Delete session.
pub async fn delete_session(&self, id: &SessionId) -> Result<bool>

// ── Agent ──

/// Spawn an agent with a task.
pub async fn spawn(&self, task: &str, persona: Option<&str>) -> Result<AgentId>

/// Wait for agent completion.
pub async fn wait(&self, agent_id: &AgentId) -> Result<String>

/// Kill a running agent.
pub async fn kill(&self, agent_id: &AgentId) -> Result<()>

/// List running agents.
pub async fn list_agents(&self) -> Result<Vec<AgentInfo>>

// ── Memory ──

/// Store a memory entry.
pub async fn remember(&self, category: &str, content: &str, tags: Vec<&str>) -> Result<String>

/// Query memory.
pub async fn recall(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>>

/// Get memory stats.
pub async fn memory_stats(&self) -> (usize, usize)  // (index_size, total_entries)

// ── Git ── (일부 이미 구현됨)

/// Get commit log.
pub fn log(&self, max: usize) -> Result<Vec<LogEntry>>

/// Tag current state.
pub fn tag(&self, name: &str, message: &str) -> Result<()>

/// Restore file from commit.
pub fn restore(&self, path: &str, hash: &str) -> Result<()>

// ── Scheduling ──

/// Schedule a cron job.
pub async fn schedule(&self, cron_expr: &str, task: &str, persona: Option<&str>) -> Result<String>

/// Unschedule a cron job.
pub async fn unschedule(&self, job_id: &str) -> Result<bool>

/// List cron jobs.
pub fn list_schedules(&self) -> Vec<CronJob>

// ── Audit ── (일부 이미 구현됨)

/// Audit an action.
pub fn audit(&self, action: AuditAction, resource: &str) -> HashDigest

/// Verify audit chain.
pub fn verify_audit(&self) -> Result<bool>

/// Query audit entries.
pub fn query_audit(&self, from_seq: u64, to_seq: u64) -> Vec<AuditEntry>

/// Query audit by agent.
pub fn query_audit_by_agent(&self, agent_id: &str) -> Vec<AuditEntry>

/// Get audit entry count.
pub fn audit_count(&self) -> usize

// ── Resources ── (일부 이미 구현됨)

/// Get resource snapshot.
pub fn resources(&self) -> ResourceSnapshot

/// Check budget.
pub fn check_budget(&self, agent_id: &AgentId) -> BudgetInfo

/// Get overload status.
pub fn is_overloaded(&self) -> bool

// ── Container ──

/// Check if container backend is available.
pub fn container_available(&self) -> bool

/// Get container backend name.
pub fn container_backend(&self) -> Option<String>

/// Create new container.
pub async fn create_container(&self, name: &str, toolchain: Option<&str>) -> Result<ContainerInfo>

/// List containers.
pub fn list_containers(&self) -> Vec<ContainerInfo>

/// Check tool health in container.
pub async fn check_tool_health(&self, name: &str) -> Result<ToolHealthReport>

/// Run command in container.
pub async fn exec(&self, container: &str, command: &str) -> Result<String>

// ── Events ──

/// Subscribe to kernel events.
pub fn subscribe(&self) -> broadcast::Receiver<KernelEvent>

// ── System ──

/// Get config.
pub fn get_config(&self) -> &OxiosConfig

/// Check system health.
pub async fn health(&self) -> Value

/// Get system uptime.
pub fn uptime(&self) -> Duration

/// Install a program.
pub async fn install_program(&self, source: &str) -> Result<String>

/// List programs.
pub fn list_programs(&self) -> Vec<ProgramMeta>
```

---

## 3. Phase 2: AppState → Arc<Kernel>

```rust
// Before:
pub struct AppState {
    pub state_store: Arc<StateStore>,
    pub container_manager: Arc<ContainerManager>,
    pub event_bus: Arc<EventBus>,
    // ... 25 fields
}

// After:
pub struct AppState {
    pub base_url: String,
    pub channel: WebChannelHandle,
    pub kernel: Arc<Kernel>,
    pub config: Arc<RwLock<OxiosConfig>>,
    pub config_path: PathBuf,
    pub start_time: Instant,
    pub rate_limiter: RateLimiter,
}
```

Routes change:
```rust
// Before:
state.state_store.list_sessions().await
state.audit_trail.entries(from, to)
state.supervisor.kill(agent_id).await

// After:
state.kernel.list_sessions().await
state.kernel.query_audit(from, to)
state.kernel.kill(&agent_id).await
```

---

## 4. Phase 3: Kernel 필드 pub → pub(crate)

```rust
pub struct Kernel {
    pub(crate) orchestrator: Arc<Orchestrator>,
    pub(crate) gateway: Gateway,
    pub(crate) event_bus: EventBus,
    pub(crate) state_store: Arc<StateStore>,
    // ... 모든 필드 pub(crate)
}
```

Only `src/kernel.rs` and `src/main.rs` can access internals.

---

## 5. Implementation Order

1. Phase 1: Add System Call methods to Kernel (no breakage)
2. Phase 2: AppState → Arc<Kernel>, migrate routes (incremental)
3. Phase 3: pub → pub(crate) (final, breaks direct access)

Each phase compiles independently. Can commit after each.
