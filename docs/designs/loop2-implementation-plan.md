# Next Loop Implementation Plan — Loop 2

> **사전 조건:** Phase 1 완료 (보안 하드닝, 버그 수정, typed errors, auth, health)
> **현재 상태:** 245 테스트 통과, 21개 Clippy 경고, 빌드 클린
> **목표:** 품질 인프라 확립 + 핵심 테스트 추가 + 유지보수성 개선

---

## 루프 구조: 4개 스텝

```
Step 1: CI 파이프라인 (GitHub Actions + Justfile)
Step 2: Ouroboros 프로토콜 테스트 + Gateway 테스트
Step 3: routes.rs 도메인별 분할
Step 4: init_kernel() Builder 패턴 리팩토링
```

---

## Step 1: CI 파이프라인

### 이유
테스트와 빌드를 수동으로만 확인하는 상태. PR마다 자동 검증 없음.

### 파일
```
.github/workflows/ci.yml     (신규)
Justfile                       (신규 — 프로젝트 루트)
```

### `.github/workflows/ci.yml`
```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  check:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
        with:
          path: oxios
      - uses: actions/checkout@v4
        with:
          repository: a7garden/oxi
          path: oxi
          ref: v0.5.0
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - name: Check formatting
        working-directory: oxios
        run: cargo fmt --all -- --check
      - name: Clippy
        working-directory: oxios
        run: cargo clippy --workspace -- -D warnings
      - name: Test
        working-directory: oxios
        run: cargo test --workspace
```

### `Justfile`
```just
# Oxios 개발 명령어

default: build test

build:
    cargo build --workspace

release:
    cargo build --release

test:
    cargo test --workspace

lint:
    cargo clippy --workspace -- -D warnings

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

ci: fmt-check lint test

run:
    cargo run

frontend:
    cd channels/oxios-web/frontend && dx build --release

clean:
    cargo clean
```

### 완료 기준
- [x] `.github/workflows/ci.yml` 생성
- [x] `Justfile` 생성
- [x] `just ci` 로컬에서 통과

---

## Step 2: Ouroboros + Gateway 테스트

### 이유
**oxios-ouroboros (핵심 프로토콜 엔진) = 0 테스트**
**oxios-gateway (메시지 라우팅) = 0 테스트**

이것은 가장 위험한 갭. 프로토콜이 깨져도 알 수 없음.

### 파일
```
crates/oxios-ouroboros/tests/protocol_test.rs    (신규)
crates/oxios-ouroboros/tests/seed_test.rs         (신규)
crates/oxios-gateway/tests/gateway_test.rs        (신규)
```

### `oxios-ouroboros` 테스트

#### `tests/seed_test.rs` (순수 데이터 — LLM 필요 없음)
```
- test_seed_new_generates_id           — Seed 생성 시 id, goal, generation 확인
- test_seed_evolve_increments_generation — evolve 시 generation + 1
- test_seed_evolve_tracks_parent        — evolved_from() = 부모 id
- test_seed_immutable_after_evolve      — 원본 seed는 변하지 않음
- test_ambiguity_score_calculation       — 가중치 (goal 40%, constraints 30%, criteria 30%)
- test_ambiguity_score_low_when_complete — 모든 필드 충족 시 < 0.2
- test_ambiguity_score_high_when_empty   — 빈 입력 시 > 0.5
- test_evaluation_result_all_passed      — mechanical + semantic 통과 시 all_passed()
- test_evaluation_result_partial         — 일부만 통과 시 !all_passed()
- test_interview_result_updates_ambiguity — Q&A 추가 시 모호성 업데이트
```

#### `tests/protocol_test.rs` (MockOuroboros로 전체 흐름)
```
- test_protocol_interview_to_seed       — MockOuroboros로 interview → seed 흐름
- test_protocol_evolution_increases_quality — 평가 실패 → evolve → 재평가
- test_protocol_phase_transitions        — Phase 순서 검증
```

#### MockOuroboros 구조 (integration_tests.rs의 기존 패턴 재사용)
```rust
struct MockOuroboros {
    interview_responses: Vec<String>,
    seed_goal: String,
    evaluation_pass: AtomicBool,
}
```

### `oxios-gateway` 테스트

#### `tests/gateway_test.rs`
```
- test_gateway_registers_channel         — 채널 등록 후 route() 동작
- test_gateway_unknown_channel_warns     — 알 수 없는 채널에 send_to() → 경고 로그
- test_gateway_message_roundtrip         — 메시지 송수신 왕복
- test_gateway_run_processes_messages    — run() 루프에서 메시지 처리
```

