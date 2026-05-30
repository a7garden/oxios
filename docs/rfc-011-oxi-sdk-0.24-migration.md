# RFC-011: oxi-sdk 0.23.0 → 0.24.0 마이그레이션

> **상태**: Draft
> **날짜**: 2026-05-30
> **범위**: oxi-sdk 의존성 업그레이드 및 API 정렬
> **영향 크레이트**: oxios-kernel, oxios-ouroboros, oxios-web

---

## 1. 배경

oxi-sdk 0.24.0이 crates.io에 배포되었다. 핵심 변경사항:

| 영역 | 변경 내용 | 영향도 |
|------|-----------|--------|
| **Model Routing** | `ComplexityRouter` + `MultiProviderBuilder` + `FallbackChain` 실구현 | High |
| **Runtime Routing** | `RoutingControl` (runtime toggle, exclude, fallback) 추가 | High |
| **Agent Supervisor** | SDK 내장 `AgentSupervisor` + `AgentHandle` + `SnapshotStore` | Medium |
| **Middleware** | `MiddlewarePipeline`, `MiddlewarePhase`, built-in middleware 실구현 | Medium |
| **Observability** | `Tracer`, `CostTracker`, `AuditLog`, `EventStore` 정식 API | Medium |
| **Security** | `Authorizer`, `CapabilitySet`, `SecurityMiddleware` | Low |
| **Coordination** | `WorkQueue`, `SharedMemory`, `Consensus`, `CoordinatedGroup` | Low |
| **Agent Builder** | `.enable_routing()`, `.supervisor()`, `provider_factory()` | High |
| **Error 타입** | `SdkError` + `SdkResult` 통일 | Low |
| **Token Usage** | `AgentHandle::run()`에서 `AgentEvent::Usage` 집계 | Low |

### 1.1 구조 비교

```
oxi-sdk 0.23.0                     oxi-sdk 0.24.0
─────────────────────              ─────────────────────────────────
src/lib.rs                         src/lib.rs (+re-exports 확대)
src/builder.rs                     src/builder.rs (+enable_routing, supervisor)
src/agent_builder.rs               src/agent_builder.rs (동일 구조)
src/routing.rs  (신규)             → RoutingControl, RoutingConfig
src/multi_provider.rs (신규)       → MultiProviderBuilder, RoutingConfig
src/metrics.rs                     → AgentMetrics, MetricsSnapshot, extract_token_usage
src/lifecycle/ (신규)              → AgentSupervisor, AgentHandle, SnapshotStore
src/middleware/ (신규)             → MiddlewarePipeline, Built-in Middleware
src/coordination/ (신규)           → WorkQueue, SharedMemory, Consensus, CoordinatedGroup
src/observability/ (신규)          → Tracer, CostTracker, AuditLog, EventStore
src/security/ (신규)               → Authorizer, CapabilitySet, SecurityMiddleware
src/message_bus.rs                 → MessageBus, InterAgentMessage
src/closure_tool.rs                → ClosureTool
src/kernel_bridge.rs               → KernelToolProvider, KernelToolContext
src/tool_factory.rs                → coding_tools(), readonly_tools()
```

### 1.2 새 re-export (lib.rs)

