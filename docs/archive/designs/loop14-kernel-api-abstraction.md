# Loop 14: Kernel API Abstraction

> **원칙:** Kernel은 내부 구조를 숨기고, 안정적인 API만 노출한다.
> **목표:** Program(동적 능력 패키지) + Daemon(백그라운드 자동화) 모두 같은 API 사용.

---

## 1. 현재 문제

```rust
pub struct Kernel {
    pub orchestrator: Arc<Orchestrator>,      // ← 노출됨
    pub state_store: Arc<StateStore>,          // ← 노출됨
    pub git_layer: Arc<GitLayer>,              // ← 노출됨
    pub audit_trail: Arc<AuditTrail>,          // ← 노출됨
    pub memory_manager: Arc<MemoryManager>,    // ← 노출됨
    pub supervisor: Arc<dyn Supervisor>,       // ← 노출됨
    pub scheduler: Arc<AgentScheduler>,        // ← 노출됨
    pub cron_scheduler: Arc<CronScheduler>,    // ← 노출됨
    pub budget_manager: Arc<BudgetManager>,    // ← 노출됨
    pub resource_monitor: Arc<ResourceMonitor>,// ← 노출됨
    // ... 20개 전부 pub
}
```

**20개 필드가 다 노출.** Program이 kernel 내부를 직접 건드릴 수 있음.
→ 내부 구조 변경 시 모든 호출 지점이 깨짐
→ 보안: Program이 supervisor.kill_all() 호출 가능
→ 캡슐화 위반

---

## 2. 해결: Kernel 내부는 pub(crate), API만 public

```rust
pub struct Kernel {
    // 내부: pub(crate) — 같은 crate(main binary)에서만 접근
    pub(crate) orchestrator: Arc<Orchestrator>,
    pub(crate) state_store: Arc<StateStore>,
    pub(crate) git_layer: Arc<GitLayer>,
    pub(crate) audit_trail: Arc<AuditTrail>,
    pub(crate) memory_manager: Arc<MemoryManager>,
    pub(crate) supervisor: Arc<dyn Supervisor>,
    pub(crate) scheduler: Arc<AgentScheduler>,
    pub(crate) cron_scheduler: Arc<CronScheduler>,
    pub(crate) budget_manager: Arc<BudgetManager>,
    pub(crate) resource_monitor: Arc<ResourceMonitor>,
    pub(crate) container_manager: Arc<ContainerManager>,
    pub(crate) event_bus: EventBus,
    pub(crate) config: OxiosConfig,
    pub(crate) program_manager: Arc<ProgramManager>,
    // ...

    // 노출: 없음. 전부 메서드로.
}
```

---

## 3. Kernel API (Program이 보는 것)

