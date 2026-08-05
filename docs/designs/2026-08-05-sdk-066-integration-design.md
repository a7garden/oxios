# SDK 0.66.0 통합 설계

> **Date:** 2026-08-05
> **Feature:** Oxicode-sdk 0.66.0 신규 기능 통합 (Router, Hooks, StreamDelta)
> **Status:** Approved

## 1. 개요

Oxicode-sdk 0.66.0은 이미 workspace 의존성으로 포함되어 있다. SDK가 제공하는
신규 기능을 Oxios에 통합한다. 각 기능은 서로 독립적이며, 개별 구현이 가능하다.

**Shake compaction** (툴 결과/코드 블록 elide)은 SDK agent loop에서 이미 사용 중 —
`agent_loop/mod.rs:1304`에서 `compaction::shake::shake()` 호출. 별도 통합 불필요.

**SnapcompactCompactor** (PNG 기반 compaction)는 SDK API 갭으로 인해 현재 통합 불가능.
`AgentBuilder`에 `with_compactor()`가 없어 `SnapcompactCompactor`를 주입할 수 없다.
SDK 변경 완료 후 별도 통합.

**원칙:**
- SDK가 제공하는 구현은 그대로 사용한다. 재발명하지 않는다.
- Oxios의 역할은 설정 바인딩(config.toml) + Web UI + KernelHandle 통합으로 제한한다.
- SDK의 trait 기반 설계를 존중: port trait 구현은 Oxios에서, middleware/agent plumbing은 SDK가 처리.

## 2. 통합 아키텍처

```
                         config.toml
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
        [engine.router]  [[hooks]]    (compaction:
              │              │          shake 이미 통합,
              ▼              ▼          snapcompact 보류)
    ┌──────────────────────────────────────────────────┐
    │              OxiosEngine::build()                │
    │                                                  │
    │  // 1. Oxicode build                             │
    │  let oxi = OxicodeBuilder::new()                 │
    │    .with_builtins()                              │
    │    .with_catalog(catalog)                        │
    │    .with_hooks(hook_runner)    // port 주입       │
    │    .build();                                     │
    │                                                  │
    │  // 2. RouterProvider를 ProviderRegistry에 등록   │
    │  let registry = oxi.providers_arc();             │
    │  let router = RouterProvider::new(cfg, registry); │
    │  oxi.providers().register_arc("router", router); │
    │                                                  │
    │  // 3. Router profile 모델을 ModelRegistry에 등록 │
    │  for profile in &["auto", "budget", "perf"] {    │
    │    oxi.models().register(Model {                 │
    │      provider: "router", id: profile, ..          │
    │    });                                           │
    │  }                                               │
    └─────────────────────┬────────────────────────────┘
                          │
         ┌────────────────┼────────────────┐
         ▼                ▼                ▼
    RouterProvider    HookMiddleware    Shake compaction
    (oxi registry로    (SDK가 자동       (already integrated,
     resolve)          agent에 주입)     no change needed)
```

## 3. Router

### 3.1 현재 상태

- `OxiosEngine`에 `routing_control: Option<RoutingControl>` 필드 존재 (SDK의 간단한 폴백 라우팅)
- `config.toml`에 `routing_enabled`, `fallback_models` 등 기초 설정만 있음
- 설계 문서(`docs/designs/2026-05-17-multi-model-router-design.md`)는 outdated — SDK가 이미 더 정교한 라우터 제공

### 3.2 SDK 제공 기능

`oxicode_ai::router` 모듈 (`#[cfg(feature = "router")]`):
- **RouterProvider** — `Provider` trait 구현체. provider 이름 `"router"`, 모델 ID `"router/auto"`, `"router/budget"` 등
- **Classifier** — 2-stage 분류기: 휴리스틱(구조적 신호: 코드블록, 파일경로, 심볼 밀도, 라인 수) + LLM(ambiguous zone에서만)
- **RouterPipeline** — 세션별 상태 관리 (tier, history, accumulated_cost)
- **RouterProfiles** — tier별 모델 매핑 (fast/balanced/strong)
- **Signals + Scoring** — 가중치 기반 점수 산출 → RouterTier 결정
- **Fallback** — provider/model 실패 시 순차 폴백
- **Vision awareness** — 이미지 포함 시 자동으로 vision-capable 모델로 승격

### 3.3 Oxios 통합

