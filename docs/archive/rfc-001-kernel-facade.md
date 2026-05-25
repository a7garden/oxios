# RFC-001: KernelHandle Facade 분리 설계

> **날짜**: 2026-05-10
> **상태**: 초안 v2 (리뷰 피드백 반영)
> **범위**: `oxios-kernel/src/kernel_handle.rs`

---

## 1. 배경

### 현재 문제

`KernelHandle`은 19개 서브시스템을 단일 struct에 평면적으로 나열하고,
101개 공개 메서드를 하나의 `impl` 블록에 정의한다.

**문제점:**
- God Object: 101개 메서드가 하나의 `impl`에 집중
- 발견성: "내가 필요한 메서드가 어디 있는지" 파악 어려움
- 테스트: 특정 도메인만 모킹하기 어려움
- 응집도: 무관한 메서드가 같은 스코프에 존재

**실제 사용 패턴 (Web routes 103곳 + CLI 7곳 + Guardian 8곳):**
```
고빈도 (4회+):  workspace_path, load_markdown, list_category
중빈도 (2회):   subscribe, publish, list_agents, query_audit, ...
저빈도 (1회):   60+ 메서드
미사용:         auto_commit_enabled, verify(), persona_store(), mcp_servers(), ...
```

---

## 2. 설계 원칙

1. **같은 프로세스, 같은 바이너리** — Micro-kernel IPC가 아닌 Facade 패턴
2. **Rust 소유권 활용** — Arc<> 공유는 그대로, 인터페이스만 재구성
3. **호환성** — Web routes의 호출부 변경은 최소화
4. **7개 도메인** — 각 Facade는 하나의 명확한 책임 영역 (8–16개 메서드)
5. **점진적 마이그레이션** — 컴파일 에러로 누락 파악

---

## 3. 새로운 구조

### 3.1 KernelHandle (재설계)

```rust
// kernel_handle/mod.rs

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

/// Oxios 커널 시스템 콜 API — 7개 도메인 Facade로 구성.
///
/// 각 Facade는 특정 도메인의 시스템 콜을 그룹화한다.
/// 모든 Facade는 동일한 서브시스템의 Arc<>를 공유한다 (같은 프로세스).
pub struct KernelHandle {
    /// 상태 관리: 저장/로드/세션/Git 커밋
    pub state: StateApi,
    /// 에이전트 관리: 라이프사이클/예산/메모리
    pub agents: AgentApi,
    /// 보안: 인증/감사/RBAC/승인
    pub security: SecurityApi,
    /// 페르소나: 멀티 페르소나 관리
    pub persona: PersonaApi,
    /// 확장: 프로그램/스킬/호스트 도구
    pub extensions: ExtensionApi,
    /// MCP: 외부 도구 서버 브릿지
    pub mcp: McpApi,
    /// 인프라: 스케줄러/크론/리소스/이벤트/Git 조회/시스템
    pub infra: InfraApi,
}
```

### 3.2 파일 구조

```
crates/oxios-kernel/src/
├── kernel_handle/
│   ├── mod.rs              KernelHandle 정의 + 생성자
│   ├── state_api.rs        StateApi    (15개 메서드)
│   ├── agent_api.rs        AgentApi    (11개 메서드)
│   ├── security_api.rs     SecurityApi (15개 메서드)
│   ├── persona_api.rs      PersonaApi  (9개 메서드)
│   ├── extension_api.rs    ExtensionApi (12개 메서드)
│   ├── mcp_api.rs          McpApi      (8개 메서드)
│   └── infra_api.rs        InfraApi    (15개 메서드)
├── kernel_handle.rs        → 삭제 (디렉토리로 대체)
└── lib.rs                  pub mod kernel_handle; (변경 없음)
```

---

## 4. 각 Facade 상세 설계

### 4.1 StateApi — "저장하고 불러오기"

**책임**: 데이터 지속성, 세션 관리

```rust
pub struct StateApi {
    pub(crate) state_store: Arc<StateStore>,
}
```

