# Oxios Kernel Extraction Plan

> Generated: 2026-05-23
> Scope: `oxios-kernel` (39,534 LOC, 105 files) → modular crate architecture
> Principle: 안전하고 점진적인 분리. 한 번에 하나씩, 매 단계에서 컴파일 + 테스트 통과.

---

## 0. 현재 상태

```
oxios-kernel/ (39,534 LOC)
├── Root modules (35 files, ~15,500 LOC)
│   ├── orchestrator.rs      1,260   ← brain (12 internal deps)
│   ├── scheduler.rs         1,106   ← task queue
│   ├── supervisor.rs          577   ← agent lifecycle
│   ├── agent_runtime.rs       803   ← LLM tool loop
│   ├── agent_lifecycle.rs     211   ← lifecycle coordinator
│   ├── engine.rs              209   ← provider factory
│   ├── state_store.rs         544   ← file persistence
│   ├── event_bus.rs           364   ← broadcast events
│   ├── config.rs            1,203   ← 20+ config structs
│   ├── daemon.rs              330   ← PID/service mgmt
│   ├── budget.rs              641   ← token/call budgets
│   ├── circuit_breaker.rs     248   ← fault tolerance
│   ├── audit_trail.rs       1,135   ← merkle audit log
│   ├── a2a.rs                 876   ← agent-to-agent
│   ├── a2a_circuit_breaker    176   ← A2A breaker
│   ├── cron.rs                836   ← scheduled jobs
│   ├── git_layer.rs           576   ← in-process git
│   ├── resource_monitor.rs    359   ← system resources
│   ├── metrics.rs             395   ← metrics registry
│   ├── onboarding.rs          509   ← setup wizard
│   ├── auth.rs                269   ← API key auth
│   ├── credential.rs          136   ← multi-source creds
│   ├── embedding.rs           183   ← embedding abstraction
│   ├── persona_store.rs       179   ← persona storage
│   ├── persona_manager.rs     130   ← persona management
│   ├── persona.rs             145   ← persona types
│   ├── error.rs               288   ← error types
│   ├── types.rs               191   ← AgentId, AgentStatus
│   ├── host_tools.rs          271   ← host tool validation
│   ├── agent_group.rs         191   ← agent grouping
│   ├── backup.rs              114   ← state backup
│   ├── wasm_sandbox.rs        385   ← WASM sandbox (feature-gated)
│   ├── skill.rs               293   ← skill definitions
│   └── lib.rs                 205   ← crate root
│
├── memory/              6,843   ← vector store, HNSW, learning
├── tools/               6,394   ← agent tool implementations
├── access_manager/      2,128   ← RBAC, sandboxing
├── space/               1,948   ← logical work partitions
├── kernel_handle/       1,908   ← unified facade (13 APIs)
├── program/             1,701   ← installable programs
├── mcp/                 1,536   ← MCP protocol client
├── capability/            961   ← capability tokens
└── workers/               590   ← background workers
```

---

## 1. 분리 원칙

1. **순환 의존 금지** — 추출된 크레이트는 kernel에 의존하지 않는다. kernel이 추출된 크레이트에 의존한다.
2. **공유 타입 최소화** — 여러 크레이트가 공유하는 타입은 `oxios-types` 로.
3. **컴파일 게이트** — 매 단계마다 `cargo test --workspace` 통과.
4. **퍼블릭 API 보존** — `oxios_kernel::XXX` 경로는 re-export로 유지. 외부 크레이트(oxios-web) 수정 최소화.
5. **Feature gate 유지** — 기존 feature 구성 깨지지 않게.

---

## 2. 공유 크레이트: `oxios-types`

추출 과정에서 여러 크레이트가 공유하게 될 타입들. 가장 먼저 만들어야 함.

### 현재 위치와 사용처

| 타입 | 현재 위치 | 사용 크레이트/모듈 |
|------|----------|------------------|
| `AgentId` (= `Uuid`) | `types.rs` | 164곳 (kernel 전체 + web) |
| `AgentStatus` | `types.rs` | orchestrator, supervisor, a2a, tools, web |
| `AgentInfo` | `types.rs` | supervisor, tools, kernel_handle |
| `ToolDef` | `program/types.rs` | mcp, tools, kernel_handle |
| `ArgumentDef` | `program/types.rs` | mcp, tools, web (infra.rs) |
| `InstallSource` | `program/types.rs` | kernel_handle, web (resources.rs) |
| `ProgramHostRequirements` | `program/types.rs` | tools |
| `HostRequirementsCheck` | `program/types.rs` | kernel_handle, web |
| `McpServerConfig` | `program/types.rs` | mcp |

