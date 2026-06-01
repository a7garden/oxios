# RFC-012: Engine Hot-Swap — OxiosEngine ↔ EngineApi 통합

> **상태**: Draft
> **날짜**: 2026-06-01
> **영역**: kernel, agent_runtime, web

---

## 1. 문제 요약

Web UI / CLI에서 프로바이더·모델·API 키를 변경해도 **런타임 엔진에 반영되지 않는다.**
커널 부팅 시 생성된 `OxiosEngine` 인스턴스가 daemon 수명 동안 불변으로 고정되기 때문이다.

### 1.1 증상

| 동작 | 기대 | 실제 |
|------|------|------|
| Web UI에서 모델을 `openai/gpt-4o`로 변경 | 다음 agent 실행부터 gpt-4o 사용 | 여전히 부팅 시 모델 사용 |
| Web UI에서 API 키 갱신 | 새 키로 즉시 인증 | 이전 키로 실패 (401) |
| CLI `oxios run` 으로 모델 변경 후 실행 | 변경된 모델 사용 | config.toml만 업데이트, 런타임 무시 |
| 프로바이더 전환 (anthropic → google) | 프로바이더 전환 즉시 동작 | daemon 재시작 필요 |

### 1.2 근원 원인

```
┌─────────────────────────────────────────────────────────┐
│  kernel.rs boot                                          │
│                                                          │
│  let engine = Arc::new(OxiosEngine::from_config(...));   │
│       │                │                                 │
│       │  ┌─────────────┴──────────────┐                  │
│       │  │                            │                  │
│       ▼  ▼                            ▼                  │
│  AgentRuntime                    BasicSupervisor          │
│  (engine: Arc<OxiosEngine>)      (runtime: Arc<AgentRuntime>)
│       │                            │                     │
│       │  execute()마다 engine 사용  │  run_with_seed()    │
│       ▼                            ▼                     │
│  engine.resolve_model()    runtime.execute()             │
│  engine.create_provider()         │                     │
│  engine.oxi()                     ▼                     │
│       │                     항상 같은 OxiosEngine        │
│       ▼                     항상 같은 Oxi 인스턴스       │
│  ❌ 불변! 부팅 시 값으로 영구 고정                        │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│  EngineApi (Web UI가 호출)                                │
│                                                          │
│  set_model()     → config.toml 업데이트 ✅                │
│  set_api_key()   → auth.json + config.toml ✅             │
│  providers()     → oxi_sdk::get_providers() ✅            │
│  models()        → oxi_sdk::get_provider_models() ✅      │
│                                                          │
│  → OxiosEngine에 대한 참조가 없다 ❌                       │
│  → AgentRuntime에 대한 참조가 없다 ❌                      │
│  → "쓰기는 성공하지만 아무도 모른다"                       │
└─────────────────────────────────────────────────────────┘
```

`EngineApi`는 config 영속성만 담당하고, `OxiosEngine`은 런타임만 담당한다.
**두 구조체 사이에 어떤 통신 채널도 없다.**

### 1.3 부가 문제

| # | 문제 | 위치 |
|---|------|------|
| A | `_engine_api` 인스턴스 생성 후 즉시 폐기 | `src/kernel.rs:768` |
| B | `OxiosEngine` vs `EngineApi` 역할 중복·혼재 | `engine.rs` / `engine_api.rs` |
| C | `AgentRuntime`이 `Arc<OxiosEngine>`을 불변으로 보관 | `agent_runtime.rs:138` |
| D | `BasicSupervisor`가 `Arc<AgentRuntime>`을 불변으로 보관 | `supervisor.rs:142` |

---

## 2. 설계 목표

1. **즉시 반영**: Web UI / CLI에서 모델·키 변경 시 **다음 agent 실행부터** 새 설정 사용
2. **재시작 불필요**: daemon 실행 중 설정 변경이 즉시 유효
3. **원자적 교체**: 모델 + 키 + provider_options가 일관되게 함께 교체
4. **최소 변경**: 기존 퍼블릭 API 시그니처 최대한 유지
5. **의존성 추가 없음**: `parking_lot::RwLock` 사용 (이미 의존)