| # | 메서드 | 시그니처 | 설명 |
|---|--------|----------|------|
| 1 | `save` | `async fn save<T: Serialize>(category, name, data) → Result<()>` | JSON 저장 |
| 2 | `save_markdown` | `async fn save_markdown(category, name, content) → Result<()>` | Markdown 저장 |
| 3 | `load` | `async fn load<T: DeserializeOwned>(category, name) → Result<Option<T>>` | JSON 로드 |
| 4 | `load_markdown` | `async fn load_markdown(category, name) → Result<Option<String>>` | Markdown 로드 |
| 5 | `delete` | `async fn delete(category, name) → Result<bool>` | 파일 삭제 |
| 6 | `list_category` | `async fn list_category(category) → Result<Vec<String>>` | 카테고리 내 파일 목록 |
| 7 | `commit_all` | `fn commit_all(git: &GitLayer, message) → Result<Option<CommitInfo>>` | 전체 Git 커밋 |
| 8 | `save_and_commit` | `async fn save_and_commit<T>(git: &GitLayer, category, name, data) → Result<()>` | 저장 + 커밋 |
| 9 | `save_md_and_commit` | `async fn save_md_and_commit(git: &GitLayer, category, name, content) → Result<()>` | Markdown 저장 + 커밋 |
| 10 | `delete_and_commit` | `async fn delete_and_commit(git: &GitLayer, category, name) → Result<bool>` | 삭제 + 커밋 |
| 11 | `save_session` | `async fn save_session(session) → Result<()>` | 세션 저장 |
| 12 | `load_session` | `async fn load_session(id) → Result<Option<Session>>` | 세션 로드 |
| 13 | `list_sessions` | `async fn list_sessions() → Result<Vec<SessionSummary>>` | 세션 목록 |
| 14 | `delete_session` | `async fn delete_session(id) → Result<bool>` | 세션 삭제 |
| 15 | `workspace_path` | `fn workspace_path() → &Path` | 워크스페이스 경로 |

**v1에서의 변경:**
- `base_path` 제거 → `workspace_path`와 동일, 하나로 통일
- `auto_commit_enabled` 제거 → 1곳도 사용 안 함. 필요시 `infra.config().git.auto_commit`로 확인
- Git 커밋 메서드(`save_and_commit` 등)는 `git_layer`를 명시적 파라미터로 받음 → StateApi는 `git_layer`를 소유하지 않고 InfraApi에서 전달

> **설계 결정**: StateApi는 `git_layer`를 소유하지 않습니다.
> 커밋이 필요한 호출부에서 `handle.state.save_and_commit(&handle.infra.git(), "cat", "name", &data)` 처럼
> InfraApi의 Git 레퍼런스를 전달합니다.
> 또는 KernelHandle 수준에서 `handle.save_and_commit()` 편의 메서드를 제공합니다.
> (자세한 내용은 Section 6 참조)

---

### 4.2 AgentApi — "에이전트 관리"

**책임**: 에이전트 라이프사이클, 예산, 에이전트별 메모리

```rust
pub struct AgentApi {
    pub(crate) supervisor: Arc<dyn Supervisor>,
    pub(crate) budget_manager: Arc<BudgetManager>,
    pub(crate) memory_manager: Arc<MemoryManager>,
}
```

| # | 메서드 | 시그니처 | 설명 |
|---|--------|----------|------|
| 1 | `list` | `async fn list() → Result<Vec<AgentInfo>>` | 에이전트 목록 |
| 2 | `kill` | `async fn kill(agent_id) → Result<()>` | 에이전트 종료 |
| 3 | `check_budget` | `fn check_budget(agent_id) → BudgetInfo` | 예산 확인 |
| 4 | `set_budget` | `fn set_budget(limit: BudgetLimit)` | 예산 설정 |
| 5 | `remove_budget` | `fn remove_budget(agent_id)` | 예산 제거 |
| 6 | `reserve_budget` | `fn reserve_budget(agent_id, tokens) → Result<(), BudgetExceeded>` | 토큰 예약 |
| 7 | `reset_budget` | `fn reset_budget(agent_id)` | 예산 윈도우 리셋 |
| 8 | `memory_stats` | `async fn memory_stats() → (usize, usize)` | 메모리 통계 |
| 9 | `remember` | `async fn remember(entry: MemoryEntry) → Result<String>` | 메모리 저장 |
| 10 | `search_memory` | `async fn search_memory(query, type?, limit) → Result<Vec<MemoryEntry>>` | 메모리 검색 |
| 11 | `recall` | `async fn recall(query) → Result<Vec<MemoryEntry>>` | 메모리 리콜 (단순 래퍼) |

**v1에서의 변경:**
- `memory_stats()` (sync) 제거 → `memory_stats_async()`를 `memory_stats()`로 통일
- `memory_remember` → `remember`, `memory_search` → `search_memory` (Facade 스코프 안에서 접두사 불필요)
- `recall` 추가 → `search_memory(query, None, 5)`의 편의 래퍼

---

### 4.3 SecurityApi — "보안과 감사"

**책임**: 인증, 감사 추적, 접근 제어, Human-in-the-Loop 승인

```rust
pub struct SecurityApi {
    pub(crate) auth_manager: Arc<parking_lot::Mutex<AuthManager>>,
    pub(crate) audit_trail: Arc<AuditTrail>,
    pub(crate) access_manager: Arc<parking_lot::Mutex<AccessManager>>,
}
```