### 크레이트 내용

```
crates/oxios-types/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── agent.rs      ← AgentId, AgentStatus, AgentInfo
    ├── tool.rs       ← ToolDef, ArgumentDef
    └── program.rs    ← InstallSource, ProgramHostRequirements, HostRequirementsCheck, McpServerConfig
```

### 의존성

```toml
[dependencies]
serde = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true, features = ["v4", "serde"] }
```

**외부 의존 없음.** 순수 데이터 타입.

### 마이그레이션

```rust
// oxios-kernel/src/types.rs
pub use oxios_types::agent::{AgentId, AgentStatus, AgentInfo};

// oxios-kernel/src/program/types.rs  
pub use oxios_types::tool::{ToolDef, ArgumentDef};
pub use oxios_types::program::{InstallSource, ProgramHostRequirements, ...};
```

기존 `oxios_kernel::AgentId` 경로는 re-export로 보존. 외부 수정 불필요.

---

## 3. Phase 1: `oxios-types` 생성

### 작업

1. `crates/oxios-types/` 생성
2. 타입 이동 (copy → re-export)
3. `Cargo.toml` workspace members 추가
4. `oxios-kernel` 의존성에 `oxios-types` 추가
5. kernel 내 `use crate::types::AgentId` → `use oxios_types::AgentId` (점진적)
6. `cargo test --workspace`

### 영향 범위

- kernel 내부: import 경로 변경 (타입 정의 변경 아님)
- oxios-web: 변경 없음 (`oxios_kernel::AgentId` re-export로 유지)
- 나머지 크레이트: 영향 없음

### 위험도: ★☆☆☆☆ (매우 낮음)

순수 데이터 타입 이동. 로직 변경 없음.

---

## 4. Phase 2: `oxios-workers` 추출

### 근거

- **590 LOC, 파일 1개**
- **kernel 내부 의존성: 0개** (`use crate::` 없음)
- Worker 구현은 현재 스텁 (요약 문자열 반환)
- 완벽한 독립 모듈

### 작업

1. `crates/oxios-workers/` 생성
2. `workers/mod.rs` → `crates/oxios-workers/src/lib.rs` 로 이동
3. `Cargo.toml` 업데이트
4. kernel에서 `pub use oxios_workers` re-export
5. `cargo test --workspace`

### 의존성

```toml
# oxios-workers/Cargo.toml
[dependencies]
parking_lot = { workspace = true }
serde = { workspace = true }
```

### 마이그레이션

```rust
// oxios-kernel/src/lib.rs
pub mod workers;  // → 제거
pub use oxios_workers;  // → re-export
```

### 위험도: ★☆☆☆☆ (매우 낮음)

제로 의존성 모듈. 추출 패턴 검증용.

---

## 5. Phase 3: `oxios-security` 추출

### 근거

- **2,128 LOC, 파일 3개**
- **kernel 내부 의존성: 1개** (`types::AgentId`)
- Phase 1 완료 후 `oxios_types::AgentId` 로 해결
- RBAC, 샌드박싱, 감사 로깅 — 명확한 보안 도메인

### 현재 kernel 내 사용처 (5곳)

| 사용처 | 임포트 |
|--------|--------|
| `tools/exec_tool.rs` | `AccessManager` |
| `agent_lifecycle.rs` | `AccessManager` |
| `kernel_handle/security_api.rs` | `AccessManager`, `AuditEntry`, `Role`, ... |
| `kernel_handle/exec_api.rs` | `AccessManager` |
| `kernel_handle/mod.rs` | `AccessManager` |

### 작업

1. `crates/oxios-security/` 생성
2. `access_manager/mod.rs`, `rbac.rs`, `permissions.rs` 이동
3. `oxios_types::AgentId` 사용
4. kernel에서 re-export
5. `cargo test --workspace`

### 의존성