### 완료 기준
- [x] ouroboros: 10+ 테스트 추가 (seed, ambiguity, evaluation, interview)
- [x] gateway: 4+ 테스트 추가
- [x] `cargo test --workspace` 여전히 전체 통과

---

## Step 3: routes.rs 도메인별 분할

### 이유
`routes.rs` = 1,812줄. 모든 HTTP 핸들러가 단일 파일에 있음.
새 엔드포인트 추가 시 스크롤이 과도하고, 코드 리뷰가 어려움.

### 현재 구조
```
channels/oxios-web/src/
├── routes.rs           (1,812줄 — 모든 핸들러)
├── persona_routes.rs   (222줄 — 이미 분리됨)
```

### 목표 구조
```
channels/oxios-web/src/
├── routes/
│   ├── mod.rs              (~60줄 — build_routes 조합)
│   ├── chat.rs             (~200줄 — POST /api/chat, WebSocket)
│   ├── control.rs          (~120줄 — status, agents, kill)
│   ├── config_routes.rs    (~130줄 — config get/put)
│   ├── workspace.rs        (~220줄 — tree, file get/put)
│   ├── seeds.rs            (~160줄 — seeds list/get/evolution)
│   ├── skills.rs           (~120줄 — skills CRUD)
│   ├── memory.rs           (~90줄 — memory list/get)
│   ├── gardens.rs          (~280줄 — gardens CRUD + exec)
│   ├── scheduler.rs        (~80줄 — stats, tasks)
│   ├── security.rs         (~140줄 — audit, permissions)
│   ├── programs.rs         (~220줄 — programs CRUD)
│   ├── host_tools.rs       (~60줄 — host tools check)
│   ├── events.rs           (~80줄 — SSE stream)
│   ├── sessions.rs         (~110줄 — sessions CRUD)
│   ├── approvals.rs        (~130줄 — HitL approvals)
│   └── persona_routes.rs   (222줄 — 기존 파일 이동)
├── middleware.rs           (이미 존재)
├── server.rs
├── channel.rs
└── lib.rs
```

### `routes/mod.rs` 구조
```rust
pub mod chat;
pub mod control;
pub mod config_routes;
// ...

mod shared_types;  // 공통 Request/Response 타입

pub fn build_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(chat::handle_health))
        .route("/dioxus", get(|| async { Redirect::permanent("/dioxus/") }))
        .merge(chat::routes())
        .merge(control::routes())
        .merge(config_routes::routes())
        .merge(workspace::routes())
        // ...
        .with_state(state)
}

// 각 모듈의 routes() 함수는 Router<Arc<AppState>> 반환
// 예: chat::routes() → Router<Arc<AppState>> with /api/chat, /api/chat/stream
```

### 각 모듈 패턴
```rust
// routes/chat.rs
use axum::{...};

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/chat", post(handle_chat))
        .route("/api/chat/stream", get(handle_chat_stream))
}

async fn handle_chat(...) { /* 기존 코드 이동 */ }
async fn handle_chat_stream(...) { /* 기존 코드 이동 */ }
```

### 공유 타입 처리
`routes.rs`에 정의된 Deserialize/Serialize 구조체(ChatRequest, GardenCreateRequest 등)는
각 모듈로 이동하거나 `shared_types` 모듈에 배치.

### 완료 기준
- [x] routes.rs → routes/ 디렉토리로 분할 (15개 파일)
- [x] 각 파일 300줄 이하
- [x] `cargo build --workspace` 통과
- [x] `cargo test --workspace` 통과
- [x] API 엔드포인트 동작 변경 없음 (순수 리팩토링)

---

## Step 4: init_kernel() Builder 패턴 리팩토링

### 이유
`init_kernel()`이 16-튜플을 반환. 이것이 BUG-1(persona_manager 중복 생성)의 근본 원인.
튜플 인덱스 잘못 참조하면 런타임에만 발견.

### 현재
```rust
async fn init_kernel(
    config_path: &Path,
    model_id: &str,
) -> Result<(
    Arc<Orchestrator>,
    Gateway,
    EventBus,
    Arc<StateStore>,
    Arc<ContainerManager>,
    OxiosConfig,
    SkillStore,
    Arc<dyn Supervisor>,
    Arc<AgentScheduler>,
    Arc<Mutex<AccessManager>>,
    Arc<ProgramManager>,
    HostToolValidator,
    PersonaManager,
    Arc<A2AProtocol>,
    Arc<Mutex<McpBridge>>,
    Arc<Mutex<AuthManager>>,  // Loop 1에서 추가
)>
```