| # | 메서드 | 시그니처 | 설명 |
|---|--------|----------|------|
| **감사 추적** | | | |
| 1 | `audit` | `fn audit(actor, action, resource) → String` | 감사 기록 |
| 2 | `verify_chain` | `fn verify_chain() → Result<bool>` | 체인 무결성 검증 |
| 3 | `query_audit` | `fn query_audit(from, to) → Vec<AuditEntry>` | 범위 조회 |
| 4 | `query_audit_by_agent` | `fn query_audit_by_agent(agent_id) → Vec<AuditEntry>` | 에이전트별 조회 |
| 5 | `audit_count` | `fn audit_count() → usize` | 총 항목 수 |
| 6 | `flush` | `fn flush() → Result<()>` | 감사 플러시 |
| **인증** | | | |
| 7 | `validate_token` | `fn validate_token(token) → bool` | Bearer 토큰 검증 |
| **접근 제어** | | | |
| 8 | `get_permissions` | `fn get_permissions(agent) → Option<AgentPermissions>` | 권한 조회 |
| 9 | `update_permissions` | `fn update_permissions(agent, update) → Result<()>` | 권한 수정 |
| 10 | `get_audit_log` | `fn get_audit_log() → Vec<AuditEntry>` | 접근 로그 |
| 11 | `log_action` | `fn log_action(agent, action, resource)` | 접근 로깅 |
| **승인 (HitL)** | | | |
| 12 | `list_approvals` | `fn list_approvals() → Vec<(PendingApproval, ApprovalStatus)>` | 승인 대기 목록 |
| 13 | `approve` | `fn approve(id) → bool` | 승인 |
| 14 | `reject` | `fn reject(id) → bool` | 거부 |
| 15 | `ensure_permissions` | `fn ensure_permissions(agent) → AgentPermissions` | 권한 보장 (get or create) |

**v1에서의 변경:**
- `verify_audit` → `verify_chain` (감사 "체인" 검증이라는 의미 명확화)
- `approve_request` → `approve`, `reject_request` → `reject`
- `audit_log_action` → `log_action`
- `get_or_create_permissions` → `ensure_permissions` (의미 명확화)

---

### 4.4 PersonaApi — "페르소나 관리"

**책임**: 멀티 페르소나 정의, 활성 페르소나 전환

```rust
pub struct PersonaApi {
    pub(crate) persona_manager: Arc<PersonaManager>,
}
```

| # | 메서드 | 시그니처 | 설명 |
|---|--------|----------|------|
| 1 | `list` | `fn list() → Vec<Persona>` | 전체 페르소나 목록 |
| 2 | `get` | `fn get(id) → Option<Persona>` | 특정 페르소나 |
| 3 | `create` | `fn create(persona)` | 생성 |
| 4 | `update` | `fn update(id, persona) → Result<()>` | 수정 |
| 5 | `delete` | `fn delete(id) → Result<()>` | 삭제 |
| 6 | `active` | `fn active() → Option<Persona>` | 활성 페르소나 |
| 7 | `set_active` | `fn set_active(id) → Result<()>` | 활성화 설정 |
| 8 | `count` | `fn count() → usize` | 수 |
| 9 | `list_enabled` | `fn list_enabled() → Vec<Persona>` | 활성 목록 |

**v1에서의 변경:**
- ExtensionApi에서 독립 Facade로 분리 (9개 메서드, 명확한 단일 책임)
- `list_personas` → `list`, `get_persona` → `get` 등 (Facade 스코프 안에서 접두사 불필요)
- `persona_store()` 내부 노출 제거 → 모든 접근은 API 메서드를 통해서

---

### 4.5 ExtensionApi — "확장성"

**책임**: 프로그램, 스킬, 호스트 도구

```rust
pub struct ExtensionApi {
    pub(crate) program_manager: Arc<ProgramManager>,
    pub(crate) skill_store: Arc<SkillStore>,
    pub(crate) host_tool_validator: Arc<HostToolValidator>,
}
```

| # | 메서드 | 시그니처 | 설명 |
|---|--------|----------|------|
| **프로그램** | | | |
| 1 | `list_programs` | `async fn list_programs() → Vec<ProgramMeta>` | 프로그램 목록 |
| 2 | `get_program` | `async fn get_program(name) → Option<Program>` | 프로그램 상세 |
| 3 | `install_program` | `async fn install_program(source) → Result<Program>` | 설치 |
| 4 | `uninstall_program` | `async fn uninstall_program(name) → Result<()>` | 제거 |
| 5 | `enable_program` | `async fn enable_program(name) → Result<()>` | 활성화 |
| 6 | `disable_program` | `async fn disable_program(name) → Result<()>` | 비활성화 |
| 7 | `check_host_requirements` | `async fn check_host_requirements(name) → Result<HostRequirementsCheck>` | 호스트 요구사항 |
| **스킬** | | | |
| 8 | `list_skills` | `async fn list_skills() → Result<Vec<SkillMeta>>` | 스킬 목록 |
| 9 | `load_skill` | `async fn load_skill(name) → Result<Option<Skill>>` | 스킬 로드 |
| 10 | `create_skill` | `async fn create_skill(name, desc, content) → Result<()>` | 스킬 생성 |
| 11 | `delete_skill` | `async fn delete_skill(name) → Result<()>` | 스킬 삭제 |
| **호스트 도구** | | | |
| 12 | `check_host_tools` | `fn check_host_tools() → HostToolStatus` | 전체 체크 |