```rust
// 신규 re-export
pub use routing::RoutingControl;
pub use multi_provider::{MultiProviderBuilder, RoutingConfig};
pub use lifecycle::{
    AgentHandle, AgentLifecycleEvent, AgentSnapshot, AgentStatus, AgentSupervisor,
    FileSnapshotStore, RestartBackoff, SnapshotStore, SupervisorPolicy, ToolManifest,
};
pub use middleware::{
    build_hooks, Middleware, MiddlewareContext, MiddlewareData, MiddlewarePhase,
    MiddlewarePipeline, MiddlewareResult,
};
pub use observability::{
    AuditEntry, AuditFilter, AuditLog, CostBreakdown, CostSnapshot, CostTracker,
    CostTrackerConfig, EventQuery, EventStore, EventStoreConfig, GlobalCostSnapshot,
    Span, SpanContext, SpanGuard, SpanId, SpanKind, SpanStatus, StoredEvent,
    TokenUsage, TraceId, Tracer,
};
pub use security::{
    Authorizer, Capability, CapabilitySet, CapabilitySubject, DefaultPolicy,
    SecurityMiddleware, StringPattern,
};
pub use coordination::{
    Consensus, CoordinatedGroup, CoordinatedGroupBuilder, MemoryEntry, MemoryEvent,
    MemoryKey, SharedMemory, VoteResult, WorkEvent, WorkItem, WorkQueue,
    WorkQueueConfig, WorkQueueStats, WorkResult, WorkStatus,
};

// oxi-ai 신규 re-export
pub use oxi_ai::multi_provider::MultiProviderConfig;
pub use oxi_ai::provider_pool::{ProviderPool, RateLimitPolicy};
pub use oxi_ai::circuit_breaker::{CircuitBreakerConfig, ProviderCircuitBreaker};
pub use oxi_ai::model_db::{...}; // model_db 전체
```

---

## 2. 현재 oxios의 oxi-sdk 사용 현황

### 2.1 직접 참조 통계

oxios-kernel에서 `oxi_sdk::` prefix로 참조하는 타입 (37개):

```
Agent, AgentTool, AgentToolResult, Api, CircuitBreakerConfig, Context,
find_env_keys, get_all_env_keys, get_env_api_key, get_provider_models,
get_providers, has_env_key, InterAgentMessage, KernelToolProvider,
load_token, Message, MessageBus, middleware, MiddlewarePipeline, Model,
ModelEntry, ModelRegistry, OutputMode, OxiBuilder, Provider,
ProviderCircuitBreaker, ProviderEvent, ProviderOptions, ReadTool, routing,
RoutingControl, save_token, search_models, SearchCache, SpanKind,
StructuredOutput, TokenBundle, TokenUsage, ToolError, ToolRegistry,
UserMessage
```

### 2.2 영향받는 파일

| 파일 | 사용 중인 API | 변경 필요 |
|------|--------------|-----------|
| `engine.rs` | `OxiBuilder`, `RoutingControl`, `ProviderPool`, `RateLimitPolicy` | `enable_routing()` 활용 |
| `agent_runtime.rs` | `Agent`, `AgentConfig`, `AgentEvent`, `CompactionStrategy`, `ProviderResolver`, `ToolRegistry`, `CircuitBreakerConfig` | Middleware 통합, Token Usage 집계 |
| `supervisor.rs` | `Agent`, `AgentPool` | SDK `AgentSupervisor`와의 관계 정립 |
| `observability.rs` | `Tracer`, `CostTracker`, `AuditLog`, `SpanKind`, `ModelRegistry` | SDK observability와 통합 |
| `coordination.rs` | `WorkQueue`, `SharedMemory`, `Consensus`, `CoordinatedGroup` | SDK 그대로 re-export |
| `tools/kernel_bridge.rs` | `AgentTool`, `AgentToolResult`, `ToolRegistry` | 변화 없음 |
| `orchestrator.rs` | 간접 참조 | 변화 없음 |
| `credential.rs` | `get_env_api_key`, `has_env_key`, `find_env_keys` | 변화 없음 |
| `onboarding.rs` | `search_models`, `get_providers` | 변화 없음 |

---

## 3. 마이그레이션 설계

### 3.1 Phase 1: Cargo.toml 업데이트 (즉시)

```toml
# Cargo.toml (workspace root)
oxi-sdk = "0.24.0"
```

**호환성 분석**: API 레벨에서 breaking change는 없다. 기존 0.23.0 타입이 0.24.0에서도 동일하게 유지됨. 새 기능은 additive.

