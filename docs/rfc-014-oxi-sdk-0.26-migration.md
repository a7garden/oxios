# RFC-014: oxi-sdk 0.24.0 → 0.26.0 마이그레이션

> **상태**: ✅ Phase A, B, D, F 완료. Phase C, E, G 대기.
> **날짜**: 2026-06-03
> **커밋**: 4aace84 (HEAD)
> **범위**: SDK 대폭 업그레이드, 중복 코드 제거, 보안 계층 통합
> **영향 크레이트**: oxios-kernel (핵심), oxios-ouroboros (경미)
> **이전 RFC**: RFC-011 (0.22→0.24 마이그레이션)

---

## 1. 배경

### 1.1 왜 0.26.0인가

oxi-sdk 0.26.0은 oxios-kernel의 핵심 인프라를 SDK 레벨로 끌어올리는 작업이
진행된 버전이다. oxi 저장소의 `docs/design/oxios-migration.md`에서 설계된
Phase 1~4에 해당하는 코드가 파일로 준비되었다.

하지만 **준비된 파일 대부분이 아직 모듈에 등록되지 않은 상태**다.
즉 0.26.0은 "반쯤 완성된 마이그레이션"이며, oxios가 이 파일들을 활성화하고
중복을 제거하는 것이 이번 마이그레이션의 목표다.

### 1.2 oxi-sdk 버전 히스토리

| 버전 | 날짜 | oxios가 사용 | 핵심 변화 |
|------|------|-------------|-----------|
| 0.22.0 | 05-25 | ✅ RFC-011 | ProviderOptions, AgentLoopConfig |
| 0.24.0 | 05-30 | ✅ RFC-011 | MiddlewarePipeline, RoutingControl, AgentSupervisor 초안 |
| **0.26.0** | **06-01** | **← 이번** | EventBus\<E\>, AuditTrail(blake3), AgentPool, Security 계층 확장 |

### 1.3 현재 oxios의 oxi-sdk 의존

```toml
# Cargo.toml (workspace root) — Phase A에서 0.26.1로 업그레이드 완료
oxi-sdk = "0.26.1"
```

oxios-kernel은 oxi-sdk를 통해서만 AI 인프라에 접근한다 (oxi-ai, oxi-agent 직접 의존 없음).

---

## 2. oxi-sdk 0.26.0 변경 분석

### 2.1 모듈별 활성화 상태

oxi-sdk 0.26.0에 **파일은 존재하지만 `mod.rs` / `lib.rs`에 등록되지 않은** 코드가
다수 있다. 이를 "dormant"(휴면) 상태라고 부른다.