**v1에서의 변경:**
- Persona(9개)와 MCP(8개)를 별도 Facade로 분리 → 12개 메서드로 축소
- Program + Skill + HostTools = "설치 가능한 확장"이라는 응집된 도메인

---

### 4.6 McpApi — "외부 도구 서버"

**책임**: MCP 서버 등록/관리, 도구 호출

```rust
pub struct McpApi {
    pub(crate) mcp_bridge: Arc<McpBridge>,
}
```

| # | 메서드 | 시그니처 | 설명 |
|---|--------|----------|------|
| 1 | `list_servers` | `fn list_servers() → Vec<String>` | 서버 목록 |
| 2 | `get_server` | `fn get_server(name) → Option<McpServer>` | 서버 정보 |
| 3 | `register_server` | `fn register_server(server)` | 서버 등록 |
| 4 | `init_server` | `async fn init_server(name) → Result<()>` | 서버 초기화 |
| 5 | `client_status` | `async fn client_status(name) → Option<bool>` | 클라이언트 상태 |
| 6 | `list_tools` | `async fn list_tools() → Result<Vec<ToolDef>>` | 전체 도구 목록 |
| 7 | `cached_tools` | `async fn cached_tools(server) → Option<Vec<ToolDef>>` | 캐시된 도구 |
| 8 | `call_tool` | `async fn call_tool(server, tool, args) → Result<McpToolCallResult>` | 도구 호출 |

**v1에서의 변경:**
- ExtensionApi에서 독립 Facade로 분리
- `mcp_list_servers` → `list_servers`, `mcp_initialize_server` → `init_server` 등 (접두사 `mcp_` 제거)
- `mcp_servers()` (중복) 제거 → `list_servers()`로 통일

---

### 4.7 InfraApi — "시스템 인프라"

**책임**: Git 버전 관리, 스케줄링, 크론, 리소스 모니터링, 이벤트 버스, 시스템 정보

```rust
pub struct InfraApi {
    pub(crate) git_layer: Arc<GitLayer>,
    pub(crate) scheduler: Arc<AgentScheduler>,
    pub(crate) cron_scheduler: Arc<CronScheduler>,
    pub(crate) resource_monitor: Arc<ResourceMonitor>,
    pub(crate) event_bus: EventBus,
    pub(crate) config: OxiosConfig,
    pub(crate) start_time: Instant,
}
```

| # | 메서드 | 시그니처 | 설명 |
|---|--------|----------|------|
| **Git** | | | |
| 1 | `git_log` | `fn git_log(max) → Result<Vec<LogEntry>>` | 커밋 로그 |
| 2 | `git_tag` | `fn git_tag(name, message) → Result<()>` | 태그 |
| 3 | `git_restore` | `fn git_restore(path, hash) → Result<()>` | 파일 복원 |
| 4 | `git_verify` | `fn git_verify() → Result<bool>` | 무결성 검증 |
| 5 | `git_tags` | `fn git_tags() → Result<Vec<String>>` | 태그 목록 |
| 6 | `git` | `fn git() → &GitLayer` | GitLayer 직접 접근 |
| **스케줄러** | | | |
| 7 | `scheduler_stats` | `fn scheduler_stats() → SchedulerStats` | 스케줄러 통계 |
| 8 | `queued_tasks` | `fn queued_tasks() → Vec<ScheduledTask>` | 대기 태스크 |
| 9 | `running_tasks` | `fn running_tasks() → Vec<ScheduledTask>` | 실행 태스크 |
| **크론** | | | |
| 10 | `add_cron` | `async fn add_cron(job: CronJob) → Result<Uuid>` | 크론 등록 |
| 11 | `get_cron` | `fn get_cron(id) → Option<CronJob>` | 크론 조회 |
| 12 | `update_cron` | `async fn update_cron(id, update) → Result<()>` | 크론 수정 |
| 13 | `remove_cron` | `async fn remove_cron(id) → Result<()>` | 크론 제거 |
| 14 | `trigger_cron` | `fn trigger_cron(id) → Result<CronJob>` | 수동 트리거 |
| 15 | `complete_cron` | `async fn complete_cron(id, success, summary)` | 완료 표시 |
| **리소스** | | | |
| 16 | `resource_snapshot` | `fn resource_snapshot() → ResourceSnapshot` | 현재 리소스 |
| 17 | `resource_history` | `fn resource_history(last_n) → Vec<ResourceSnapshot>` | 이력 |
| 18 | `is_overloaded` | `fn is_overloaded() → bool` | 과부하 여부 |
| **이벤트** | | | |
| 19 | `subscribe` | `fn subscribe() → Receiver<KernelEvent>` | 이벤트 구독 |
| 20 | `publish` | `fn publish(event) → Result<()>` | 이벤트 발행 |
| **시스템** | | | |
| 21 | `config` | `fn config() → &OxiosConfig` | 설정 참조 |
| 22 | `uptime` | `fn uptime() → Duration` | 가동 시간 |