---

## 3. 핵심 설계: `EngineHandle`

### 3.1 개념

```
              ┌──────────────────────────────────┐
              │  EngineHandle                     │
              │  (Arc<RwLock<Arc<OxiosEngine>>>)  │
              │                                    │
              │  ┌────────────┐  swap()  ┌──────┐ │
              │  │ EngineApi   │ ──────→  │ 새   │ │
              │  │ (write-side)│          │ 엔진 │ │
              │  └────────────┘          └──────┘ │
              │                                    │
              │  ┌────────────┐  read()  ┌──────┐ │
              │  │ AgentRuntime│ ←────── │현재  │ │
              │  │ (read-side) │         │ 엔진 │ │
              │  └────────────┘         └──────┘ │
              └──────────────────────────────────┘
```

`EngineHandle`은 `OxiosEngine` 인스턴스에 대한 **원자적 교체 가능 참조**다.
`EngineApi`는 쓰기 측(설정 변경 → 엔진 리빌드 → swap), `AgentRuntime`은 읽기 측(매 실행마다 최신 엔진 획득).

### 3.2 새 타입 정의

**`crates/oxios-kernel/src/engine.rs`**에 추가:

```rust
/// Shared, hot-swappable reference to the active `OxiosEngine`.
///
/// Wraps `RwLock<Arc<OxiosEngine>>` so that:
/// - **Writers** (EngineApi) can atomically replace the engine on config change
/// - **Readers** (AgentRuntime) always get the current engine at execution time
///
/// # Cost
///
/// Rebuilding `OxiosEngine` is cheap: `OxiBuilder::new().with_builtins().build()`
/// populates registries from static `model_db` data (~1μs, no I/O, no network).
///
/// # Concurrency
///
/// - `parking_lot::RwLock` is not async-aware, but engine swap only occurs on
///   explicit user action (Web UI / CLI config change) — never in a hot path.
/// - Agent execution reads the engine once at the start of `execute()` and
///   uses the same `Arc<OxiosEngine>` for the entire run (consistent within one execution).
pub struct EngineHandle {
    inner: parking_lot::RwLock<Arc<OxiosEngine>>,
}

impl EngineHandle {
    /// Create a new handle wrapping the given engine.
    pub fn new(engine: Arc<OxiosEngine>) -> Self {
        Self {
            inner: parking_lot::RwLock::new(engine),
        }
    }

    /// Get a snapshot of the current engine.
    ///
    /// The returned `Arc` is stable — it won't change even if another thread
    /// calls `swap()` concurrently.
    pub fn get(&self) -> Arc<OxiosEngine> {
        Arc::clone(&self.inner.read())
    }

    /// Atomically replace the engine with a new one.
    ///
    /// Callers should rebuild `OxiosEngine` with updated credentials/model
    /// before calling this.
    pub fn swap(&self, new_engine: OxiosEngine) {
        let mut guard = self.inner.write();
        let old_id = guard.default_model_id().to_string();
        *guard = Arc::new(new_engine);
        tracing::info!(
            old_model = %old_id,
            new_model = %guard.default_model_id(),
            "Engine hot-swapped"
        );
    }
}
```

### 3.3 `EngineApi`에 `EngineHandle` 참조 추가

**`crates/oxios-kernel/src/kernel_handle/engine_api.rs`** 수정:

```rust
pub struct EngineApi {
    config: Arc<RwLock<OxiosConfig>>,
    config_path: PathBuf,
    routing_stats: Arc<RoutingStats>,
    /// Hot-swap handle — when config writes change model/key,
    /// we rebuild OxiosEngine and swap it in.
    engine_handle: Arc<EngineHandle>,
}
```

`set_model()` 변경:

```rust
pub fn set_model(&self, model_id: &str) -> anyhow::Result<()> {
    {
        let mut cfg = self.config.write();
        cfg.engine.default_model = model_id.to_string();
        self.persist(&cfg)?;
    }
    // Hot-swap: rebuild engine with new model
    self.rebuild_and_swap();
    Ok(())
}
```

`set_api_key()` 변경:

```rust
pub fn set_api_key(&self, provider: &str, key: &str) -> anyhow::Result<()> {
    CredentialStore::store(provider, key)?;
    // Update config.toml if provider matches current model
    let cfg = self.config.read();
    if let Some(current_provider) =
        CredentialStore::provider_from_model(&cfg.engine.default_model)
    {
        if current_provider == provider {
            drop(cfg);
            let mut cfg = self.config.write();
            cfg.engine.api_key = Some(key.to_string());
            self.persist(&cfg)?;
        }
    }
    // Hot-swap: rebuild engine with new credential
    self.rebuild_and_swap();
    Ok(())
}
```

새 private 메서드 `rebuild_and_swap()`:

```rust
/// Rebuild `OxiosEngine` from current config and swap into the handle.
///
/// This is cheap (~1μs): `OxiBuilder` populates registries from static data.
/// No network calls, no I/O beyond what `CredentialStore` already caches in memory.
fn rebuild_and_swap(&self) {
    let cfg = self.config.read();
    let model_id = &cfg.engine.default_model;
    let new_engine = OxiosEngine::from_config(model_id, cfg.api_key().as_deref());
    self.engine_handle.swap(new_engine);
}
```

### 3.4 `AgentRuntime`을 `EngineHandle` 기반으로 변경

**`crates/oxios-kernel/src/agent_runtime.rs`** 수정:

```rust
pub struct AgentRuntime {
-   engine: Arc<OxiosEngine>,
+   engine_handle: Arc<EngineHandle>,
    config: AgentRuntimeConfig,
    kernel_handle: Arc<KernelHandle>,
    // ...나머지 동일
}
```

`execute()` 내부 변경:

```rust
pub async fn execute(...) -> Result<ExecutionResult> {
    // 매 실행마다 최신 엔진 획득
+   let engine = self.engine_handle.get();
    // ... 이후 engine 사용은 동일
-   let _model = self.engine.resolve_model(&self.config.model_id)?;
+   let _model = engine.resolve_model(&self.config.model_id)?;
}
```

`run_agent()` 시그니처 변경:

```rust
async fn run_agent(
    config: &AgentRuntimeConfig,
-   engine: &OxiosEngine,
+   engine: &OxiosEngine,  // 그대로 — execute()에서 Arc를 넘김
    ...
)
```

`run_agent()` 호출부에서:

```rust
-   run_agent(&config, &self.engine, ...)
+   let engine = self.engine_handle.get();
+   run_agent(&config, &engine, ...)
```

### 3.5 `BasicSupervisor` 변경

`supervisor.rs`는 `Arc<AgentRuntime>`을 보관하므로 변경 없음.
`AgentRuntime` 내부에서 `engine_handle.get()`으로 매 실행마다 최신 엔진을 읽으므로,
supervisor는 신경 쓸 필요 없다.

---

## 4. `kernel.rs` 조립 변경

### 4.1 현재 (Before)

```rust
// ① OxiosEngine 생성 — 한 번 만들어지면 불변
let engine = Arc::new(OxiosEngine::from_config(model_id, config.engine.api_key.as_deref()));

// ② EngineApi 생성 — engine에 대한 참조 없음
let _engine_api = oxios_kernel::EngineApi::new(config_rwlock, config_path, routing_stats);

// ③ KernelHandle 안에 또 하나의 EngineApi — 역시 engine 참조 없음
let kernel_handle = Arc::new(oxios_kernel::KernelHandle::new(
    ...,
    oxios_kernel::EngineApi::new(config_rwlock, config_path, routing_stats),
    ...
));

// ④ AgentRuntime에 engine Arc 전달 — 불변
let agent_runtime = AgentRuntime::new(Arc::clone(&engine), model_id, kernel_handle.clone(), ...);
```