| 모듈 | 파일 | 라인 | mod.rs 등록 | 활성 | 비고 |
|------|------|------|------------|------|------|
| `event_bus.rs` | `src/event_bus.rs` | 175 | ❌ `lib.rs`에 없음 | 🔴 Dormant | 제네릭 `EventBus<E>` 구현 완료 |
| `lifecycle/agent_pool.rs` | `src/lifecycle/agent_pool.rs` | 139 | ❌ `lifecycle/mod.rs`에 없음 | 🔴 Dormant | `HashMap<String, Arc<Agent>>` 풀 |
| `observability/audit_trail.rs` | `src/observability/audit_trail.rs` | 973 | ❌ `observability/mod.rs`에 없음 | 🔴 Dormant | blake3 해시 체인 (Cargo.toml에 blake3 의존 누락) |
| `security/rbac.rs` | `src/security/rbac.rs` | 402 | ❌ | 🔴 Dormant | 3-tier RBAC |
| `security/gate.rs` | `src/security/gate.rs` | 462 | ❌ | 🔴 Dormant | 접근 게이트 |
| `security/permissions.rs` | `src/security/permissions.rs` | 249 | ❌ | 🔴 Dormant | 권한 정의 |
| `security/context.rs` | `src/security/context.rs` | 84 | ❌ | 🔴 Dormant | 보안 컨텍스트 |
| `security/exec_policy.rs` | `src/security/exec_policy.rs` | 104 | ❌ | 🔴 Dormant | 실행 정책 |
| `security/audit_sink.rs` | `src/security/audit_sink.rs` | 235 | ❌ | 🔴 Dormant | 감사 싱크 |
| **활성 모듈** | | | | | |
| `builder.rs` | OxiBuilder + Oxi | 451 | ✅ | 🟢 | credential() 추가 |
| `agent_builder.rs` | AgentBuilder | 437 | ✅ | 🟢 | .kernel_tools(), .browsing() 추가 |
| `security/capability.rs` | Capability + CapabilitySet | 570 | ✅ | 🟢 | preset: coding, read_only, research |
| `security/authorizer.rs` | Authorizer | 351 | ✅ | 🟢 | grant/check/revoke |
| `security/middleware.rs` | SecurityMiddleware | 240 | ✅ | 🟢 | BeforeTool hook |
| `lifecycle/supervisor.rs` | AgentSupervisor | 1060 | ✅ | 🟢 | spawn/run/suspend/terminate |
| `lifecycle/snapshot.rs` | SnapshotStore | 334 | ✅ | 🟢 | 파일 기반 스냅샷 |
| `middleware/mod.rs` | MiddlewarePipeline | 314 | ✅ | 🟢 | 변화 없음 |
| `observability/audit.rs` | AuditLog (단순) | 296 | ✅ | 🟢 | 변화 없음 |
| `observability/cost.rs` | CostTracker | 434 | ✅ | 🟢 | 변화 없음 |
| `observability/trace.rs` | Tracer | 303 | ✅ | 🟢 | 변화 없음 |
| `observability/event_store.rs` | EventStore | 236 | ✅ | 🟢 | 변화 없음 |

**총 휴면 코드**: ~2,824줄 (활성 코드 ~5,700줄 대비 33%)

### 2.2 Dormant 코드의 의미

휴면 파일은 oxi 측에서 "oxios에서 가져올 준비를 마친 코드"다.
하지만:

1. **AuditTrail**: `blake3`, `chrono` 크레이트가 `Cargo.toml`에 없어 컴파일 안 됨
2. **EventBus**: `lib.rs`에 `pub mod event_bus` 선언만 추가하면 즉시 사용 가능
3. **AgentPool**: `lifecycle/mod.rs`에 `mod agent_pool` 선언만 추가하면 즉시 사용 가능
4. **Security 계층** (rbac, gate, permissions 등): oxi-sdk의 Cargo.toml에 `chrono`, `uuid` 이미 있으므로 mod 선언만으로 활성화 가능

### 2.3 활성 API 변화 (0.24 → 0.26)

이미 활성화된 API에서 다음이 추가/변경되었다:

| API | 변화 | 영향 |
|-----|------|------|
| `OxiBuilder::credential()` | 신규 — api_key + base_url 동시 설정 | engine.rs에서 활용 가능 |
| `OxiBuilder::supervisor()` | 신규 — SupervisorBuilder 반환 | kernel.rs에서 활용 가능 |
| `AgentBuilder::kernel_tools()` | 신규 — KernelToolProvider 플러그 | kernel_bridge.rs와 직접 연동 |
| `AgentBuilder::browsing()` | 신규 — BrowserEngine 주입 | 현재 oxios는 별도 등록 |
| `AgentBuilder::capabilities()` | 신규 — CapabilitySet 설정 | access_manager와 보완적 사용 |
| `AgentBuilder::authorizer()` | 신규 — Authorizer 부착 | " |
| `AgentBuilder::tracer()` | 신규 — Tracer 부착 | observability 통합 |
| `AgentBuilder::cost_tracker()` | 신규 — CostTracker 부착 | " |
| `CapabilitySet` | 신규 — coding/read_only/research/all/browser preset | CSpace와 다른 모델 |
| `Authorizer` | 신규 — grant/check/revoke | access_manager와 보완적 |

### 2.4 변하지 않은 것 (Backward Compatible)

0.24.0의 **모든 공개 API가 그대로 유지**된다. Breaking change 없음.