**통합 흐름:**
1. `Cargo.toml` — `oxicode-sdk` features에 `"router"` 추가
2. `OxiosEngineBuilder` — router config 필드 추가
3. `OxiosEngine::build()`:
   - Oxicode build 완료 → `providers_arc()` 획득
   - `RouterProvider::new(&router_config, registry)` — instance-based (global registry 사용 안 함)
   - `oxi.providers().register_arc("router", Arc::new(router))` — ProviderRegistry에 post-build 등록
   - **각 profile에 대해 `oxi.models().register(Model { provider: "router", id: "auto", .. })` — ModelRegistry 등록**
     - 이것이 없으면 `AgentBuilder::build()` 내부의 `resolve_model("router/auto")`가 `ModelNotFound`로 실패
4. `AgentRuntime::execute_directive()` — `model_id = "router/auto"`로 resolve

**ModelRegistry 등록 (신규 — 리뷰에서 발견된 갭):**
```rust
// Synthetic models for router profiles. 실제 과금/제한은
// delegate provider가 처리하므로 price/limit은 의미 없음.
for (id, display) in &[
    ("auto", "Router Auto"),
    ("budget", "Router Budget"),
    ("performance", "Router Performance"),
] {
    oxi.models().register(Model {
        provider: "router".into(),
        id: id.to_string(),
        display_name: display.to_string(),
        supports_vision: true,   // router 내부에서 vision-aware 처리
        ..Default::default()
    });
}
```

**Credential 일관성:**
`RouterProvider::resolve_provider()`는 `ProviderRegistry::get()`을 호출한다.
`get()`은 custom → builtin fallback 순서로 resolve.
Built-in provider (anthropic, openai 등)는 SDK의 기본 credential resolution (env var → auth.json)을 그대로 사용.
`OxicodeBuilder::api_key()`로 주입된 오버라이드는 router resolve 경로에서 사용되지 않지만,
이는 예외적인 케이스이며 실사용 credential은 env var/auth.json으로 충당된다.

### 3.4 config.toml 스키마

```toml
[engine.router]
# true면 default_model이 "router/<default_profile>"로 설정됨
enabled = false
# 기본 프로파일: "auto" | "budget" | "performance"
default_profile = "auto"
# classifier_model이 설정되면 ambiguous case에 LLM 분류 사용 (없으면 휴리스틱 only)
# classifier_model = "anthropic/claude-haiku-4-20250514"
# 세션 최대 예산 (USD, null = unlimited)
# max_session_budget = 5.0
# context token이 이 값을 넘으면 strong tier로 강제 승격
# context_upgrade_threshold = 50000

[engine.router.scoring]
# structural = 0.25   # 코드 구조 신호
# behavioral = 0.20   # tool 사용 패턴
# context = 0.15      # 컨텍스트 크기
# vision = 0.10       # 이미지 포함 여부
# message = 0.30      # 메시지 내용

[engine.router.profiles.auto.tiers]
# RouterTier enum → toml table key. "fast" | "balanced" | "strong"
fast = { model = "anthropic/claude-haiku-4-20250514" }
balanced = { model = "anthropic/claude-sonnet-4-20250514", thinking = { budget = 4000 } }
strong = { model = "anthropic/claude-opus-4-20250514", thinking = { budget = 16000 } }
```

### 3.5 코드 변경 지점

| 파일 | 변경 |
|---|---|
| `Cargo.toml` | `oxicode-sdk` features에 `"router"` 추가 |
| `crates/oxios-kernel/src/engine.rs` | RouterConfig 역직렬화, build()에서 router provider + model 등록 |
| `crates/oxios-kernel/src/agent_runtime.rs` | router enabled 시 model_id를 `"router/<profile>"`로 설정 |
| `share/default-config.toml` | `[engine.router]` 섹션 (기본값 비활성화) |
| Web UI | 모델 선택기에 router 프로파일 노출, 엔진 설정 페이지 |

---

## 4. Hooks

### 4.1 SDK vs Oxios 개념 분리

| | SDK Hook | Oxios PersistenceHook |
|---|---|---|
| **트리거** | PreToolUse, PostToolUse, SessionStart, Stop, Notification 등 lifecycle event | 에이전트 실행 완료 후 |
| **실행 주체** | 사용자 정의 shell command | oxios 커널 코드 |
| **용도** | "tool 실행 전에 검증", "파일 변경 시 lint 돌리기" | "에이전트가 만든 문서를 지식창고에 저장" |
| **통합 지점** | `HookMiddleware` → `MiddlewarePipeline` (SDK가 자동 주입) | `AgentRuntime::execute_directive` 종료 후 |