### 3.2 Phase 2: OxiosEngine에 Routing 통합 (Medium)

**현재** (`engine.rs`):
```rust
pub struct OxiosEngine {
    oxi: Oxi,
    default_model_id: String,
    routing_control: Option<oxi_sdk::RoutingControl>,  // 수동 생성
    pools: parking_lot::RwLock<HashMap<String, Arc<dyn Provider>>>,
}
```

**변경 후**:
```rust
pub struct OxiosEngine {
    oxi: Oxi,
    default_model_id: String,
    routing_control: oxi_sdk::RoutingControl,  // 항상 존재
    pools: parking_lot::RwLock<HashMap<String, Arc<dyn Provider>>>,
}
```

**OxiosEngineBuilder 변경**:

```rust
impl OxiosEngineBuilder {
    /// 기존 build() — routing 비활성화
    pub fn build(self) -> OxiosEngine { ... }

    /// 라우팅 활성화 빌드
    pub fn build_with_routing(self) -> (OxiosEngine, oxi_sdk::RoutingControl) {
        let routing_config = oxi_sdk::routing::RoutingConfig::default();
        let routing_control = RoutingControl::new(routing_config);

        // OxiBuilder에 라우팅 설정 반영
        let inner = self.inner.enable_routing(
            oxi_sdk::RoutingConfig::new()
                .auto_routing(true)
                .prefer_cost_efficient(true)
        );

        let engine = OxiosEngine {
            oxi: inner.build(),
            default_model_id: self.default_model_id,
            routing_control: routing_control.clone(),
            pools: parking_lot::RwLock::new(HashMap::new()),
        };
        (engine, routing_control)
    }
}
```

**주요 변경점**:
- `routing_control: Option` → 항상 존재 (기본은 `RoutingControl::disabled()`)
- `OxiBuilder::enable_routing()`으로 MultiProvider 자동 구성
- `pooled_provider()`는 유지하되, 라우팅 활성 시 routing을 통한 fallback 지원

### 3.3 Phase 3: AgentRuntime Middleware 파이프라인 정식화 (Medium)

**현재** (`agent_runtime.rs`):
- middleware 설정이 있으나 실제 연결이 약함
- SDK의 `MiddlewarePipeline`을 직접 사용하지 않고 커스텀 훅 사용

**변경 후**:
```rust
impl AgentRuntime {
    fn build_agent_with_middleware(
        config: &AgentRuntimeConfig,
        engine: &OxiosEngine,
        tools: Arc<ToolRegistry>,
    ) -> Result<Agent> {
        let agent_config = AgentConfig {
            model_id: config.model_id.clone(),
            max_iterations: config.max_iterations,
            tool_execution: config.tool_execution,
            ..Default::default()
        };

        let mut builder = engine.oxi().agent(agent_config)
            .workspace(&config.workspace_dir)
            .tracer(Arc::new(observability::tracer().clone()))
            .cost_tracker(Arc::new(observability::cost_tracker().clone()))
            .audit_log(Arc::new(observability::audit_log().clone()));

        // Rate limiting
        if config.rate_limit_per_minute > 0 {
            builder = builder.with_rate_limit(config.rate_limit_per_minute);
        }

        // Token budget
        if config.token_budget > 0 {
            builder = builder.with_token_budget(config.token_budget);
        }

        // 커스텀 audit middleware
        if config.audit_enabled {
            builder = builder.middleware(AuditMiddleware::new(audit_trail));
        }

        // Tools (KernelBridge를 통한 등록은 유지)
        builder = builder.kernel_tools(&*kernel_provider, &context);

        builder.build()
    }
}
```

**이점**:
- SDK의 `MiddlewarePipeline`이 `AgentHooks`로 자동 변환 (`build_hooks()`)
- BeforeTool → RBAC 체크, AfterTool → audit 로깅 등이 구조화
- Token budget, rate limit이 SDK 미들웨어로 통합

