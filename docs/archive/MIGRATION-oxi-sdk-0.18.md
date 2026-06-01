# oxi-sdk 0.18 마이그레이션 가이드

> **대상**: oxios-kernel에서 oxi-sdk를 사용하는 모든 코드
> **날짜**: 2026-05-17
> **oxi 변경 커밋**: 이 문서와 함께 oxi 워크스페이스에서 수정됨

oxi 측에 4가지 개선사항이 적용되었습니다. 각 항목별로 oxios-kernel에서 변경이 필요한 부분을 설명합니다.

---

## 1. `AgentLoop::run()` Future가 이제 `Send` (P0)

### 배경

기존에는 `AgentLoop::run()`이 반환하는 Future가 `!Send`여서, oxios가 `spawn_blocking` + `Handle::block_on`으로 실행해야 했습니다.

**원인**: `RetryCallback` 트레이트가 `Send`만 있고 `Sync`가 없어서, `&dyn RetryCallback`가 `!Send`가 됨.

**수정**: oxi-agent의 `RetryCallback: Send + Sync`로 변경. 이제 `run()`의 Future가 `Send`입니다.

### oxios 변경 사항

#### 1.1 `agent_runtime.rs` — `spawn_blocking` 제거

**AS-IS** (L268~272):
```rust
let (final_content, steps_completed, success) =
    tokio::task::spawn_blocking(move || run_agent_loop(ctx)).await??;
```

**TO-BE**:
```rust
let (final_content, steps_completed, success) = run_agent_loop(ctx).await?;
```

#### 1.2 `agent_runtime.rs` — `run_agent_loop` 함수 시그니처 변경

**AS-IS**:
```rust
fn run_agent_loop(ctx: AgentLoopContext) -> Result<(String, usize, bool)> {
```

**TO-BE**:
```rust
async fn run_agent_loop(ctx: AgentLoopContext) -> Result<(String, usize, bool)> {
```

#### 1.3 `run_agent_loop` 내부의 `Handle::block_on` 호출 제거

함수 내부에 두 군데의 `rt.block_on()`이 있습니다:

**패턴 A** — 프로그램 목록 조회 (L359):
```rust
// AS-IS
let rt = tokio::runtime::Handle::current();
let programs: Vec<_> = rt.block_on(async { pm.list_enabled().await });
```
```rust
// TO-BE
let programs: Vec<_> = pm.list_enabled().await;
```

**패턴 B** — MCP 브릿지 초기화 (L373~378):
```rust
// AS-IS
if let Err(e) = rt.block_on(bridge.initialize_all()) { ... }
let _ = rt.block_on(bridge.list_tools());
if let Some(tool_defs) = rt.block_on(bridge.cached_tools(server_name)) { ... }
```
```rust
// TO-BE
if let Err(e) = bridge.initialize_all().await { ... }
let _ = bridge.list_tools().await;
if let Some(tool_defs) = bridge.cached_tools(server_name).await { ... }
```

**패턴 C** — 메인 agent loop 실행 (L462~465):
```rust
// AS-IS
let rt = tokio::runtime::Handle::current();
rt.block_on(async {
    let result = agent_loop.run(prompt, move |event| { ... }).await;
    // ...
});
```
```rust
// TO-BE (async 함수이므로 직접 await)
let result = agent_loop
    .run(prompt, move |event| { ... })
    .await;
```

#### 1.4 `run_agent_loop` 내부 콜백의 `rt_for_callback.block_on` 제거

Compaction 콜백 내부 (L504):
```rust
// AS-IS
if let Err(e) = rt_for_callback.block_on(mm.remember(entry)) {
```
```rust
// TO-BE
if let Err(e) = mm.remember(entry).await {
```

#### 1.5 `AgentLoopContext`에서 `!Send` 관련 제약 정리

`AgentLoopContext`의 필드들이 이미 모두 `Send + Sync`이므로 변경 불필요. 다만, `KernelHandle`이 `Send + Sync`인지 확인 필요:
```bash
grep -rn '!Send\|Rc\|RefCell\|UnsafeCell' oxios-kernel/src/kernel_handle/
```

---

## 2. `ToolRegistry::missing()` / `has_all()` 헬퍼 (P4)

### 배경

프로그램 의존성 검증 패턴이 3곳에서 동일한 보일러플레이트로 반복되었습니다.

### oxios 변경 사항

#### 2.1 `agent_runtime.rs` — 프로그램 의존성 검증 단순화

**AS-IS** (L396~410):
```rust
let missing_tools: Vec<&str> = program
    .meta
    .dependencies
    .iter()
    .filter(|tool_name| registry.get(tool_name).is_none())
    .map(|s| s.as_str())
    .collect();
if !missing_tools.is_empty() {
    tracing::warn!(
        program = %program.meta.name,
        missing_tools = ?missing_tools,
        "Skipping program: required tools not found"
    );
    continue;
}
```

**TO-BE**:
```rust
let dep_names: Vec<&str> = program.meta.dependencies.iter().map(|s| s.as_str()).collect();
let missing = registry.missing(&dep_names);
if !missing.is_empty() {
    tracing::warn!(
        program = %program.meta.name,
        missing_tools = ?missing,
        "Skipping program: required tools not found"
    );
    continue;
}
```

#### 2.2 테스트 단순화

