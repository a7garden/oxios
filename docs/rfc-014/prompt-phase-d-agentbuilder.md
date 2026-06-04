# Phase D: AgentBuilder 새 API 활용 — Subagent 프롬프트

---

## 컨텍스트

oxios-kernel은 oxi-sdk 0.26.2를 사용한다. Phase A(0.24.0→0.26.2 업그레이드)는 완료됐다.
이제 RFC-014에 따라 Phase D를 독립적으로 진행한다.

**작업 디렉토리**: `/Volumes/MERCURY/PROJECTS/oxios`
**대상 crate**: `oxios-kernel`
**RFC 문서**: `/Volumes/MERCURY/PROJECTS/oxios/docs/rfc-014/phase-d-agentbuilder.md`
**메인 RFC**: `/Volumes/MERCURY/PROJECTS/oxios/docs/rfc-014-oxi-sdk-0.26-migration.md`

---

## 진행 방식: Git Worktree 격리 (필수)

이 Phase와 다른 Phase (B, F) 가 동시에 진행되므로, **반드시 별도 worktree에서 작업해야 한다**.

```bash
# 1. 메인 작업 트리에서 시작
cd /Volumes/MERCURY/PROJECTS/oxios

# 2. 깨끗한 main에서 새 worktree 생성
git worktree add ../oxios-phase-d-agentbuilder -b phase/d-agentbuilder main

# 3. worktree로 이동
cd ../oxios-phase-d-agentbuilder

# 4. 이후 모든 작업은 여기서 진행
```

**절대 main에서 직접 작업하지 말 것.** 작업 완료 후 커밋된 브랜치만 메인으로 머지된다.

---

## 작업 내용

### 1. 사전 조사

oxi-sdk 0.26.2의 `AgentBuilder` API를 정확히 파악:

```bash
SDK=/Users/won/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/oxi-sdk-0.26.2

echo "═══ AgentBuilder 메서드 목록 ═══"
grep -E "pub fn [a-z]" $SDK/src/agent_builder.rs | grep -v "pub fn default\|pub fn new" | head -30

echo ""
echo "═══ AgentBuilder::new /::build 위치 ═══"
grep -n "pub fn new\|pub fn build\|impl AgentBuilder" $SDK/src/agent_builder.rs | head -20

echo ""
echo "═══ Authorizer, Tracer, CostTracker 타입 ═══"
grep -E "^pub struct (Authorizer|Tracer|CostTracker)" \
  $SDK/src/security/authorizer.rs \
  $SDK/src/observability/trace.rs \
  $SDK/src/observability/cost.rs

echo ""
echo "═══ Oxi::agent() 반환 타입 ═══"
grep -n "pub fn agent" $SDK/src/builder.rs
```

현재 oxios의 `engine.rs`와 `agent_runtime.rs`를 읽고:

```bash
# OxiosEngine 구조
sed -n '1,40p' crates/oxios-kernel/src/engine.rs

# OxiosEngineBuilder 구조
grep -A20 "pub struct OxiosEngineBuilder" crates/oxios-kernel/src/engine.rs | head -25

# run_agent() 위치
grep -n "pub async fn run_agent\|fn new_with_resolver" crates/oxios-kernel/src/agent_runtime.rs
```

### 2. 작업 단계

RFC-014/phase-d-agentbuilder.md의 "변경 후" 섹션 그대로 진행:

#### Step 1. `OxiosEngine`에 authorizer/tracer/cost_tracker 필드 추가

```rust
// crates/oxios-kernel/src/engine.rs
pub struct OxiosEngine {
    oxi: Oxi,
    default_model_id: String,
    routing_control: Option<oxi_sdk::RoutingControl>,
    pools: ...,
    // ── 신규 ──
    authorizer: Option<Arc<oxi_sdk::Authorizer>>,
    tracer: Option<Arc<oxi_sdk::Tracer>>,
    cost_tracker: Option<Arc<oxi_sdk::CostTracker>>,
}
```