### 4.2 변경 후 (After)

```rust
// ① OxiosEngine 생성
let engine = Arc::new(OxiosEngine::from_config(model_id, config.engine.api_key.as_deref()));

// ② EngineHandle 생성 — engine을 wrapping
let engine_handle = Arc::new(oxios_kernel::EngineHandle::new(engine));

// ③ EngineApi에 engine_handle 전달 — 이제 쓰기가 engine에 도달함
let engine_api = oxios_kernel::EngineApi::new(
    config_rwlock,
    config_path,
    Arc::clone(&routing_stats),
    Arc::clone(&engine_handle),  // ← 추가
);

// ④ KernelHandle에 EngineApi 전달 (기존과 동일)
let kernel_handle = Arc::new(oxios_kernel::KernelHandle::new(
    ...,
    engine_api,
    ...,
));

// ⑤ AgentRuntime에 engine_handle 전달
let agent_runtime = AgentRuntime::new(
    Arc::clone(&engine_handle),  // ← Arc<OxiosEngine> 대신 Arc<EngineHandle>
    model_id,
    kernel_handle.clone(),
    Some(Arc::clone(&routing_stats)),
);
```

### 4.3 `_engine_api` 제거

```diff
- // Routing stats — shared between EngineApi and AgentRuntime
- let routing_stats = Arc::new(oxios_kernel::RoutingStats::new());
- let _engine_api = oxios_kernel::EngineApi::new(
-     Arc::new(parking_lot::RwLock::new(config.clone())),
-     config_path.clone(),
-     Arc::clone(&routing_stats),
- );
```

`routing_stats` 생성은 유지하되, 불필요한 `_engine_api` 생성을 제거한다.

---

## 5. 변경 파일 목록

| 파일 | 변경 유형 | 설명 |
|------|-----------|------|
| `crates/oxios-kernel/src/engine.rs` | **수정** | `EngineHandle` 구조체 추가 |
| `crates/oxios-kernel/src/kernel_handle/engine_api.rs` | **수정** | `EngineHandle` 참조 추가, `rebuild_and_swap()` 구현 |
| `crates/oxios-kernel/src/kernel_handle/mod.rs` | **수정** | `EngineHandle` re-export |
| `crates/oxios-kernel/src/agent_runtime.rs` | **수정** | `engine: Arc<OxiosEngine>` → `engine_handle: Arc<EngineHandle>` |
| `crates/oxios-kernel/src/lib.rs` | **수정** | `EngineHandle` pub export |
| `crates/oxios-kernel/src/supervisor.rs` | **수정** | `AgentRuntime::new` 시그니처 변경 반영 |
| `src/kernel.rs` | **수정** | 조립 로직: `EngineHandle` 생성, `_engine_api` 제거 |

**Web / CLI / Telegram 채널은 변경 없음** — 이들은 `KernelHandle.engine` (EngineApi)을 통해서만 접근.

---

## 6. `EngineApi` 생성자 변경

```rust
impl EngineApi {
    pub fn new(
        config: Arc<RwLock<OxiosConfig>>,
        config_path: PathBuf,
        routing_stats: Arc<RoutingStats>,
+       engine_handle: Arc<EngineHandle>,
    ) -> Self {
        Self {
            config,
            config_path,
            routing_stats,
+           engine_handle,
        }
    }
}
```

`KernelHandle::from_subsystems()` (deprecated)에도 `engine_handle`을 받도록 추가하거나,
deprecated이므로 임시로 `EngineHandle::new(Arc::new(OxiosEngine::new("anthropic/claude-sonnet-4-20250514")))`로 stub 처리.

---

## 7. `AgentRuntime::new()` 시그니처 변경