### 3.4 Phase 4: Supervisor와 AgentSupervisor의 관계 정립 (Low)

**현재**: oxios의 `supervisor.rs`가 자체 `AgentPool`과 lifecycle을 관리
**SDK 신규**: `AgentSupervisor` + `AgentHandle` + `SnapshotStore` 제공

**전략: 점진적 위임**

```
현재:
  oxios Supervisor (fork/exec/wait/kill)
    ├── AgentPool (HashMap<AgentId, Arc<Agent>>)
    ├── AgentRuntime (tool loop)
    └── SessionContext

0.24.0 이후 (Phase 4):
  oxios AgentLifecycleManager
    ├── SDK AgentSupervisor (spawn/terminate/restart/snapshot)
    │   └── AgentHandle (status, metrics, routing)
    ├── AgentRuntime (tool loop — 변경 없음)
    └── SessionContext
```

**변경 내용**:
- `AgentLifecycleManager`가 내부적으로 `AgentSupervisor`를 사용
- `AgentHandle`의 `run()`, `suspend()`, `terminate()`, `restart()` 위임
- Snapshot persistence를 SDK의 `FileSnapshotStore`로 통합
- `RoutingControl` per-agent 관리는 `AgentHandle.routing()`으로

**단, 이 단계는 선택사항**:
- 기존 `supervisor.rs`는 여전히 동작함
- `AgentSupervisor`는 더 높은 수준의 lifecycle 관리이므로,
  oxios의 supervisor가 충분하면 그대로 유지 가능
- 권장: `AgentLifecycleManager`에서 SDK supervisor를 시도해보고,  
  기존 방식으로 fallback

### 3.5 Phase 5: Observability 글로벌 인스턴스 정리 (Low)

**현재** (`observability.rs`):
```rust
// OnceLock으로 글로벌 인스턴스 관리
static TRACER: OnceLock<Tracer> = OnceLock::new();
static COST_TRACKER: OnceLock<CostTracker> = OnceLock::new();
static AUDIT_LOG: OnceLock<AuditLog> = OnceLock::new();
```

**0.24.0에서의 변화**: 동일한 패턴 유지. `EventStore`가 추가됨.

**변경**:
```rust
/// Global EventStore instance (신규).
static EVENT_STORE: OnceLock<EventStore> = OnceLock::new();

pub fn event_store() -> &'static EventStore {
    EVENT_STORE.get_or_init(|| EventStore::new(EventStoreConfig::default()))
}
```

이것만 추가하면 됨. 기존 구조와 완전 호환.

---

## 4. 호환성 매트릭스

| oxios 모듈 | SDK 0.23.0 API | SDK 0.24.0 상태 | 조치 |
|------------|----------------|-----------------|------|
| `engine.rs` | `OxiBuilder::new().with_builtins()` | 동일 | 없음 |
| `engine.rs` | `OxiBuilder::api_key()` | 동일 | 없음 |
| `engine.rs` | `Oxi::resolve_model()` | 동일 | 없음 |
| `engine.rs` | `Oxi::create_provider()` | 동일 | 없음 |
| `engine.rs` | `ProviderPool::new()` | 동일 + re-export | 없음 |
| `engine.rs` | `RoutingControl` | **개선**: runtime toggle | Phase 2 |
| `agent_runtime.rs` | `Agent::new_with_resolver()` | 동일 | 없음 |
| `agent_runtime.rs` | `Agent::run_streaming()` | 동일 | 없음 |
| `agent_runtime.rs` | `AgentEvent::Usage` | **개선**: 실제 emit | Phase 3 |
| `agent_runtime.rs` | `MiddlewarePipeline` | **개선**: build_hooks() | Phase 3 |
| `supervisor.rs` | `Agent` pool | 동일 + SDK 대안 | Phase 4 (선택) |
| `observability.rs` | `Tracer`, `CostTracker`, `AuditLog` | 동일 + `EventStore` | Phase 5 |
| `coordination.rs` | re-export만 | 동일 | 없음 |
| `credential.rs` | `get_env_api_key`, `has_env_key` | 동일 | 없음 |