`agent_runtime.rs`의 테스트들도 `registry.missing()`으로 교체 가능:

```rust
// AS-IS
let missing: Vec<&str> = required_tools
    .iter()
    .filter(|name| registry.get(name).is_none())
    .map(|s| s.as_str())
    .collect();

// TO-BE
let missing = registry.missing(&required_tools.iter().map(|s| s.as_str()).collect::<Vec<_>>());
```

---

## 3. `KernelToolContext.metadata` 확장 필드 (P3)

### 배경

`KernelToolContext`에 `metadata: HashMap<String, serde_json::Value>` 필드가 추가되었습니다. space_id, cspace_name, seed_id 등을 확장 필드로 전달할 수 있습니다.

### oxios 변경 사항

#### 3.1 `OxiosKernelBridge` 또는 새 경로에서 Context에 메타데이터 주입

현재 `agent_runtime.rs`는 `register_tools_from_cspace()`를 직접 호출하므로 `KernelToolContext`를 생성하지 않습니다. 하지만 향후 `KernelToolProvider` 트레이트를 통해 등록할 경우:

```rust
let ctx = KernelToolContext::new(&workspace, &agent_id.to_string())
    .with_session(seed_id.to_string())
    .with_meta("space_id", serde_json::json!(space_id))
    .with_meta("cspace_name", serde_json::json!(cspace.name()))
    .with_meta("seed_id", serde_json::json!(seed_id));
```

#### 3.2 커널 툴에서 메타데이터 읽기

```rust
// 툴 내부에서
if let Some(space_id) = ctx.get_meta_str("space_id") {
    // space_id를 사용한 로직
}
```

---

## 4. `OxiBuilder::provider_factory()` (P6)

### 배경

`engine.rs`의 `zai` provider 하드코딩 if문을 팩토리 패턴으로 교체할 수 있습니다.

### oxios 변경 사항

#### 4.1 `engine.rs` — provider 팩토리 등록

**AS-IS**:
```rust
let mut builder = OxiBuilder::new().with_builtins();

if provider_name == "zai" {
    let api_key = crate::credential::CredentialStore::resolve("zai", None)
        .map(|(key, _)| key);
    let zai_base_url = std::env::var("ZAI_BASE_URL")
        .unwrap_or_else(|_| "https://api.z.ai/api/coding/paas/v4".to_string());
    let zai_provider = oxi_ai::OpenAiProvider::with_base_url_and_key(&zai_base_url, api_key);
    builder = builder.provider("zai", zai_provider);
    tracing::info!("Registered zai provider...");
}
```

**TO-BE**:
```rust
let mut builder = OxiBuilder::new().with_builtins();

// Register OpenAI-compatible providers via factory
builder = register_compatible_providers(builder, provider_name);
```

별도 함수로:
```rust
fn register_compatible_providers(builder: OxiBuilder, default_provider: &str) -> OxiBuilder {
    let compatible_providers: &[(&str, &str)] = &[
        ("zai", "https://api.z.ai/api/coding/paas/v4"),
        // Future OpenAI-compatible providers can be added here
    ];

    let mut builder = builder;
    for (name, default_url) in compatible_providers {
        let name_owned = name.to_string();
        let url_owned = default_url.to_string();
        builder = builder.provider_factory(name, move || {
            let api_key = crate::credential::CredentialStore::resolve(&name_owned, None)
                .map(|(key, _)| key);
            let base_url = std::env::var(format!("{}_BASE_URL", name_owned.to_uppercase()))
                .unwrap_or_else(|_| url_owned.clone());
            let provider = oxi_ai::OpenAiProvider::with_base_url_and_key(&base_url, api_key);
            tracing::info!(
                "Registered {} provider (OpenAI-compatible, base_url: {})",
                name_owned, base_url
            );
            Ok(std::sync::Arc::new(provider))
        });
    }
    builder
}
```

---

## 변경 요약

| 항목 | oxi 측 (완료) | oxios 측 (필요) |
|------|--------------|-----------------|
| **P0** | `RetryCallback: Send + Sync`, `Agent::run()`에서 `spawn_blocking` 제거 | `agent_runtime.rs`에서 `spawn_blocking`, `block_on` 제거 |
| **P4** | `ToolRegistry::missing()`, `has_all()` 추가 | `agent_runtime.rs` 의존성 검증 보일러플레이트 교체 |
| **P3** | `KernelToolContext.metadata`, `with_meta()`, `get_meta()` 추가 | 필요시 Context에 메타데이터 주입 |
| **P6** | `OxiBuilder::provider_factory()`, `ProviderRegistry::register_factory()` | `engine.rs`의 하드코딩 provider 분기문 교체 |

## 영향을 받지 않는 것들

다음은 분석 결과 **변경이 불필요**한 것들입니다:

- **MessageBus / EventBus 통합 (P1)**: 서로 다른 추상화 레벨. MessageBus는 inter-agent 메시징, EventBus는 커널 수명주기 이벤트. 역할이 다르므로 그대로 유지.
- **AgentGroup / OxiosAgentGroup 통합 (P2)**: SDK AgentGroup은 "실행 전략", OxiosAgentGroup은 "상태 모델". SRP에 의해 분리가 올바름.
- **AgentEvent Progress (P5)**: SDK의 이벤트는 충분. 스트리밍 필요시 oxios 채널 측에서 EventBus 구독으로 해결 가능.