**v1에서의 변경:**
- `git()` 접근자 추가 → StateApi의 `save_and_commit` 등에서 GitLayer 참조 필요
- 크론 API 중복 제거: `schedule()`/`unschedule()`/`list_schedules()` 제거 → `add_cron()`/`remove_cron()`/목록은 `get_cron()`으로 통일
- `scheduler_rate_remaining` 제거 → `scheduler_stats()`에 포함. 실제 1곳(infra.rs:43)만 사용, stats에서 가져오면 됨
- `overload_threshold` 제거 → `is_overloaded()`로 충분
- `start_time()` 메서드 제거 → `uptime()`으로 충분

---

## 5. KernelHandle 편의 메서드

Guardian daemon이나 자주 쓰는 패턴을 위해 KernelHandle 수준의 편의 메서드를 제공한다.

```rust
impl KernelHandle {
    /// 저장 + Git 커밋 (StateApi + InfraApi 연동)
    pub async fn save_and_commit<T: Serialize>(
        &self,
        category: &str,
        name: &str,
        data: &T,
    ) -> Result<()> {
        self.state.save(category, name, data).await?;
        self.state.commit_all(&self.infra.git(), format!("save {}/{}", category, name))?;
        Ok(())
    }

    /// Markdown 저장 + Git 커밋
    pub async fn save_md_and_commit(
        &self,
        category: &str,
        name: &str,
        content: &str,
    ) -> Result<()> {
        self.state.save_markdown(category, name, content).await?;
        self.state.commit_all(&self.infra.git(), format!("save {}/{}", category, name))?;
        Ok(())
    }

    /// 삭제 + Git 커밋
    pub async fn delete_and_commit(
        &self,
        category: &str,
        name: &str,
    ) -> Result<bool> {
        let deleted = self.state.delete(category, name).await?;
        if deleted {
            self.state.commit_all(&self.infra.git(), format!("delete {}/{}", category, name))?;
        }
        Ok(deleted)
    }

    /// 감사 플러시 + Git 커밋
    pub fn flush_audit(&self) -> Result<()> {
        let _ = self.infra.git().commit_file("audit", "audit trail flush");
        Ok(())
    }
}
```

> **설계 결정**: `save_and_commit` 등은 KernelHandle 수준 편의 메서드로 제공.
> StateApi는 순수하게 데이터 저장에만 집중하고, Git 커밋과의 연동은
> KernelHandle에서 조율한다. 이것이 Facade 간 결합을 명시적으로 만든다.

---

## 6. KernelHandle 생성자

```rust
// kernel_handle/mod.rs

impl KernelHandle {
    /// 7개 Facade를 직접 받아 생성.
    ///
    /// `kernel.rs`의 `Kernel::handle()`에서 각 Facade를 개별적으로 조립 후 전달.
    /// 테스트에서는 필요한 Facade만 생성 가능.
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
}
```

`kernel.rs`에서의 조립:

```rust
impl Kernel {
    pub fn handle(&self) -> Arc<KernelHandle> {
        Arc::new(KernelHandle::new(
            StateApi {
                state_store: self.state_store.clone(),
            },
            AgentApi {
                supervisor: self.supervisor.clone(),
                budget_manager: self.budget_manager.clone(),
                memory_manager: self.memory_manager.clone(),
            },
            SecurityApi {
                auth_manager: self.auth_manager.clone(),
                audit_trail: self.audit_trail.clone(),
                access_manager: self.access_manager.clone(),
            },
            PersonaApi {
                persona_manager: Arc::new(self.persona_manager.clone()),
            },
            ExtensionApi {
                program_manager: self.program_manager.clone(),
                skill_store: Arc::new(self.skill_store.clone()),
                host_tool_validator: Arc::new(self.host_tool_validator.clone()),
            },
            McpApi {
                mcp_bridge: self.mcp_bridge.clone(),
            },
            InfraApi {
                git_layer: self.git_layer.clone(),
                scheduler: self.scheduler.clone(),
                cron_scheduler: self.cron_scheduler.clone(),
                resource_monitor: self.resource_monitor.clone(),
                event_bus: self.event_bus.clone(),
                config: self.config.clone(),
                start_time: self.start_time,
            },
        ))
    }
}
```