```rust
// Before
pub fn new(
    engine: Arc<OxiosEngine>,
    model_id: impl Into<String>,
    kernel_handle: Arc<KernelHandle>,
    routing_stats: Option<Arc<RoutingStats>>,
) -> Self

// After
pub fn new(
    engine_handle: Arc<EngineHandle>,
    model_id: impl Into<String>,
    kernel_handle: Arc<KernelHandle>,
    routing_stats: Option<Arc<RoutingStats>>,
) -> Self {
    Self {
        engine_handle,
        config: AgentRuntimeConfig {
            model_id: model_id.into(),
            ..Default::default()
        },
        kernel_handle,
        // ...
    }
}
```

---

## 8. 테스트 계획

### 8.1 단위 테스트

| 테스트 | 파일 | 내용 |
|--------|------|------|
| `engine_handle_get_swap` | `engine.rs` | `get()` 후 `swap()` → 다시 `get()` 시 새 엔진 반환 |
| `engine_handle_concurrent_read` | `engine.rs` | `get()`으로 얻은 `Arc`가 swap 후에도 유효 |
| `engine_api_set_model_hot_swap` | `engine_api.rs` | `set_model()` → `EngineHandle.get()`이 새 모델 반환 |
| `engine_api_set_api_key_hot_swap` | `engine_api.rs` | `set_api_key()` → `EngineHandle.get()`의 엔진이 새 키 포함 |
| `agent_runtime_uses_latest_engine` | `agent_runtime.rs` | 모의로 swap 후 `execute()` 시 새 엔진 사용 |

### 8.2 통합 테스트 (시나리오)

```bash
# 1. daemon 시작 (anthropic/claude-sonnet-4)
cargo run -- --foreground &

# 2. Web UI에서 모델 변경
curl -X PUT http://localhost:4200/api/engine/model \
  -H 'Content-Type: application/json' \
  -d '{"model_id": "openai/gpt-4o"}'

# 3. 바로 실행 — gpt-4o 사용 확인
curl -X POST http://localhost:4200/api/chat \
  -d '{"message": "what model are you?"}'

# 4. API 키 변경
curl -X PUT http://localhost:4200/api/engine/api-key \
  -d '{"provider": "openai", "api_key": "sk-new-key"}'

# 5. 실행 — 새 키로 인증 시도 확인 (401 또는 성공)
```

---

## 9. 동시성 안전성 분석

| 시나리오 | 안전성 | 설명 |
|----------|--------|------|
| 두 Web 요청이 동시에 `set_model()` | ✅ | `parking_lot::RwLock`이 순차 보장. 마지막 write가 승리 |
| `set_model()` 중 `execute()` 시작 | ✅ | `execute()`는 `get()`으로 `Arc`를 복제하므로 swap 전후 어느 쪽이든 일관된 엔진 사용 |
| 한 실행 중 swap 발생 | ✅ | 진행 중 실행은 이전 `Arc<OxiosEngine>`을 계속 사용. 다음 실행부터 새 엔진 |
| swap 빈도 | ✅ | 사용자 액션에 의해서만 발생. 초당 수백 번 swap 같은 시나리오 없음 |

---

## 10. 마이그레이션 가이드

### 외부 크레이트 (oxios-web, oxios-cli, oxios-telegram)

**변경 없음.** 채널들은 `KernelHandle.engine` (EngineApi)만 사용하며,
`EngineApi`의 퍼블릭 메서드 시그니처는 변경되지 않는다.

### 직접 `OxiosEngine`을 생성하는 코드

```rust
// Before
let engine = Arc::new(OxiosEngine::new("anthropic/claude-sonnet-4-20250514"));

// After
let engine = Arc::new(OxiosEngine::new("anthropic/claude-sonnet-4-20250514"));
let engine_handle = Arc::new(EngineHandle::new(engine));
```

### 직접 `AgentRuntime::new()`를 호출하는 코드

```rust
// Before
AgentRuntime::new(engine, model_id, kernel_handle, routing_stats)

// After
AgentRuntime::new(engine_handle, model_id, kernel_handle, routing_stats)
```

### 직접 `EngineApi::new()`를 호출하는 코드

```rust
// Before
EngineApi::new(config, path, routing_stats)

// After
EngineApi::new(config, path, routing_stats, engine_handle)
```