- `Oxi`, `OxiBuilder` — 시그니처 동일
- `Agent`, `AgentConfig`, `AgentEvent` — 동일
- `ToolRegistry`, `AgentTool`, `AgentToolResult`, `ToolContext` — 동일
- `ProviderResolver` — 동일
- `MiddlewarePipeline`, `Middleware` trait — 동일
- `ClosureTool` — 동일
- `KernelToolProvider`, `KernelToolContext` — 동일
- `RoutingControl` — 동일
- 모든 oxi-ai re-export (`Provider`, `Model`, `ProviderEvent`, etc.) — 동일

---

## 3. Oxios와의 중복 분석

### 3.1 중복 매트릭스

```
oxios-kernel (0.24 기준)            oxi-sdk 0.26 (dormant 포함)         중복도
──────────────────────────────────   ────────────────────────────────────  ──────
audit_trail.rs (1134줄)         ←→  observability/audit_trail.rs (973줄)  ⚠️ 95%
event_bus.rs (595줄)            ←→  event_bus.rs (175줄)                  ⚠️ 70%
supervisor.rs + AgentPool (676) ←→  lifecycle/supervisor.rs (1060줄)      ⚠️ 40%
                                   + lifecycle/agent_pool.rs (139줄)
access_manager/ (3681줄)        ←→  security/ (active 1161 + dormant     ⚠️ 30%
                                       1536 = 2697줄)
capability/ (958줄)             ←→  security/capability.rs (570줄)       ⚠️ 20%
observability.rs (143줄)        ←→  observability/ (active ~1269줄)       ✅ 사용 중
```

### 3.2 중복의 성격

**A. 동일 코드 마이그레이션 (거의 복사-페스트)**

`audit_trail.rs`는 oxios에서 oxi-sdk로 **그대로 복사**된 코드다.
oxios의 원본과 SDK의 복사본이 95% 동일하다. 유일한 차이:
- oxios: `use crate::state_store::StateStore` (직접 의존)
- SDK: `pub trait AuditPersistence` (추상화)

→ oxios는 자체 `audit_trail.rs`를 삭제하고 SDK에서 re-export.

**B. 유사하지만 다른 구현**

`EventBus`는 oxios가 595줄(20개 KernelEvent variant 포함)이고 SDK는 175줄(제네릭).
oxios의 `KernelEvent` enum은 SDK에 없는 oxios 전용 variant(SeedCreated, PhaseStarted 등)를 포함한다.

→ oxios의 `KernelEvent` 정의는 유지, `EventBus<E>` 컨테이너는 SDK에서 가져옴.

**C. 다른 철학, 보완적 관계**

`access_manager/` vs `security/`는 근본적으로 다른 접근:
- oxios: **seL4 Capability 스타일** (Rights 비트플래그, CSpace, 4-layer gate)
- SDK: **POSIX Capability 스타일** (Capability enum, CapabilitySet preset)

→ 두 시스템을 **계층화**하여 사용. CSpace가 1차 필터, SDK Capability가 2차 필터.

### 3.3 의존성 그래프 영향

```
                         oxi-sdk 0.26.0
                    ┌────────────────────────┐
                    │  OxiBuilder → Oxi       │
                    │  AgentBuilder           │
                    │  EventBus<E> (dormant)  │
                    │  AgentPool (dormant)     │
                    │  AuditTrail (dormant)    │
                    │  Security (active+sleep) │
                    └──────────┬──────────────┘
                               │
         ┌─────────────────────┼───────────────────────┐
         │                     │                        │
    oxios-kernel          oxios-ouroboros          oxios-web
         │                     │                        │
    engine.rs             ouroboros_engine.rs      routes/
    agent_runtime.rs      (AgentConfig만 사용)
    supervisor.rs
    access_manager/
    capability/
    audit_trail.rs ←─ 제거 대상
    event_bus.rs   ←─ 간소화 대상
```

---

## 4. 마이그레이션 전략

### 4.1 원칙

1. **Backward compatibility first**: 0.24.0 API가 0.26.0에서 그대로 작동하므로
   Cargo.toml만 바꿔도 컴파일은 된다.