### 목표
```rust
/// Oxios 커널 빌더
pub struct Kernel {
    pub orchestrator: Arc<Orchestrator>,
    pub gateway: Gateway,
    pub event_bus: EventBus,
    pub state_store: Arc<StateStore>,
    pub container_manager: Arc<ContainerManager>,
    pub config: OxiosConfig,
    pub skill_store: SkillStore,
    pub supervisor: Arc<dyn Supervisor>,
    pub scheduler: Arc<AgentScheduler>,
    pub access_manager: Arc<Mutex<AccessManager>>,
    pub program_manager: Arc<ProgramManager>,
    pub host_tool_validator: HostToolValidator,
    pub persona_manager: PersonaManager,
    pub a2a_protocol: Arc<A2AProtocol>,
    pub mcp_bridge: Arc<Mutex<McpBridge>>,
    pub auth_manager: Arc<Mutex<AuthManager>>,
}

impl Kernel {
    pub fn builder() -> KernelBuilder {
        KernelBuilder::default()
    }
}

#[derive(Default)]
pub struct KernelBuilder {
    config_path: Option<PathBuf>,
    model_id: Option<String>,
}

impl KernelBuilder {
    pub fn config_path(mut self, path: PathBuf) -> Self { ... }
    pub fn model_id(mut self, model: &str) -> Self { ... }

    pub async fn build(self) -> Result<Kernel> {
        // init_kernel 로직을 여기로 이동
        // persona_manager 한 번만 생성
        // a2a_protocol 한 번만 생성
    }
}
```

### main.rs 사용법 변경
```rust
// BEFORE
let (orchestrator, gateway, event_bus, state_store, container_manager,
     config, skill_store, supervisor, scheduler, access_manager,
     program_manager, host_tool_validator, persona_manager, _a2a,
     mcp_bridge, auth_manager) = init_kernel(&config_path, default_model).await?;

// AFTER
let kernel = Kernel::builder()
    .config_path(config_path)
    .model_id(default_model)
    .build()
    .await?;
```

### 파일
```
crates/oxios-kernel/src/kernel.rs    (신규 — Kernel 구조체 + Builder)
crates/oxios-kernel/src/lib.rs       (수정 — pub mod kernel, pub use)
src/main.rs                          (수정 — Kernel 사용으로 전환)
```

### 완료 기준
- [x] `Kernel` 구조체 + `KernelBuilder` 구현
- [x] `main.rs` 모든 서브커맨드가 `Kernel` 사용
- [x] 16-튜플 반환 제거
- [x] `cargo test --workspace` 통과
- [x] BUG-1 재발 불가 (구조적으로 차단)

---

## 구현 순서 및 의존성

```
Step 1: CI 파이프라인
  │   (독립적 — 다른 스텝에 의존하지 않음)
  ▼
Step 2: Ouroboros + Gateway 테스트
  │   (CI가 먼저 있어야 자동 검증 가능)
  ▼
Step 3: routes.rs 분할
  │   (테스트가 있어야 리팩토링 안전)
  ▼
Step 4: init_kernel() Builder 리팩토링
      (가장 위험도가 높은 변경 — 마지막에)
```

**예상 소요 시간:** 각 스텝 1-2일, 총 4-8일

---

## 루프 완료 후 예상 상태

| 항목 | Loop 1 (현재) | Loop 2 (목표) |
|------|---------------|---------------|
| 테스트 | 245개 | 265+개 |
| CI/CD | 없음 | GitHub Actions |
| routes.rs | 1,812줄 단일 파일 | 15개 모듈, 각 300줄 이하 |
| init_kernel() | 16-튜플 반환 | Builder 패턴 |
| Clippy 경고 | 21개 | 5개 이하 |
| 커버되지 않은 크레이트 | ouroboros, gateway | 테스트 추가 |
| 프로덕션 점수 | 58/100 | **68/100** |

---

## 이후 루프 예고 (Loop 3)

Loop 2 완료 후 진행 가능:
- Phase 3: Metrics (Prometheus), oxi 의존성 git 태그 전환
- Phase 4: Ouroboros execute() 실구현, Audit log 영속화
- TUI: ratatui 대시보드 (oxi-tui 위젯 재사용)
