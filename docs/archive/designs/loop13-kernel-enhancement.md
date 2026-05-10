# Loop 13: Kernel Layer Enhancement

> **버전:** v0.2.0-alpha  
> **작성일:** 2026-05-06

---

## 1. 현재 Kernel 모듈 (28개)

| Module | 목적 | 상태 |
|--------|------|------|
| `supervisor` | Agent lifecycle (fork/exec/wait/kill) | ✅ |
| `event_bus` | Kernel event broadcast | ✅ |
| `state_store` | Markdown/JSON persistent state | ✅ |
| `scheduler` | Task scheduling, zombie reap | ✅ |
| `access_manager` | RBAC + audit log | ✅ |
| `orchestrator` | Ouroboros lifecycle coordinator | ✅ |
| `agent_runtime` | oxi-agent wrapper | ✅ |
| `agent_lifecycle` | Fork→register→run→cleanup | ✅ |
| `mcp` | MCP bridge multi-server | ✅ |
| `host_exec` | UDS relay + security | ✅ |
| `container` | Apple Container backend | ✅ |
| `circuit_breaker` | LLM outage protection | ✅ |
| `metrics` | Prometheus registry | ✅ |
| `program` | Installable programs | ✅ |
| `cron` | Time-based scheduling | ✅ |
| `git_layer` | Version control (gix) | ✅ |
| `persona` | Agent persona system | ✅ |
| `persona_manager` | Multi-persona management | ✅ |
| `skill` | Instruction templates | ✅ |
| `embedding` | TF-IDF embedding provider | ✅ |
| `memory` | Cross-session agent memory | ✅ |
| `auth` | API key authentication | ✅ |
| `config` | TOML configuration | ✅ |
| `container_manager` | Container lifecycle manager | ✅ |
| `host_tools` | Host tool validator | ✅ |
| `a2a` | Agent-to-agent messaging | ✅ |
| `backup` | State backup management | ✅ |
| `telemetry` | OTel trace spans | ✅ |
| `engine` | LLM engine provider | ✅ |

---

## 2. Kernel에 추가 필요한 것

### 2.1 Rate Limiter (Budget Enforcement)

**목적:** Agent별 token/호출 budget 관리. AIOS Kernel의 resource manager 영감.

```rust
pub struct BudgetLimit {
    pub agent_id: AgentId,
    pub token_budget: u64,
    pub calls_budget: u64,
    pub window_secs: u64,
}

pub struct BudgetManager {
    budgets: RwLock<HashMap<AgentId, BudgetLimit>>,
    usage: RwLock<HashMap<AgentId, Usage>>,
    enforcement: BudgetEnforcement,
}

impl BudgetManager {
    /// Reserve budget for an agent. Returns Ok if budget available.
    pub async fn reserve(&self, agent_id: &AgentId, tokens: u64) -> Result<BudgetReservation, BudgetExceeded>;
    
    /// Get remaining budget for an agent.
    pub fn remaining(&self, agent_id: &AgentId) -> BudgetInfo;
    
    /// Track usage.
    pub fn track(&self, agent_id: &AgentId, tokens: u64, calls: u64);
    
    /// Enforce budget in scheduler — don't schedule if exceeded.
    pub fn can_schedule(&self, agent_id: &AgentId) -> bool;
}
```

**Integrate:** `AgentScheduler::can_schedule()` → `BudgetManager::can_schedule()`

### 2.2 Audit Trail (Merkle Chain)

**목적:** 모든 kernel event를 cryptographic hash chain으로 기록. OpenFang의 Merkle hash-chain 감사 추적 영감.