```toml
# oxios-security/Cargo.toml
[dependencies]
oxios-types = { path = "../oxios-types" }
chrono = { workspace = true }
glob = "0.3"
serde = { workspace = true }
tokio = { workspace = true }  # mpsc channel for audit persistence
```

### web 영향

```rust
// channels/oxios-web/src/routes/infra.rs — 현재
use oxios_kernel::access_manager::AuditEntry;

// 분리 후에도 그대로 동작 (kernel re-export 유지)
// 선택적으로 나중에 oxios_security::AuditEntry 로 변경 가능
```

### 위험도: ★★☆☆☆ (낮음)

의존성이 `AgentId` 하나. API 변경 없음.

---

## 6. Phase 4: `oxios-mcp` 추출

### 근거

- **1,536 LOC, 파일 3개**
- **kernel 내부 의존성: 1개** (`program::ToolDef`)
- Phase 1 완료 후 `oxios_types::ToolDef` 로 해결
- 완전한 JSON-RPC 2.0 프로토콜 구현

### 현재 kernel 내 사용처 (3곳)

| 사용처 | 임포트 |
|--------|--------|
| `tools/mcp_tool.rs` | `McpBridge`, `McpContentBlock` |
| `kernel_handle/mcp_api.rs` | `McpBridge`, `McpServer`, `McpToolCallResult` |
| `kernel_handle/mod.rs` | `McpBridge` |

### 작업

1. `crates/oxios-mcp/` 생성
2. `mcp/client.rs`, `protocol.rs`, `mod.rs` 이동
3. `oxios_types::ToolDef` 사용
4. kernel에서 re-export
5. `cargo test --workspace`

### 의존성

```toml
# oxios-mcp/Cargo.toml
[dependencies]
oxios-types = { path = "../oxios-types" }
anyhow = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
parking_lot = { workspace = true }
```

### 위험도: ★★☆☆☆ (낮음)

자체적인 프로토콜 구현. 외부 의존 최소.

---

## 7. Phase 5: `oxios-program` 추출

### 근거

- **1,701 LOC, 파일 4개**
- **kernel 내부 의존성: 1개** (`host_tools::HostToolValidator`)
- Phase 1 완료 후 `ToolDef` 등 공유 타입 해결

### 현재 kernel 내 사용처 (5곳)

| 사용처 | 임포트 |
|--------|--------|
| `tools/program_tool.rs` | `ToolDef`, `ProgramHostRequirements` |
| `mcp/protocol.rs` | `ToolDef` (→ Phase 4에서 해결됨) |
| `mcp/mod.rs` | `ToolDef` (→ Phase 4에서 해결됨) |
| `kernel_handle/extension_api.rs` | `ProgramManager`, `InstallSource`, ... |
| `kernel_handle/mod.rs` | `ProgramManager` |

### `host_tools.rs` 처리

`host_tools.rs` (271 LOC) 는 단순한 `which` 명령어 래퍼. `program` 만 사용.
선택지:

- **A) host_tools를 oxios-program에 포함** — 가장 간단
- **B) oxios-types에 포함** — 다른 모듈도 사용할 수 있게

**선택: A** — 현재 program만 사용하므로 oxios-program에 포함.

### 의존성

```toml
# oxios-program/Cargo.toml
[dependencies]
oxios-types = { path = "../oxios-types" }
anyhow = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
```

### 위험도: ★★☆☆☆ (낮음)

---

## 8. Phase 6: `oxios-memory` 추출 (가장 큰 수확)

### 근거

- **6,843 LOC, 파일 16개** — kernel의 17%
- **kernel 내부 의존성: 3개**
  - `embedding::EmbeddingProvider` (trait + TF-IDF 구현)
  - `git_layer::GitLayer`
  - `state_store::StateStore`

### 현재 kernel 내 사용처 (8곳)

| 사용처 | 임포트 |
|--------|--------|
| `tools/memory_tools.rs` | `MemoryEntry`, `MemoryManager`, `MemoryType` |
| `agent_runtime.rs` | `MemoryEntry`, `MemoryManager`, `MemoryType` |
| `kernel_handle/mod.rs` | `MemoryManager` |
| `kernel_handle/knowledge_lens.rs` | `MemoryEntry`, `MemoryManager`, `MemoryType` |
| `kernel_handle/agent_api.rs` | `HnswMemoryIndex`, `SemanticHit`, `MemoryEntry`, `MemoryManager` |
| `space/space_bridge.rs` | `MemoryEntry`, `MemoryManager` |
| `embedding.rs` | `memory::normalizer::cosine_similarity_f32` (역방향!) |

