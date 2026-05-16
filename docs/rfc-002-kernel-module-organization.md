# RFC-002: Kernel Module Organization

> **상태:** 제안
> **날짜:** 2026-05-16
> **동기:** lib.rs의 41개 평평한 pub mod가 탐색성을 해친다. 하지만 파일 크기 자체는 문제가 아니다.

## 원칙

1. **줄 수로 판단하지 않는다.** `config.rs` 1086줄은 23개 설정 struct일 뿐이고, `scheduler.rs` 1102줄은 하나의 응집된 도메인이다. 쪼갤 이유가 없다.
2. **파일을 이동하지 않는다.** Rust에서 파일 이동은 모든 `use crate::` 경로를 바꾼다. 실제 이득이 명확할 때만 한다.
3. **클린 코드 강박에 빠지지 않는다.** "파일이 45개다 → 폴더 7개로 나누자"가 아니라, "탐색이 어려운 이유가 뭔가?"에서 시작한다.

## 변경 대상 (1개 파일)

### `crates/oxios-kernel/src/lib.rs`

**문제:** 41개 `pub mod`가 알파벳순으로 나열되어 있다. 새 사람이 "보안 관련 코드가 어디 있지?"라고 물으면 `access_manager`, `auth`, `capability`, `credential`, `audit_trail` 다섯 개를 뒤져야 한다.

**해결:** 파일을 이동하지 않고, `pub mod` 선언을 **의미별 섹션**으로 묶고 섹션 주석을 단다. re-export도 같은 섹션에 배치한다.

#### Before

```rust
pub mod a2a;
pub mod access_manager;
pub mod agent_group;
pub mod agent_lifecycle;
pub mod agent_runtime;
pub mod audit_trail;
pub mod backup;
pub mod budget;
pub mod capability;
pub mod circuit_breaker;
pub mod metrics;
pub mod auth;
pub mod config;
pub mod credential;
pub mod cron;
pub mod daemon;
pub mod embedding;
pub mod engine;
pub mod error;
pub mod event_bus;
pub mod git_layer;
pub mod host_tools;
pub mod mcp;
pub mod memory;
pub mod onboarding;
pub mod orchestrator;
pub mod persona;
pub mod persona_manager;
pub mod persona_store;
pub mod program;
pub mod resource_monitor;
pub mod scheduler;
pub mod skill;
pub mod space;
pub mod state_store;
pub mod supervisor;
pub mod tools;
pub mod types;
pub mod wasm_sandbox;
pub mod telemetry_otel;
pub mod telemetry_stub;
pub mod kernel_handle;

// ... 100줄 가까은 pub use ...
```

#### After

```rust
// ─── Lifecycle ──────────────────────────────────────────────────────
// Agent 생성, 실행, 종료. OS의 init+process management.
pub mod agent_group;
pub mod agent_lifecycle;
pub mod agent_runtime;
pub mod daemon;
pub mod supervisor;

// ─── Orchestration ──────────────────────────────────────────────────
// 작업 조율, 스케줄링, 예산 관리.
pub mod orchestrator;
pub mod scheduler;
pub mod cron;
pub mod budget;
pub mod circuit_breaker;

// ─── Security ───────────────────────────────────────────────────────
// 접근 제어, 인증, 권한, 감사.
pub mod access_manager;
pub mod auth;
pub mod capability;
pub mod credential;
pub mod audit_trail;

// ─── Communication ──────────────────────────────────────────────────
// 이벤트, 메시징, 외부 프로토콜.
pub mod event_bus;
pub mod a2a;
pub mod mcp;

// ─── Intelligence ───────────────────────────────────────────────────
// 메모리, 임베딩, 페르소나, 온보딩.
pub mod memory;
pub mod embedding;
pub mod persona;
pub mod persona_manager;
pub mod persona_store;
pub mod onboarding;

// ─── Tools & Programs ──────────────────────────────────────────────
// 에이전트가 사용하는 도구, 프로그램, 스킬.
pub mod tools;
pub mod host_tools;
pub mod program;
pub mod skill;
#[cfg(feature = "wasm-sandbox")]
pub mod wasm_sandbox;

// ─── State & Config ─────────────────────────────────────────────────
// 영속 상태, 설정, 백업, 리소스 모니터링.
pub mod state_store;
pub mod config;
pub mod backup;
pub mod git_layer;
pub mod resource_monitor;

// ─── Infrastructure ─────────────────────────────────────────────────
// 엔진, 에러, 타입, 메트릭, 텔레메트리.
pub mod engine;
pub mod error;
pub mod types;
pub mod metrics;
#[cfg(feature = "otel")]
pub mod telemetry_otel;
#[cfg(feature = "otel")]
pub use telemetry_otel as telemetry;
#[cfg(not(feature = "otel"))]
pub mod telemetry_stub;
#[cfg(not(feature = "otel"))]
pub use telemetry_stub as telemetry;

// ─── API Surface ────────────────────────────────────────────────────
// 외부에 노출하는 typed facade.
pub mod kernel_handle;
```