**v1에서의 변경:**
- `new()`가 19개 파라미터 → 7개 Facade 파라미터
- 각 Facade를 독립적으로 조립 → 테스트에서는 필요한 Facade만 생성 가능

---

## 7. 공유 Arc 맵 (수정됨)

```
서브시스템                State  Agent  Security  Persona  Extension  Mcp  Infra
─────────────────────────────────────────────────────────────────────────────────
state_store (Arc)           ●
event_bus (EventBus)                                                       ●
supervisor (Arc<dyn>)               ●
scheduler (Arc)                                                            ●
memory_manager (Arc)                ●
git_layer (Arc)                                                            ●
audit_trail (Arc)                            ●
budget_manager (Arc)                ●
resource_monitor (Arc)                                                     ●
cron_scheduler (Arc)                                                       ●
program_manager (Arc)                                          ●
skill_store (Arc)                                              ●
persona_manager (Arc)                               ●
mcp_bridge (Arc)                                                    ●
auth_manager (Arc<Mutex>)                    ●
access_manager (Arc<Mutex>)                  ●
host_tool_validator (Arc)                                      ●
config (OxiosConfig)                                                       ●
start_time (Instant)                                                       ●
─────────────────────────────────────────────────────────────────────────────────
서브시스템 수:              1      3      3         1        3         1     6
```

**v1에서의 수정:**
- `auth_manager` → SecurityApi (v1에서 ExtensionApi로 잘못 분류되었던 것 수정)
- `access_manager` → SecurityApi (동일)
- `persona_manager` → PersonaApi (ExtensionApi에서 분리)
- `mcp_bridge` → McpApi (ExtensionApi에서 분리)
- `git_layer` → InfraApi만 소유 (v1에서 State+Infra 양쪽에 있던 것을 Infra로 통일)

**공유 서브시스템 없음** — 모든 Arc가 정확히 하나의 Facade에만 속한다.
이것이 가장 큰 개선: v1에서 유일했던 교차 소유권(git_layer)이 제거됨.

---

## 8. 제거/병합된 메서드 목록

기존 101개에서 85개로 축소. 16개 제거/병합:

| # | 제거된 메서드 | 사유 | 대안 |
|---|--------------|------|------|
| 1 | `memory_stats()` (sync) | 동일 기능의 async 버전 존재 | `agents.memory_stats()` (async로 통일) |
| 2 | `auto_commit_enabled()` | 사용처 0곳 | `infra.config().git.auto_commit` |
| 3 | `verify()` | 테스트 전용, 프로덕션 미사용 | `security.verify_chain()` |
| 4 | `state_store_base_path()` | `workspace_path()`와 동일 | `state.workspace_path()` |
| 5 | `persona_store()` | 내부 타입 노출 | `persona.*` API 메서드 |
| 6 | `mcp_servers()` | `mcp_list_servers()`와 중복 | `mcp.list_servers()` |
| 7 | `scheduler_rate_remaining()` | 1곳 사용, `scheduler_stats()`에 포함 | `infra.scheduler_stats().rate_remaining` |
| 8 | `schedule()` | `add_cron_job()`과 중복 | `infra.add_cron(CronJob::new(...))` |
| 9 | `unschedule()` | `remove_cron`으로 통일 | `infra.remove_cron(id)` |
| 10 | `list_schedules()` | 크론 목록은 `get_cron` + 순회로 대체 | 크론 라우트에서 `list_schedules()` 추가 시 별도 구현 |
| 11 | `get_or_create_permissions()` | 혼합 read/write 의미 | `security.ensure_permissions()`로 명확화 |
| 12 | `overload_threshold()` | `is_overloaded()`로 충분 | `infra.is_overloaded()` |
| 13 | `start_time()` 메서드 | `uptime()`으로 충분 | `infra.uptime()` |
| 14 | `flush_audit()` (SecurityApi) | Git 커밋과 연동 필요 | KernelHandle 편의 메서드로 이동 |
| 15 | `save_and_commit()` (StateApi) | Git 커밋과 연동 필요 | KernelHandle 편의 메서드로 이동 |
| 16 | `save_markdown_and_commit()` | 동일 | KernelHandle 편의 메서드로 이동 |

---

## 9. 메서드 분류별 통계