```rust
pub struct AuditEntry {
    pub seq: u64,
    pub timestamp: DateTime<Utc>,
    pub actor: AgentId,
    pub action: AuditAction,
    pub resource: String,
    pub prev_hash: HashDigest,
    pub hash: HashDigest,
    pub signature: Option<Signature>,
}

pub struct AuditTrail {
    entries: RwLock<Vec<AuditEntry>>,
    hash_chain: blake3::Hasher,
}

impl AuditTrail {
    /// Append an audit entry.
    pub fn append(&self, entry: AuditEntry) -> Result<HashDigest>;
    
    /// Verify chain integrity.
    pub fn verify(&self) -> Result<bool>;
    
    /// Query entries by agent/time/action.
    pub fn query(&self, filter: AuditFilter) -> Vec<AuditEntry>;
    
    /// Export for external audit.
    pub fn export_json(&self, from_seq: u64) -> Result<String>;
}
```

**Integrate:** `KernelEvent::dispatch()` → `AuditTrail::append()`  
**Storage:** StateStore + GitLayer commit on flush

### 2.3 WASM Sandbox (Tool Isolation)

**목적:** Untrusted tool code를 WASM sandbox에서 실행. Extism/wasmtime 기반.

```rust
pub struct WasmSandbox {
    engine: wasmtime::Engine,
    linker: wasmtime::Linker,
    max_memory_bytes: u64,
    max_instructions: u64,
}

impl WasmSandbox {
    /// Load a WASM module from bytes or path.
    pub fn load_module(&self, wasm_bytes: &[u8], plugin_name: &str) -> Result<WasmModule>;
    
    /// Execute a tool in the sandbox.
    pub async fn execute_tool(
        &self,
        module: &WasmModule,
        func_name: &str,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, WasmError>;
    
    /// Kill a runaway module (instruction limit exceeded).
    pub fn terminate(&self, module: &WasmModule);
}

/// Program tool execution flow:
/// 1. Program::has_wasm_module() → WASM tool?
/// 2. WasmSandbox::execute_tool() → sandboxed execution
/// 3. Fallback to normal exec if sandbox unavailable
```

**Integrate:** `ProgramTool::execute()` → check if program has WASM → route to `WasmSandbox`  
**Feature gate:** `wasm-sandbox` feature (default off)

### 2.4 Resource Monitor

**목적:** CPU/memory/토큰 사용량 실시간 모니터링. Scheduler decision 지원.

```rust
pub struct ResourceSnapshot {
    pub cpu_percent: f32,
    pub memory_mb: u64,
    pub active_agents: usize,
    pub pending_tasks: usize,
    pub token_usage: u64,
}

pub struct ResourceMonitor {
    interval_secs: u64,
    history: RwLock<VecDeque<ResourceSnapshot>>,
}

impl ResourceMonitor {
    /// Get current snapshot.
    pub fn snapshot(&self) -> ResourceSnapshot;
    
    /// Get historical data for analysis.
    pub fn history(&self, last_n: usize) -> Vec<ResourceSnapshot>;
    
    /// Check if system is under pressure (throttle scheduling).
    pub fn is_overloaded(&self) -> bool;
}
```

**Integrate:** `AgentScheduler::pick_next()` → check `ResourceMonitor::is_overloaded()`  
**Expose:** `GET /api/system/resources` endpoint

---

## 3. 우선순위

| Priority | Feature | Impact | Effort |
|----------|----------|--------|--------|
| P0 | Budget Manager | Security/Isolation | Medium |
| P0 | Audit Trail | Compliance/Audit | Medium |
| P1 | WASM Sandbox | Security isolation | High |
| P1 | Resource Monitor | Reliability | Low |

---

## 4. 구현 메모

### Budget vs Rate Limiter 차이

- `RateLimiter` (현재): HTTP-level request throttling. WebServer에 있음.
- `BudgetManager` (신규): Agent-level token/call budget enforcement. Kernel level.

### Audit vs Access Manager 차이

- `AccessManager` (현재): RBAC permission checks + action log.
- `AuditTrail` (신규): Cryptographic Merkle chain of all events. tamper-evidence.

### WASM vs Container 차이

- `Container`: Heavy isolation, full process. Apple Container.
- `WasmSandbox`: Lightweight isolation, inline execution. For untrusted tool plugins.