**re-export도 같은 섹션에 배치:**

```rust
// ─── Lifecycle exports ──────────────────────────────────────────────
pub use agent_lifecycle::AgentLifecycleManager;
pub use agent_runtime::AgentRuntime;
pub use agent_group::{OxiosAgentGroup, OxiosAgentGroupStatus, OxiosGroupAgent};
pub use daemon::{DaemonManager, DaemonStatus};
pub use supervisor::{BasicSupervisor, Supervisor};

// ─── Orchestration exports ──────────────────────────────────────────
pub use orchestrator::{AgentRole, OrchestrationResult, Orchestrator, SubTask};
pub use scheduler::{AgentScheduler, Priority, ScheduledTask, SchedulerStats, TaskStatus};
pub use cron::{CronJob, CronJobResult, CronJobUpdate, CronScheduler, JobSource};
pub use budget::{BudgetExceeded, BudgetInfo, BudgetKind, BudgetLimit, BudgetManager};
pub use circuit_breaker::CircuitBreaker;

// ... (각 섹션별로 동일한 패턴) ...
```

**효과:**
- 파일 이동 없음 → `use crate::` 경로 변경 없음 → 기존 코드 영향 제로
- `lib.rs`만 수정
- "보안은 어디?" → Security 섹션만 보면 됨

## 변경하지 않는 것 (명시적)

| 항목 | 이유 |
|------|------|
| `config.rs` (1086줄) | 설정 struct 23개. 하나당 ~47줄. 쪼개면 23개 파일을 왔다갔다 해야 함 |
| `scheduler.rs` (1102줄) | 우선순위 큐 + 레이트 리미터 + 좀비 탐지. 하나의 응집된 도메인 |
| `audit_trail.rs` (1135줄) | 해시 체인 + 검증 + 직렬화. 하나로 읽어야 이해됨 |
| `orchestrator.rs` (1052줄) | Ouroboros 루프 조율. 책임이 하나임 |
| `oxios-ouroboros` (1052줄) | 7파일, 각 페이즈별 응집. 정확히 필요한 만큼 |
| `oxios-kernel` 분리 | 아직 시기상조. 의존성 방향이 안정화되지 않음 |
| `a2a.rs` 이동 | event_bus, types만 쓰고 kernel_handle에서 참조. 지금 위치가 무난함 |

## 나중에 고려할 것

kernel의 41개 모듈이 계속 자라나면, 그때 **실제 하위 크레이트 분리**를 고려한다. 기준은:

1. **순환 의존이 발생하는가?** — 그러면 경계가 틀렸다는 뜻
2. **다른 크레이트가 그 모듈만 필요로 하는가?** — 그러면 분리 이득이 있음
3. **컴파일 시간이 병목인가?** — 그러면 분리로 인크리멘탈 빌드 이득

지금은 어느 것도 해당하지 않는다.

## 작업 범위

- 수정 파일: `crates/oxios-kernel/src/lib.rs` 1개
- 코드 변경: 없음 (선언 순서 + 주석만)
- 위험도: 제로
- 예상 소요: 10분