```
┌────────────────┬──────────┬──────────────────────────────┐
│ Facade         │ 메서드 수 │ 서브시스템                    │
├────────────────┼──────────┼──────────────────────────────┤
│ StateApi       │    15    │ StateStore                    │
│ AgentApi       │    11    │ Supervisor, Budget, Memory    │
│ SecurityApi    │    15    │ Auth, AuditTrail, AccessMgr   │
│ PersonaApi     │     9    │ PersonaManager                │
│ ExtensionApi   │    12    │ Program, Skill, HostTools     │
│ McpApi         │     8    │ McpBridge                     │
│ InfraApi       │    22    │ Git, Scheduler, Cron,         │
│                │          │ Resource, EventBus, Config    │
├────────────────┼──────────┼──────────────────────────────┤
│ Facade 계      │    92    │                               │
│ KernelHandle   │     4    │ 편의 메서드                   │
│ 편의           │          │ (save_and_commit 등)          │
├────────────────┼──────────┼──────────────────────────────┤
│ Total          │    96    │ 기존 101 → 96                │
└────────────────┴──────────┴──────────────────────────────┘
```

---

## 10. 호출부 변경 예시

### Web Route Handler (88곳)

```rust
// ── agents/list ──────────────────────────────────────────
// Before:  state.kernel.list_agents().await
// After:   state.kernel.agents.list().await

// ── budget/get ───────────────────────────────────────────
// Before:  state.kernel.check_budget(&aid)
// After:   state.kernel.agents.check_budget(&aid)

// ── audit/entries ────────────────────────────────────────
// Before:  state.kernel.query_audit(from_seq, to_seq)
// After:   state.kernel.security.query_audit(from_seq, to_seq)

// ── git/log ──────────────────────────────────────────────
// Before:  state.kernel.git_log(100)
// After:   state.kernel.infra.git_log(100)

// ── personas/list ────────────────────────────────────────
// Before:  state.kernel.list_personas()
// After:   state.kernel.persona.list()

// ── programs/list ────────────────────────────────────────
// Before:  state.kernel.list_programs().await
// After:   state.kernel.extensions.list_programs().await

// ── mcp/list_tools ───────────────────────────────────────
// Before:  state.kernel.mcp_list_tools().await
// After:   state.kernel.mcp.list_tools().await

// ── cron/list ────────────────────────────────────────────
// Before:  state.kernel.list_schedules()
// After:   state.kernel.infra.list_crons()  (또는 별도 쿼리)

// ── chat/stream (session) ────────────────────────────────
// Before:  state.kernel.load_session(&session_id).await
// After:   state.kernel.state.load_session(&session_id).await

// ── chat (auth) ──────────────────────────────────────────
// Before:  state.kernel.validate_token(token)
// After:   state.kernel.security.validate_token(token)
```

### CLI (main.rs, 7곳)

```rust
// Before:  kernel.handle().query_audit(0, 20)
// After:   kernel.handle().security.query_audit(0, 20)

// Before:  kernel.handle().check_budget(&uuid)
// After:   kernel.handle().agents.check_budget(&uuid)

// Before:  kernel.handle().git_log(limit)?
// After:   kernel.handle().infra.git_log(limit)?
```

### Guardian Daemon (src/kernel.rs, 8곳)

```rust
// Before:  handle.verify_audit()
// After:   handle.security.verify_chain()

// Before:  handle.is_overloaded()
// After:   handle.infra.is_overloaded()

// Before:  handle.resource_snapshot()
// After:   handle.infra.resource_snapshot()

// Before:  handle.git_verify()
// After:   handle.infra.git_verify()

// Before:  handle.commit_all("guardian: checkpoint")
// After:   handle.save_and_commit(...) / handle.state.commit_all(&handle.infra.git(), "...")
//       또는 KernelHandle 편의 메서드: handle.commit_all("guardian: checkpoint")
```

---

## 11. 마이그레이션 계획

### Phase 1: 디렉토리 생성 + 위임 (기계적, 컴파일 가능 상태 유지)

1. `kernel_handle.rs` → `kernel_handle/mod.rs` 로 이름 변경
2. 7개 Facade 파일 생성 (각 메서드를 기존 `self.xxx`에서 `self.<subsystem>.xxx`로 위임)
3. 기존 KernelHandle의 모든 메서드를 `#[deprecated]`로 표시하고 Facade에 위임
4. `cargo build` 성공 확인

```
// 예: 위임 패턴
impl KernelHandle {
    #[deprecated(note = "use handle.agents.list()")]
    pub async fn list_agents(&self) -> Result<Vec<AgentInfo>> {
        self.agents.list().await
    }
}
```

### Phase 2: Web routes 업데이트

1. `channels/oxios-web/src/routes/*.rs` (88곳) 일괄 변경
2. `channels/oxios-web/src/persona_routes.rs` (12곳) 일괄 변경
3. `channels/oxios-web/src/middleware.rs` (1곳) 변경
4. 컴파일 에러 + `#[deprecated]` 경고로 누락 파악