**둘 다 유지한다.** 충돌하지 않는다. 이름이 비슷할 뿐 완전히 다른 시스템.

### 4.2 SDK 제공 기능

`oxicode_sdk::ports::hooks`:
- **HookRunner** — trait. `run(event, context) → HookOutcome` (block 여부 결정)
- **HookEvent** — PreToolUse, PostToolUse, SessionStart, SessionEnd, Stop, Notification, SubagentStop
- **HookSpec** — 사용자 설정: event, matcher(regex), command, timeout
- **HookMiddleware** — `Middleware` trait 구현체. `HookRunner`를 middleware pipeline에 연결
- **NoopHookRunner** — 기본값 (아무것도 안 함)

**통합 방식:**
- `OxicodeBuilder::with_hooks(runner: Arc<dyn HookRunner>)` — builder에 port 주입 (SDK API, 확인 완료: `builder.rs:570`)
- SDK가 `Agent` 생성 시 자동으로 `HookMiddleware`를 pipeline에 삽입 — Oxios가 수동으로 per-agent 호출할 필요 없음

### 4.3 Oxios 통합

1. `CommandHookRunner` — `HookRunner` trait을 구현하는 struct.
   - Oxios config에서 `[[hooks]]` 배열을 읽어 `Vec<HookSpec>` 보관
   - `run(event, ctx)` 호출 시 매칭되는 spec의 command를 shell 실행 (timeout 적용)
2. `OxiosEngineBuilder`:
   - hook_specs를 받아 `Arc::new(CommandHookRunner::new(specs))` 생성
   - `OxicodeBuilder::with_hooks(runner)` 호출
3. Agent 실행 시: SDK가 자동으로 `HookMiddleware`를 pipeline에 삽입 — **Oxios 코드 변경 불필요**

### 4.4 config.toml 스키마

```toml
# SDK lifecycle hooks (Claude Code 호환 스키마)
# [[hooks]]
# event = "PreToolUse"
# matcher = "Bash|Write"
# command = "echo 'Tool $OXICODE_TOOL_NAME starting' >> ~/.oxios/logs/hooks.log"
# timeout_secs = 10

# [[hooks]]
# event = "PostToolUse"
# matcher = ""
# command = "echo 'Tool completed' >> ~/.oxios/logs/hooks.log"
```

### 4.5 코드 변경 지점

| 파일 | 변경 |
|---|---|
| `crates/oxios-kernel/src/hook_runner.rs` | 신규: `CommandHookRunner` struct, `HookRunner` trait 구현 |
| `crates/oxios-kernel/src/engine.rs` | OxiosEngineBuilder에 hook_specs 필드 추가, build 시 with_hooks 호출 |
| `crates/oxios-kernel/src/lib.rs` | `CommandHookRunner` pub export |
| `share/default-config.toml` | `[[hooks]]` 예시 (주석 처리) |

---

## 5. Compaction

### 5.1 Shake compaction — already integrated

SDK의 `compaction::shake` 모듈은 LLM 호출 없이 큰 tool result와 code block을 elide한다.
Oxios의 agent loop (`agent_loop/mod.rs:1304`)에서 이미 `shake()`를 호출 중.
**추가 통합 작업 불필요.**

### 5.2 SnapcompactCompactor — 보류 (SDK API 갭)

`SnapcompactCompactor`는 대화 tail을 PNG 프레임으로 렌더링하는 compactor.
`CompactionStrategy::Snapcompact` variant + `SnapcompactCompactor` builder 존재.

**통합 불가능한 이유:**
- `Agent::build_inner()` (private)는 strategy가 `Disabled`가 아니면 무조건 `LlmCompactor` 생성
- `CompactionManager::set_compactor(&mut self, ...)`는 존재하지만 `Agent`가 `&self`만 노출
- `AgentBuilder`에 `with_compactor()` 메서드 없음

SDK에 `AgentBuilder::with_compactor()` 또는 `AgentConfig.compactor` 필드가 추가되면 통합 가능.
그전까지는 `CompactionStrategy::Threshold(0.8)` (LLM compaction) 유지.

---

## 6. StreamDelta