2. **Dormant 코드는 oxios에서 활성화**: oxi-sdk의 휴면 파일을 활성화하는 것은
   oxi 저장소에서 작업해야 한다. 이 RFC에서는 oxios 측의 변경만 다룬다.
3. **점진적 통합**: 한 번에 모든 중복을 제거하지 않는다. Phase별로 진행.
4. **더 강한 구현 유지**: 두 구현이 충돌하면 oxios의 것을 유지 (access_manager 등).

### 4.2 Phase 구성

| Phase | 내용 | 위험 | 예상 시간 | 선행 |
|-------|------|------|----------|------|
| **A** | Cargo.toml 업그레이드 | 낮음 | 10분 | 없음 |
| **B** | Observability 정리 (AuditLog 유지, 타입 정리) | 낮음 | 1시간 | A |
| **C** | EventBus 제네릭 전환 | 중간 | 2시간 | A |
| **D** | AgentBuilder 새 API 활용 (capabilities, authorizer, tracer) | 낮음 | 3시간 | A |
| **E** | AgentPool SDK 도입 | 중간 | 2시간 | A |
| **F** | audit_trail.rs 중복 제거 | 중간 | 3시간 | oxi-sdk에서 dormant 활성화 필요 |
| **G** | 최종 정리 (dead code, 불필요한 re-export) | 낮음 | 1시간 | B~F |

각 Phase는 독립 PR로 제출 가능하다.

---

## 5. Phase별 상세 설계

상세 설계는 별도 문서로 분리했다:

- **[Phase A: Cargo.toml 업그레이드](./rfc-014/phase-a-upgrade.md)** — 즉시 실행
- **[Phase B: Observability 정리](./rfc-014/phase-b-observability.md)** — 타입 정리
- **[Phase C: EventBus 제네릭 전환](./rfc-014/phase-c-eventbus.md)** — 커널 이벤트 버스 교체
- **[Phase D: AgentBuilder 새 API 활용](./rfc-014/phase-d-agentbuilder.md)** — 보안/관측 통합
- **[Phase E: AgentPool SDK 도입](./rfc-014/phase-e-agentpool.md)** — supervisor 간소화
- **[Phase F: AuditTrail 중복 제거](./rfc-014/phase-f-audit-trail.md)** — oxi-sdk 수정 포함
- **[Phase G: 최종 정리](./rfc-014/phase-g-cleanup.md)** — dead code 정리

---

## 6. 영향받는 파일 총정리

### 6.1 수정 파일

| 파일 | Phase | 변경 내용 |
|------|-------|----------|
| `Cargo.toml` (root) | A | `oxi-sdk = "0.26.0"` |
| `crates/oxios-kernel/src/observability.rs` | B | AuditLog→AuditTrail 전환 시 타입 수정 |
| `crates/oxios-kernel/src/event_bus.rs` | C | `EventBus<KernelEvent>`로 간소화 |
| `crates/oxios-kernel/src/agent_runtime.rs` | D | AgentBuilder 새 API 활용 |
| `crates/oxios-kernel/src/engine.rs` | D | OxiosEngine에 authorizer/tracer 통합 |
| `crates/oxios-kernel/src/supervisor.rs` | E | AgentPool을 SDK에서 가져오기 |

### 6.2 삭제 파일

| 파일 | Phase | 라인 | 사유 |
|------|-------|------|------|
| `crates/oxios-kernel/src/audit_trail.rs` | F | ~1134 | SDK에서 re-export |

### 6.3 간소화 파일 (라인 절감)

| 파일 | Phase | 전 | 후 | 절감 |
|------|-------|---|---|------|
| `event_bus.rs` | C | 595 | ~120 | -475 |
| `observability.rs` | B | 143 | ~100 | -43 |

### 6.4 영향받는 참조 파일 (import 경로 변경)