`OxiosEngineBuilder`에도 `with_authorizer`, `with_tracer`, `with_cost_tracker` 메서드 추가:

```rust
impl OxiosEngineBuilder {
    pub fn with_authorizer(mut self, auth: Arc<oxi_sdk::Authorizer>) -> Self {
        self.authorizer = Some(auth);
        self
    }
    pub fn with_tracer(mut self, tracer: Arc<oxi_sdk::Tracer>) -> Self {
        self.tracer = Some(tracer);
        self
    }
    pub fn with_cost_tracker(mut self, ct: Arc<oxi_sdk::CostTracker>) -> Self {
        self.cost_tracker = Some(ct);
        self
    }
}
```

**호환성**: 기존 필드는 모두 유지 (None 기본값). 이미 만들어진 OxiosEngine은 그대로 동작.

#### Step 2. `agent_runtime.rs`의 `run_agent()`를 AgentBuilder 사용으로 전환

현재 (0.26.2):
```rust
let agent = Arc::new(Agent::new_with_resolver(
    provider, agent_config, Arc::new(registry), resolver,
));
if !pipeline.is_empty() {
    let terminate_flag = Arc::new(AtomicBool::new(false));
    let hooks = oxi_sdk::middleware::build_hooks(pipeline, agent_id, terminate_flag);
    agent.set_hooks(hooks);
}
```

변경 후 (AgentBuilder 사용):

**주의**: 이 Phase의 핵심은 **"새 API로 전환한다"** 가 아니라
**"engine에 부착된 authorizer/tracer/cost_tracker를 agent에 전달한다"** 다.
Provider 생성은 AgentBuilder가 자동 처리한다. 미들웨어는 그대로 두거나
AgentBuilder의 `with_*` 메서드로 옮긴다.

```rust
let mut builder = engine.oxi().agent(agent_config)
    .workspace(&workspace)
    .system_prompt(system_prompt);

// CSpace 기반 도구 등록 (oxios 고유 — 그대로 유지)
register_tools_from_cspace_gated(&registry, ...);
// registry의 도구를 builder에 전달
for name in registry.names() {
    if let Some(tool) = registry.get(&name) {
        builder = builder.tool(tool.clone());
    }
}

// 0.26.0 새 API — 보안/관측 통합
if let Some(auth) = engine.authorizer() {
    builder = builder.authorizer(auth.clone());
}
if let Some(tracer) = engine.tracer() {
    builder = builder.tracer(tracer.clone());
}
if let Some(ct) = engine.cost_tracker() {
    builder = builder.cost_tracker(ct.clone());
}

let agent = builder.build()?;
```

**중요한 예외 케이스 — Provider 교체 문제**:

oxios의 `agent_runtime.rs`는 provider를 명시적으로 선택/교체하는 로직이 있을 수 있다.
`engine.oxi().agent(config)`가 default provider를 쓰는 것과 다를 수 있다.

확인할 것:
```bash
grep -n "provider\|ProviderResolver\|with_provider" crates/oxios-kernel/src/agent_runtime.rs | head -20
```

만약 명시적 provider 선택 로직이 있다면:
- `OxiBuilder`의 `.with_provider()` 등으로 전달하거나
- `AgentConfig`에 provider 정보를 담아 builder에 넘긴다.

이 부분이 가장 까다로운 디테일이므로, **Provider 관련 로직은 그대로 두는 것이 안전**할 수 있다.
대안으로, 이 Phase의 scope를 "engine의 authorizer/tracer/cost_tracker를 agent에 전달"로만 한정하고
provider 선택은 향후 단계로 미뤄도 된다.

#### Step 3. 미들웨어 호환성

`build_hooks()` 직접 호출을 AgentBuilder의 `with_rate_limit`/`with_token_budget`/`with_logging`으로 대체:

```rust
if config.rate_limit_per_minute > 0 {
    builder = builder.with_rate_limit(config.rate_limit_per_minute);
}
if config.token_budget > 0 {
    builder = builder.with_token_budget(config.token_budget);
}
if config.audit_tool_calls {
    builder = builder.with_logging();
}
```

`oxi_sdk::middleware::build_hooks`를 호출하는 코드가 더 이상 없으면 그 호출도 제거.

### 3. 변경 범위

| 파일 | 변경 |
|------|------|
| `crates/oxios-kernel/src/engine.rs` | `OxiosEngine`, `OxiosEngineBuilder` 필드 + 빌더 메서드 |
| `crates/oxios-kernel/src/agent_runtime.rs` | `run_agent()` builder 전환 |

다른 Phase와 파일 겹침:
- `agent_runtime.rs` — Phase F와 겹침. Phase F는 `use crate::audit_trail` → `use oxi_sdk::audit_trail`만 변경.
  머지 시 import 라인만 합치면 OK.

### 4. 핵심 안전장치

- **CSpace 기반 도구 등록은 그대로 유지**: `register_tools_from_cspace_gated()`는
  oxios 고유 로직. AgentBuilder의 `.coding_tools()`로 대체하지 말 것.
- **기존 빌드 호환성**: 기존 호출자 코드(`engine.builder()` 후 `.build()`)는
  모두 새 필드가 None이어도 동작해야 한다.
- **에러 발생 시 단계적으로**: 일단 builder 전환만 시도하고, 미들웨어는
  기존 `set_hooks` 호출을 유지해도 좋다. 점진적 전환.

---

## 검증

```bash
# 빌드 확인
cargo build -p oxios-kernel

# 테스트 확인
cargo test -p oxios-kernel

# 회귀 확인
cargo test --workspace

# 신규 필드 사용 (선택)
cargo test -p oxios-kernel --features engine-with-tracing  # feature가 있다면
```

기대 결과:
- `cargo build -p oxios-kernel`: 0 errors
- `cargo test --workspace`: 0 failed
- `cargo clippy -p oxios-kernel -- -D warnings`: 통과

---

## 커밋 형식

```bash
git add crates/oxios-kernel/src/engine.rs \
        crates/oxios-kernel/src/agent_runtime.rs

git commit -m "refactor(kernel): use AgentBuilder new API (RFC-014 Phase D)

- Add authorizer/tracer/cost_tracker fields to OxiosEngine
- Add with_authorizer/with_tracer/with_cost_tracker to OxiosEngineBuilder
- Convert run_agent() from Agent::new_with_resolver to AgentBuilder
- Attach engine's authorizer/tracer/cost_tracker via new API
- Move middleware from set_hooks() to builder.with_rate_limit/with_token_budget
- CSpace-based tool registration unchanged (oxios-specific)

Tests: cargo test --workspace passes (0 failed)
Backwards compatible: existing OxiosEngine::builder().build() works without new fields"
```

---

## 완료 보고

다음 정보를 출력:
1. `git log --oneline -3` 결과
2. `git diff main --stat` 결과
3. `cargo test --workspace` 마지막 5줄
4. 추가 / 발견한 사항 (특히 provider 선택 로직 관련)

---

## 주의사항

- **이 Phase는 additive change다**. 기존 OxiosEngine 인스턴스 생성 패턴은 깨지면 안 된다.
- **scope를 좁게 유지**: engine 필드 추가 + agent_runtime의 builder 전환이 핵심.
  너무 많이 건드려서 테스트가 깨지면 머지가 힘들다.
- **provider 선택 로직은 보존**: engine.oxi().agent(config)가 default provider를
  쓰는 것과 다를 수 있으니, 명시적 provider 선택 코드는 건드리지 않는 게 안전.
- **worktree 사용 절대 필수**. Phase F와 `agent_runtime.rs`가 겹치므로
  worktree 격리 없이는 동시 진행 불가.