### 6.1 현재 상태

Oxios는 자체 `StreamDelta` enum을 `agent_runtime.rs`에 정의:

```rust
pub enum StreamDelta {
    Model(String),          // 모델 announcement (한 번)
    Text(String),           // 텍스트 청크
    Thinking,               // 추론 시작 (signal-only)
    ThinkingDelta(String),  // 추론 텍스트 (batched, ~50ms)
    ThinkingEnd,            // 추론 종료 (signal-only)
}
```

SDK의 `oxicode_agent::events::StreamDelta`:

```rust
pub enum StreamDelta {
    Text(String),
    Thinking(String),
    Sync,
}
```

### 6.2 설계 결정: 유지 + 래핑

Oxios enum을 **유지**한다. 이유:
- `Model` variant → Web UI가 실제 사용된 모델을 표시
- `Thinking` (signal) + `ThinkingEnd` → 추론 패널의 시작/종료 + gateway의 `reasoning.end` 마커
- SDK enum에는 이 개념이 없음

**마이그레이션 방식:**
1. SDK `AgentEvent::StreamingDelta { delta, message }`를 이벤트 콜백에서 수신
2. SDK `StreamDelta`를 oxios `StreamDelta`로 변환
3. **중복 방지:** `StreamingDelta`와 기존 `TextChunk`/`ThinkingDelta`가 모두 emit될 경우,
   `StreamingDelta`를 우선 사용하고 기존 분기를 `#[allow(unreachable)]` 또는 flag로 skip.
   구현 시 실제 emit 패턴을 확인하여 결정.

```rust
// agent_runtime.rs 이벤트 콜백 — SDK StreamDelta → oxios StreamDelta 변환
AgentEvent::StreamingDelta { delta, message, .. } => {
    match delta {
        sdk::StreamDelta::Text(text) => {
            let _ = tx.try_send(StreamDelta::Text(text));
        }
        sdk::StreamDelta::Thinking(text) if !text.is_empty() => {
            if !thinking_active {
                let _ = tx.try_send(StreamDelta::Thinking);
                thinking_active = true;
            }
            let _ = tx.try_send(StreamDelta::ThinkingDelta(text));
        }
        sdk::StreamDelta::Sync => {
            if thinking_active {
                let _ = tx.try_send(StreamDelta::ThinkingEnd);
                thinking_active = false;
            }
        }
    }
}
```

### 6.3 영향 분석

- `StreamingSinkTx`, `StreamingSinkRegistry` — 타입 변경 없음
- Gateway collector의 `reasoning.end` 마커 — 로직 변경 없음
- Web UI 추론 패널 — 영향 없음
- `agent_runtime.rs` 이벤트 콜백 내부만 변경

### 6.4 코드 변경 지점

| 파일 | 변경 |
|---|---|
| `crates/oxios-kernel/src/agent_runtime.rs` | 이벤트 콜백에 `AgentEvent::StreamingDelta` 분기 추가, 변환 로직 |

---

## 7. 우선순위 및 의존성

세 기능은 모두 서로 독립적. Router가 사용자 체감 임팩트가 가장 크므로 1순위.

| 순서 | 작업 | 의존성 | 영향 범위 |
|---|---|---|---|
| 1 | **Router** | 없음 | config, engine, agent_runtime, web UI |
| 2 | **StreamDelta** | 없음 | agent_runtime.rs 이벤트 콜백 |
| 3 | **Hooks** | 없음 | 신규 파일 + engine builder |

---

## 8. 검증 계획

### Router
- `cargo build -p oxios --features router` 통과
- TOML deserialization: `HashMap<RouterTier, RoutedTierConfig>` enum 키 파싱 확인
- `cargo test -p oxios-kernel` 통과
- Smoke test: `oxios run --json "hello"` → router/auto resolve → 정상 응답
- ModelRegistry에 router profile 모델이 등록되었는지 단위 테스트

### Hooks
- `cargo test -p oxios-kernel` — CommandHookRunner 단위 테스트
- 통합: `[[hooks]]` 설정 후 agent 실행 → hook 로그 파일 확인

### StreamDelta
- 기존 streaming test 통과 (`streaming_sink.rs` 테스트)
- `StreamingDelta`와 `TextChunk`/`ThinkingDelta` 중복 emit 여부 확인 → dedup
- Web UI에서 추론 패널 정상 동작 확인