| 파일 | 변경 |
|------|------|
| `access_manager/audit_sink.rs` | `use crate::audit_trail::*` → `use oxi_sdk::...` |
| `kernel_handle/security_api.rs` | 동일 |
| `tools/kernel/security_tool.rs` | 동일 |
| `kernel_handle/mod.rs` | `use crate::event_bus::*` → `use oxi_sdk::EventBus` |
| `kernel_handle/agent_api.rs` | 동일 |
| `kernel_handle/infra_api.rs` | 동일 |
| `orchestrator.rs` | 동일 |
| `agent_lifecycle.rs` | 동일 |
| `a2a/mod.rs` | 동일 |
| `project/manager.rs` | 동일 |
| `tools/a2a_tools.rs` | 동일 |

---

## 7. Phase A 결과 (2026-06-03)

Phase A를 실행했다:

```
oxios/Cargo.toml: oxi-sdk = "0.24.0" → "0.26.2"
```

**0.26.2는 oxi의 0.26.1 dorman 모듈 활성화 작업을 포함한다:**

- `event_bus` → `lib.rs`에 등록
- `lifecycle/agent_pool` → 등록 + `export_state()`/`import_state()` 실구현
- `observability/audit_trail` → 등록 (blake3, chrono 의존성 추가)
- `security/capability` → 디렉토리 전환 (mod.rs, types.rs, resolve.rs)
- `security/{audit_sink, context, exec_policy, gate, permissions, rbac}` → 등록 (glob 의존성 추가)
- `lib.rs` re-export 업데이트

**oxios 측 결과:**
- `cargo build -p oxios-kernel` ✅ 성공 (경고 5개, 기존과 동일)
- `cargo test --workspace` ✅ 전체 통과 (0 failed)
- Breaking change: `ExecutionResult`에 `tool_calls` 필드 추가 → 테스트 4개 업데이트
- doctest 경로 수정: `a2a_circuit_breaker` → `a2a::circuit_breaker`, `coordination`/`clawhub`/`skills_sh`의 `no_run`을 `ignore`로 변경
- 커밋: `12c9990`

**Phase B~G는 다음 마일스톤으로 연기. Phase A (0.26.2 업그레이드) 완료로 의존성 해소 완료.**

## 8. 완료된 Phase

| Phase | 커밋 | 변경 파일 | 효과 |
|-------|------|----------|------|
| **A** (Upgrade) | 12c9990 | 11 | oxi-sdk 0.24.0 → 0.26.2 |
| **B** (Observability) | 71a5301 | 1 | re-export 그룹화, 문서화 |
| **D** (AgentBuilder) | 5c96a00 | 2 | authorizer/tracer/cost_tracker engine 통합 |
| **F** (AuditTrail) | a1ee2ad | 11 | **-1104줄** (audit_trail 중복 제거) |
| hotfix | 4aace84 | 3 | binary crate 및 테스트 적응 |

**상쇄**: Phase F에서 audit_trail.rs (1134줄) 삭제, audit_persistence.rs (30줄) 추가.
-1134 + 30 = **-1104줄 순감** (95% 중복 제거).

## 9. 남은 Phase

- **Phase C** (EventBus 재작성) — 10파일 임팩트. SDK의 `EventBus<E>`로 전환.
- **Phase E** (AgentPool SDK 도입) — supervisor.rs 단일 파일.
- **Phase G** (Cleanup) — 미사용 import, dead code 정리.

## 10. 위험 및 완화

| 위험 | 가능성 | 영향 | 완화 |
|------|--------|------|------|
| SDK dormant 코드 미활성화로 Phase F 지연 | 높음 | 중간 | Phase A~E는 독립 진행 가능 |
| EventBus 전환 시 KernelEvent 직렬화 불일치 | 낮음 | 높음 | 컴파일 타임에 확인됨 |
| Security 계층 충돌 (CSpace vs Capability) | 중간 | 높음 | Phase D는 additive only, 기존 코드 변경 없음 |
| AgentPool 교체 시 동시성 버그 | 낮음 | 높음 | 기존 테스트로 커버 |

---

## 8. 성공 기준

1. `cargo test --workspace` 통과 (모든 Phase 후)
2. `oxios run --json "test prompt"` 정상 동작
3. audit_trail.rs 중복 제거로 ~1100줄 절감
4. event_bus.rs 간소화로 ~475줄 절감
5. AgentBuilder에 capability/authorizer/tracer 통합 완료