### Phase 3: CLI + Guardian 업데이트

1. `src/main.rs` (7곳) 변경
2. `src/kernel.rs` Guardian (8곳) 변경
3. `#[deprecated]` 메서드 제거

### Phase 4: 이름 정리

1. 접두사 제거 (`list_agents` → `list`, `mcp_list_servers` → `list_servers`)
2. 명확화 (`verify_audit` → `verify_chain`)

### Phase 5: 정리

1. 사용되지 않는 메서드 제거
2. `#[warn(dead_code)]` 확인
3. 문서 업데이트

---

## 12. 테스트 전략

### Facade 독립 테스트

```rust
#[cfg(test)]
mod tests {
    // StateApi: 임시 디렉토리로 StateStore 생성
    fn test_state_api() {
        let dir = tempfile::tempdir().unwrap();
        let state = StateApi {
            state_store: Arc::new(StateStore::new(dir.path().to_path_buf()).unwrap()),
        };
        // save → load → delete 사이클 테스트
    }

    // AgentApi: Mock Supervisor 주입
    fn test_agent_api() {
        let agents = AgentApi {
            supervisor: Arc::new(MockSupervisor::new()),
            budget_manager: Arc::new(BudgetManager::new()),
            memory_manager: Arc::new(MemoryManager::new(state_store)),
        };
        // list → check_budget → kill 테스트
    }

    // SecurityApi: 인메모리 AuthManager
    fn test_security_api() {
        let security = SecurityApi {
            auth_manager: Arc::new(Mutex::new(AuthManager::new())),
            audit_trail: Arc::new(AuditTrail::new(100)),
            access_manager: Arc::new(Mutex::new(AccessManager::new())),
        };
        // validate_token → audit → verify_chain 테스트
    }
}
```

### 통합 테스트

```rust
#[test]
fn test_kernel_handle_facade_composition() {
    let handle = create_test_kernel_handle();
    
    // Facade 간 독립성: 각 Facade의 메서드가 정상 동작
    assert!(handle.agents.list().await.is_ok());
    assert!(handle.security.validate_token("invalid") == false);
    assert!(handle.persona.list().is_empty());
    
    // KernelHandle 편의 메서드: State + Infra 연동
    handle.save_and_commit("test", "key", &serde_json::json!({"v": 1})).await.unwrap();
    let log = handle.infra.git_log(10).unwrap();
    assert!(log.len() > 0);
}
```

---

## 13. 리스크와 대응

| 리스크 | 영향 | 대응 |
|--------|------|------|
| Arc 공유 교차 | 없음 (v2에서 모든 Arc가 정확히 1개 Facade에만 속함) | 해당 없음 |
| InfraApi 22개 메서드로 가장 큼 | 발견성 약간 저하 | 하위 도메인별로 메서드가 명확히 구분됨 (Git/스케줄러/크론/리소스/이벤트/시스템). 추후 분리 필요시 CronApi 등으로 분리 가능 |
| KernelHandle 편의 메서드가 Facade 간 결합 생성 | State ↔ Infra 연동 명시적 | 4개뿐이고, 결합이 명시적이라 관리 가능 |
| 기존 API 호환성 깨짐 | 103곳 호출부 수정 필요 | `#[deprecated]` + Phase별 점진적 마이그레이션 |

---

## 14. v1→v2 변경 요약

| 항목 | v1 | v2 (본 문서) |
|------|----|----|
| Facade 수 | 5개 | 7개 (Persona, Mcp 분리) |
| 최대 Facade 크기 | 25개 (ExtensionApi) | 22개 (InfraApi) |
| Arc 교차 소유 | 1개 (git_layer) | 0개 |
| `new()` 파라미터 | 19개 서브시스템 | 7개 Facade |
| 크론 중복 | schedule/unschedule + add_cron/remove_cron | add_cron/remove_cron으로 통일 |
| 제거 메서드 | "15개" (명시 안 됨) | 16개 (구체적 목록) |
| 편의 메서드 | 없음 | 4개 (save_and_commit 등) |

---

## 15. 결론

이 설계는:
- **Micro-kernel이 아님** — 같은 프로세스, Arc 공유
- **God Object 해소** — 7개 Facade로 책임 분산, 최대 22개 메서드
- **Arc 교차 소유 제거** — 모든 서브시스템이 정확히 하나의 Facade에 속함
- **Rust 친화적** — 타입 안전, Arc 소유권, zero-cost
- **테스트 가능** — 각 Facade 독립 생성/테스트
- **실용적** — `#[deprecated]`로 점진적 마이그레이션, 컴파일 에러로 검증