### `embedding.rs` 역방향 의존성 문제

`embedding.rs` (183 LOC) 가 `memory::normalizer` 에 의존한다:

```rust
// embedding.rs
(EmbeddingVector::DenseF32(a), EmbeddingVector::DenseF32(b)) => {
    crate::memory::normalizer::cosine_similarity_f32(a, b) as f64
}
```

**해결: normalizer를 oxios-memory로 이동하면 embedding.rs도 함께 이동해야 함.**
또는 `cosine_similarity_f32`를 `oxios-types`나 `embedding.rs` 자체에 복제.

**선택: `embedding.rs`를 `oxios-memory`에 포함.**

이유:
- `embedding.rs`는 `EmbeddingProvider` trait과 `TfIdfEmbeddingProvider` 구현을 정의
- memory가 이 trait에 의존 (`sona.rs`, `reasoning_bank.rs`, `mod.rs`)
- 다른 kernel 모듈 중 embedding을 직접 사용하는 곳은 `tools/retrieval.rs` 뿐
- retrieval은 kernel에 남으므로, `oxios-memory`의 `EmbeddingProvider`를 re-export하면 됨

### `state_store` 및 `git_layer` 의존성

- `state_store` (544 LOC): leaf 모듈. 파일 I/O만. memory 외에도 orchestrator, space, kernel_handle에서 사용.
- `git_layer` (576 LOC): memory에서는 버전 관리용으로 선택적 사용. 다른 모듈에서도 사용.

**해결: trait으로 추상화.**

```rust
// oxios-memory에 trait 정의
pub trait StatePersistence: Send + Sync {
    async fn load_json(&self, category: &str, name: &str) -> Result<Option<String>>;
    async fn save_json(&self, category: &str, name: &str, data: &str) -> Result<()>;
    async fn list_category(&self, category: &str) -> Result<Vec<String>>;
}

pub trait VersionControl: Send + Sync {
    async fn commit(&self, message: &str) -> Result<()>;
    async fn log(&self, limit: usize) -> Result<Vec<CommitEntry>>;
}
```

kernel의 `StateStore`와 `GitLayer`는 이 trait을 구현. memory는 trait에만 의존.

### 의존성

```toml
# oxios-memory/Cargo.toml
[dependencies]
oxios-types = { path = "../oxios-types" }
anyhow = { workspace = true }
chrono = { workspace = true }
parking_lot = { workspace = true }
serde = { workspace = true }
uuid = { workspace = true }
lru = "0.10"
usearch = "2.25"
```

### 크레이트 구조

```
crates/oxios-memory/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── traits.rs           ← StatePersistence, VersionControl, EmbeddingProvider
    ├── embedding.rs        ← TfIdfEmbeddingProvider (kernel에서 이동)
    ├── normalizer.rs       ← 수학 유틸리티
    ├── manager.rs          ← MemoryManager (mod.rs에서 이름 변경)
    ├── store.rs            ← HnswMemoryIndex
    ├── auto_memory_bridge.rs
    ├── hyperbolic.rs
    ├── flash_attention.rs
    ├── sona.rs
    ├── rvf_store.rs
    ├── reasoning_bank.rs
    ├── hnsw.rs
    ├── graph.rs
    ├── embedding_cache.rs
    ├── chunking.rs
    ├── migrate.rs
    └── budget.rs
```

### 위험도: ★★★☆☆ (중간)

- trait 추상화 필요
- `embedding.rs` 이동으로 인한 import 경로 변경 다수
- 하지만 로직 변경은 없음. 타입과 trait만 이동.

---

## 9. Phase 7 (선택): `oxios-capability` 추출

### 근거

- **961 LOC, 파일 4개**
- **kernel 내부 의존성: 2개** (`types::AgentId`, `space` 의 Space 관련 타입)
- 순수 타입 시스템. 런타임 상태 없음.

### space 의존성 처리

