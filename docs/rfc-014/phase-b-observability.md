# Phase B: Observability 정리

> **위험**: 낮음
> **예상 시간**: 1시간
> **선행**: Phase A

---

## 현재 상태

`crates/oxios-kernel/src/observability.rs`는 oxi-sdk의 관측 가능성 타입을
글로벌 `OnceLock` 인스턴스로 관리한다.

```rust
use oxi_sdk::ModelRegistry;
pub use oxi_sdk::{
    AuditEntry, AuditFilter, AuditLog, CostBreakdown, CostSnapshot, CostTracker,
    CostTrackerConfig, GlobalCostSnapshot, Span, SpanContext, SpanGuard, SpanId,
    SpanKind, SpanStatus, TokenUsage, TraceId, Tracer,
};

static TRACER: OnceLock<Tracer> = ...;
static COST_TRACKER: OnceLock<CostTracker> = ...;
static AUDIT_LOG: OnceLock<AuditLog> = ...;
```

0.26.0에서 `AuditEntry`, `AuditFilter`, `AuditLog`는 그대로 유지된다.
`audit_trail.rs`는 dormant이므로 이 Phase에서는 영향 없다.

## 작업

### 1. 불필요한 re-export 정리

0.26.0에서 추가된 관측 가능성 타입을 확인하고, 필요한 것만 re-export:

```rust
// 변경 후 — 필요한 것만 명시적으로 re-export
pub use oxi_sdk::{
    // Tracing
    Span, SpanContext, SpanGuard, SpanId, SpanKind, SpanStatus, TraceId, Tracer,
    // Cost
    CostBreakdown, CostSnapshot, CostTracker, CostTrackerConfig, GlobalCostSnapshot, TokenUsage,
    // Audit (단순 버전 — AuditTrail 전환 전까지 유지)
    AuditEntry, AuditFilter, AuditLog,
    // ModelRegistry (CostTracker 생성에 필요)
    ModelRegistry,
};
```

변화 없음 — 현재와 동일. 단지 정리 목적.

### 2. 글로벌 인스턴스 초기화 순서 확인

`observability::init()`이 커널 부팅 시 호출되는지 확인.
이미 `kernel.rs`에서 호출되고 있으면 변경 없음.

---

## 검증 기준

- [ ] `cargo test -p oxios-kernel` 통과
- [ ] 기존 기능에 영향 없음