---

## 5. 작업 순서

```
Phase 1: Cargo.toml 업데이트 + 빌드 확인     [~10분]
   └── oxi-sdk = "0.24.0" 변경 후 cargo build

Phase 2: Engine 라우팅 통합                   [~2시간]
   ├── build_with_routing() 구현
   ├── RoutingConfig 기본값 설정
   └── Scheduler에 라우팅 상태 노출

Phase 3: AgentRuntime 미들웨어 정식화          [~3시간]
   ├── SDK MiddlewarePipeline 적용
   ├── Token Usage 집계 (extract_token_usage)
   ├── AuditMiddleware를 SDK 내장으로 교체
   └── AgentBuilder의 .with_rate_limit(), .with_token_budget() 활용

Phase 4: Supervisor 위임 (선택)               [~4시간]
   ├── AgentLifecycleManager에 AgentSupervisor 통합
   ├── FileSnapshotStore로 세션 영속화
   └── AgentHandle.routing()으로 per-agent routing

Phase 5: Observability 확장                   [~1시간]
   ├── EventStore 글로벌 인스턴스 추가
   └── TokenUsage → CostTracker 자동 연결
```

---

## 6. 위험 및 완화

| 위험 | 가능성 | 영향 | 완화 |
|------|--------|------|------|
| oxi-ai 타입 변경으로 컴파일 에러 | 낮음 | 중간 | Phase 1에서 `cargo build`로 즉시 확인 |
| RoutingConfig 기본값이 기존 동작 변경 | 낮음 | 낮음 | `RoutingControl::disabled()`로 명시적 비활성 |
| AgentSupervisor와 기존 Supervisor 충돌 | 중간 | 높음 | Phase 4는 선택사항으로 분리 |
| MiddlewarePipeline hooks 변환 버그 | 낮음 | 중간 | 기존 hooks 테스트로 회귀 확인 |

---

## 7. 테스트 계획

1. **Phase 1**: `cargo test --workspace` 전체 통과 확인
2. **Phase 2**: `OxiosEngine::build_with_routing()` → `RoutingControl.is_enabled()` 확인
3. **Phase 3**: `agent_runtime` 통합 테스트 — middleware 파이프라인 실행 확인
4. **Phase 4**: supervisor lifecycle 테스트 — spawn/suspend/restore/restart
5. **Phase 5**: observability 테스트 — EventStore 저장/조회 확인
6. **E2E**: `cargo run -- run --json "hello"` 기본 실행 확인

---

## 8. 커밋 전략

```
feat(deps): upgrade oxi-sdk 0.23.0 → 0.24.0              (Phase 1)
feat(kernel): integrate SDK routing into OxiosEngine      (Phase 2)
feat(kernel): adopt SDK middleware pipeline in runtime     (Phase 3)
refactor(kernel): delegate lifecycle to SDK supervisor     (Phase 4)
feat(kernel): add EventStore to observability globals      (Phase 5)
```

---

## 9. 결론

oxi-sdk 0.24.0은 **additive changes**만 포함한다. 기존 API가 깨지지 않으므로, Phase 1 (버전업)만으로도 즉시 빌드 가능하다. 그 위에 Phase 2~5는 점진적으로 SDK의 새 기능을 활용하는 개선 작업이다.

**권장 우선순위**: Phase 1 → Phase 3 → Phase 2 → Phase 5 → Phase 4 (선택)

Phase 3(미들웨어)이 Phase 2(라우팅)보다 먼저인 이유:
- 미들웨어는 모든 에이전트 실행에 영향을 미치고
- 라우팅은 고급 기능이므로 안정적인 기반 위에서 활성화하는 것이 안전