`resolve.rs`가 `space::Space` 타입을 사용. 선택지:

- **A) space 관련 타입을 oxios-types에 추가** — `SpaceId`, `SpaceConfig` 등
- **B) resolve.rs를 kernel에 남김** — capability core만 추출

**선택: B** — capability core (types.rs, template.rs)만 추출. resolve.rs는 kernel에 남김.
이유: resolve는 kernel의 다른 서브시스템(Space, Seed)과 긴밀하게 연결되어 있고,
capability의 본질인 "토큰 타입 시스템"은 types.rs + template.rs로 충분.

### 위험도: ★★☆☆☆ (낮음)

---

## 10. 분리 후 최종 구조

```
oxios/
├── crates/
│   ├── oxios-types/        ← NEW (공유 타입)
│   │   └── ~200 LOC
│   ├── oxios-workers/      ← NEW (백그라운드 워커)
│   │   └── 590 LOC
│   ├── oxios-security/     ← NEW (RBAC + 샌드박싱)
│   │   └── 2,128 LOC
│   ├── oxios-mcp/          ← NEW (MCP 프로토콜)
│   │   └── 1,536 LOC
│   ├── oxios-program/      ← NEW (프로그램 관리)
│   │   └── 1,971 LOC (host_tools 포함)
│   ├── oxios-memory/       ← NEW (벡터 스토어 + 임베딩)
│   │   └── 7,026 LOC (embedding.rs 포함)
│   ├── oxios-capability/   ← NEW (선택, capability 토큰)
│   │   └── 750 LOC (resolve.rs 제외)
│   │
│   ├── oxios-kernel/       ← 축소됨 (24,500 LOC → 60%)
│   │   ├── orchestrator, scheduler, supervisor
│   │   ├── agent_runtime, agent_lifecycle
│   │   ├── state_store, event_bus
│   │   ├── config, daemon, budget
│   │   ├── circuit_breaker, audit_trail
│   │   ├── a2a, cron, git_layer
│   │   ├── space, tools (kernel_handle 의존)
│   │   ├── kernel_handle (facade, re-exports)
│   │   ├── persona, onboarding, skill
│   │   └── resource_monitor, metrics, engine
│   │
│   ├── oxios-ouroboros/    ← 변경 없음
│   ├── oxios-markdown/     ← 변경 없음
│   └── oxios-gateway/      ← 변경 없음
│
├── channels/               ← 변경 없음 (kernel re-export로 API 보존)
│   ├── oxios-web/
│   ├── oxios-cli/
│   └── oxios-telegram/
│
└── oxios (binary)          ← Cargo.toml만 업데이트
```

### 크기 변화

| 크레이트 | 현재 LOC | 분리 후 LOC |
|----------|----------|------------|
| `oxios-kernel` | 39,534 | ~24,500 |
| `oxios-memory` | — | 7,026 |
| `oxios-security` | — | 2,128 |
| `oxios-program` | — | 1,971 |
| `oxios-mcp` | — | 1,536 |
| `oxios-capability` | — | 750 |
| `oxios-workers` | — | 590 |
| `oxios-types` | — | ~200 |

### 의존성 그래프 (분리 후)

```
oxios-types ←─────────────────────────────────┐
    ↑         ↑        ↑        ↑        ↑    │
    │         │        │        │        │    │
oxios-    oxios-   oxios-   oxios-   oxios-   │
memory    security  mcp     program  capability│
    ↑         ↑        ↑        ↑              │
    └─────────┴────────┴────────┴──────→ oxios-kernel
                                              │ ↑
                    oxios-ouroboros ───────────┘ │
                    oxios-markdown ──────────────┘
                          ↑
                    oxios-gateway
                     ↑    ↑    ↑
                   web  cli  telegram
                     ↑
                 oxios (binary)
```

---

## 11. 실행 순서 및 체크포인트

```
Phase 1: oxios-types       ← 기반. 모든后续이 이것에 의존
  ✅ cargo test --workspace
  
Phase 2: oxios-workers     ← 가장 간단. 패턴 검증
  ✅ cargo test --workspace

Phase 3: oxios-security    ← 의존성 AgentId 하나
  ✅ cargo test --workspace

Phase 4: oxios-mcp         ← ToolDef를 types로
  ✅ cargo test --workspace

Phase 5: oxios-program     ← host_tools 포함
  ✅ cargo test --workspace

Phase 6: oxios-memory      ← 가장 복잡. trait 추상화 필요
  ✅ cargo test --workspace

Phase 7: oxios-capability  ← 선택적. resolve.rs는 kernel에 남김
  ✅ cargo test --workspace
```