```rust
impl Kernel {
    // ── State ─────────────────────────────────────
    /// 데이터 저장 + git commit.
    pub async fn save(&self, category: &str, name: &str, data: &Value) -> Result<()>;
    
    /// 데이터 로드.
    pub async fn load(&self, category: &str, name: &str) -> Result<Option<Value>>;
    
    /// 데이터 삭제 + git commit.
    pub async fn delete(&self, category: &str, name: &str) -> Result<bool>;
    
    // ── Agent ─────────────────────────────────────
    /// 에이전트 스폰.
    pub async fn spawn(&self, task: &str, persona: Option<&str>) -> Result<AgentId>;
    
    /// 에이전트 완료 대기.
    pub async fn wait(&self, agent_id: &AgentId) -> Result<String>;
    
    /// 에이전트 종료.
    pub async fn kill(&self, agent_id: &AgentId) -> Result<()>;
    
    // ── Memory ────────────────────────────────────
    /// 기억 저장.
    pub async fn remember(&self, category: &str, content: &str, tags: Vec<&str>) -> Result<String>;
    
    /// 기억 검색.
    pub async fn recall(&self, query: &str, limit: usize) -> Result<Vec<Value>>;
    
    // ── Git ───────────────────────────────────────
    /// 현재 변경사항 커밋.
    pub fn commit(&self, message: &str) -> Result<()>;
    
    /// 태그 생성.
    pub fn tag(&self, name: &str, message: &str) -> Result<()>;
    
    /// 파일 복원.
    pub fn restore(&self, path: &str, hash: &str) -> Result<()>;
    
    /// 커밋 로그.
    pub fn log(&self, max: usize) -> Result<Vec<Value>>;
    
    // ── Scheduling ────────────────────────────────
    /// 크론 작업 등록.
    pub async fn schedule(&self, cron_expr: &str, task: &str) -> Result<String>;
    
    /// 크론 작업 취소.
    pub async fn unschedule(&self, job_id: &str) -> Result<bool>;
    
    // ── Audit ─────────────────────────────────────
    /// 감사 기록.
    pub fn audit(&self, action: &str, resource: &str) -> Result<()>;
    
    /// 감사 체인 검증.
    pub fn verify_audit(&self) -> Result<bool>;
    
    // ── Resources ─────────────────────────────────
    /// 시스템 리소스 조회.
    pub fn resources(&self) -> Value;
    
    /// 에이전트 예산 확인.
    pub fn check_budget(&self, agent_id: &AgentId) -> bool;
    
    // ── Container ─────────────────────────────────
    /// 컨테이너에서 명령 실행.
    pub async fn exec(&self, container: &str, command: &str) -> Result<String>;
    
    // ── Events ────────────────────────────────────
    /// 이벤트 구독.
    pub fn subscribe(&self) -> broadcast::Receiver<KernelEvent>;
    
    // ── Config ────────────────────────────────────
    /// 설정 조회.
    pub fn get_config(&self) -> &OxiosConfig;
}
```

---

## 4. Program이 Kernel API를 사용하는 방식

### 방식 A: SKILL.md의 instruction에 kernel API 호출 지시 (현재 방식 유지)

program.toml + SKILL.md는 agent에게 "이 도구를 이렇게 써라"라고 지시.
Agent가 kernel API를 직접 호출하는 게 아니라, tool을 통해 간접적으로 사용.

### 방식 B: Daemon이 Kernel API를 직접 호출 (새로 추가)

백그라운드 자동화는 agent 없이 kernel API를 직접 호출:

```rust
// Git-sync daemon — kernel.api 직접 사용
async fn git_sync_daemon(kernel: &Kernel) {
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
        kernel.commit("auto-sync: hourly").ok();
        kernel.tag(&format!("auto-{}", Utc::now().format("%Y%m%d-%H%M")), "auto").ok();
        kernel.audit("git-sync", "hourly auto-commit").ok();
    }
}
```

```rust
// Audit verify daemon
async fn audit_verify_daemon(kernel: &Kernel) {
    loop {
        tokio::time::sleep(Duration::from_secs(300)).await;
        if !kernel.verify_audit().unwrap_or(false) {
            kernel.audit("alert", "audit chain integrity check FAILED").ok();
        }
    }
}
```

### 방식 C: Program definition에 kernel API 권한 선언 (미래)

```toml
# program.toml
[program]
name = "git-sync"
version = "1.0.0"

[kernel_api]
allow = ["commit", "tag", "audit", "schedule"]   # 허용된 API만
deny = ["spawn", "kill", "exec"]                  # 금지된 API
```

→ 보안: Program마다 사용 가능한 kernel API를 제한

---

## 5. 구현 계획

### Phase 1: Kernel 필드 pub → pub(crate)
- 20개 필드 전부 pub(crate)로 변경
- src/main.rs, src/kernel.rs 내부에서만 직접 접근
- WebServer는 Kernel API 메서드로만 접근

### Phase 2: Kernel API 메서드 구현
- save/load/delete/spawn/wait/kill/remember/recall
- commit/tag/restore/log/schedule/unschedule
- audit/verify_audit/resources/check_budget/exec/subscribe

### Phase 3: WebServer를 Kernel API로 마이그레이션
- AppState가 Kernel 전체 대신 Arc<Kernel>만 보유
- Route handler가 kernel.save(), kernel.log() 등만 호출

### Phase 4: Built-in daemon tasks
- git-sync, audit-verify, resource-watch를 tokio::spawn으로

### Phase 5: Program kernel_api 권한 (미래)
- program.toml에 [kernel_api] allow/deny
- AccessManager가 런타임에 권한 체크