---

## 11. 리빌드 비용 분석

`OxiosEngine::from_config()` 내부:

```rust
pub fn from_config(default_model_id: &str, config_api_key: Option<&str>) -> Self {
    let mut builder = OxiBuilder::new().with_builtins();  // 정적 레지스트리 로드 (~1μs)
    for provider in ["anthropic", "openai", "google", "deepseek", "xai"] {
        if let Some((key, _)) = CredentialStore::resolve(provider, ...) {
            builder = builder.api_key(provider, key);      // HashMap insert (~ns)
        }
    }
    let oxi = builder.build();                              // Arc 생성 (~ns)
    // ...
}
```

| 단계 | 비용 | I/O | 네트워크 |
|------|------|-----|----------|
| `OxiBuilder::new().with_builtins()` | ~1μs | 없음 | 없음 |
| `CredentialStore::resolve()` × 5 | ~1μs (메모리 조회) | 없음 | 없음 |
| `builder.build()` | ~100ns | 없음 | 없음 |
| **합계** | **~3μs** | **없음** | **없음** |

결론: **리빌드 비용은 무시할 수 있다.** 사용자 액션에 의해 발생하므로 빈도도 극히 낮다.

---

## 12. 리스크 & 완화

| 리스크 | 가능성 | 영향 | 완화 |
|--------|--------|------|------|
| swap 중 짧은 write lock으로 execute 지연 | 낮음 | ~3μs | parking_lot RwLock은 write를 위해 read를 기다리지 않음 (fairness 정책). 3μs는 무시 가능 |
| `from_subsystems()` deprecated 경로 깨짐 | 중간 | 테스트만 영향 | stub EngineHandle으로 임시 처리 |
| `OxiBuilder`가 나중에 I/O를 추가하면 리빌드 비용 증가 | 낮음 | 향후 문제 | 그 때 async rebuild로 전환. 현재는 sync로 충분 |
| config가 여러 소스에서 동시 변경 (Web + CLI) | 있음 | 마지막 write 승리 | config.toml이 single source of truth. 이미 RwLock 보호 |

---

## 13. 구현 순서

```
Phase 1: EngineHandle (engine.rs)
  ├── EngineHandle struct + get() + swap()
  └── 단위 테스트

Phase 2: EngineApi 통합 (engine_api.rs)
  ├── engine_handle 필드 추가
  ├── rebuild_and_swap() 구현
  ├── set_model() / set_api_key() / set_routing() 에서 rebuild_and_swap() 호출
  └── 통합 테스트

Phase 3: AgentRuntime 마이그레이션 (agent_runtime.rs)
  ├── engine → engine_handle
  ├── execute()에서 get() 호출
  └── 기존 테스트 수정

Phase 4: kernel.rs 조립 (kernel.rs)
  ├── EngineHandle 생성
  ├── _engine_api 제거
  ├── EngineApi::new()에 engine_handle 전달
  └── AgentRuntime::new()에 engine_handle 전달

Phase 5: supervisor.rs + 테스트 수정
  ├── supervisor mock 업데이트
  └── e2e 시나리오 테스트
```

---

## 14. 체크리스트

- [ ] `EngineHandle` 구현 + 테스트
- [ ] `EngineApi.engine_handle` 필드 + `rebuild_and_swap()`
- [ ] `EngineApi::set_model()` → hot-swap
- [ ] `EngineApi::set_api_key()` → hot-swap
- [ ] `EngineApi::set_routing()` → hot-swap
- [ ] `AgentRuntime.engine_handle` 마이그레이션
- [ ] `kernel.rs` `_engine_api` 제거
- [ ] `kernel.rs` EngineHandle 조립
- [ ] `supervisor.rs` 테스트 수정
- [ ] `from_subsystems()` deprecated 경로 수정
- [ ] `cargo test --workspace` 통과
- [ ] Web UI 시나리오 수동 테스트 (모델 전환 → 즉시 실행)