### 각 Phase 소요 시간 추정

| Phase | 난이도 | 파일 변경 | 예상 시간 |
|-------|--------|----------|----------|
| 1. types | ★☆☆ | ~15 (import 경로) | 1-2시간 |
| 2. workers | ★☆☆ | ~5 | 30분 |
| 3. security | ★★☆ | ~10 | 1-2시간 |
| 4. mcp | ★★☆ | ~8 | 1시간 |
| 5. program | ★★☆ | ~10 | 1시간 |
| 6. memory | ★★★ | ~25 | 3-4시간 |
| 7. capability | ★★☆ | ~8 | 1시간 |

---

## 12. 추출하지 않는 것 (그리고 이유)

### `tools/` (6,394 LOC) — kernel에 잔류

이유: **12개 이상의 kernel 모듈에 의존.** 각 tool이 다른 서브시스템을 래핑:

```
ExecTool     → access_manager, config
MemoryTools  → memory
McpTool      → mcp
A2aTools     → a2a
ProgramTool  → program
BrowserTool  → oxibrowser-core
KernelTools  → kernel_handle (모든 API)
```

이걸 빼려면 모든 서브시스템을 trait으로 추상화해야 함. 비용 대비 효과 없음.

### `kernel_handle/` (1,908 LOC) — kernel에 잔류 (당연함)

이유: **facade by design.** 모든 서브시스템의 통합 진입점. 이게 빠지면 kernel의 의미가 없음.

### `space/` (1,948 LOC) — kernel에 잔류

이유: 4개 의존성 (`state_store`, `event_bus`, `memory`, `audit_trail`).
Medium coupling + 다른 핵심 모듈(orchestrator, agent_lifecycle)에서 직접 사용.
독립성이 떨어져 추출 효율 낮음.

### `config.rs` (1,203 LOC) — kernel에 잔류

이유: `SchedulerConfig`가 `scheduler::Priority` enum을 참조.
다른 모듈들의 설정을 모두 포함. 추출하려면 순환 의존 발생.
나중에 필요하면 config 내의 struct 들을 각 모듈로 분산시키는 방식 고려 가능.

### `orchestrator.rs` (1,260 LOC) — kernel에 잔류

이유: **brain.** 12개 내부 의존성. kernel의 본질.

### `state_store.rs` (544 LOC) — kernel에 잔류

이유: memory, space, orchestrator, kernel_handle 등 핵심 모듈이 모두 사용.
독립 추출하면 모든 곳에서 의존성만 증가. trait 추상화로 memory에서 분리하는 것으로 충분.

---

## 13. 컴파일 시간 영향

현재 `oxios-kernel` 수정 시 전체 재컴파일. 분리 후:

| 수정 대상 | 재컴파일 범위 |
|-----------|--------------|
| memory 내부 | oxios-memory → oxios-kernel (re-export) → binary |
| security 정책 | oxios-security → oxios-kernel → binary |
| mcp 프로토콜 | oxios-mcp → oxios-kernel → binary |
| program 로직 | oxios-program → oxios-kernel → binary |
| workers | oxios-workers만 (다른 크레이트 영향 없음) |

kernel은 여전히 re-export를 통해 모든 크레이트에 의존하지만,
**수정이 잦은 모듈(memory, mcp)이 독립 크레이트가 되면 증분 빌드가 빨라짐.**

---

## 14. 체크리스트 (각 Phase 공통)

매 Phase 완료 시 확인:

- [ ] `cargo build` — 컴파일 성공
- [ ] `cargo test --workspace` — 모든 테스트 통과
- [ ] `cargo clippy --workspace` — 경고 없음
- [ ] `oxios_kernel::XXX` 경로 re-export 확인 (외부 API 보존)
- [ ] 기존 feature gates 정상 동작 (`--features web,cli,browser`)
- [ ] web channel 정상 빌드 및 실행
