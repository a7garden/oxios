# RFC: Browser Migration — oxios → oxi-sdk

> **Status**: In Progress (awaiting SDK publish)
> **Depends on**: `oxi-sdk` `native-browser` feature + `BrowseSessionTool` (RFC pending in oxi)

## Summary

Remove the custom `BrowserTool` that directly depends on `oxibrowser-core`, and migrate to the browser tools provided by `oxi-sdk` (`browse`, `browse_extract`, `browse_script`, + new `browse_session`).

## Current Architecture

```
oxios-kernel
├── Cargo.toml              ← oxibrowser-core = "0.9.1" (직접 의존)
├── src/kernel_handle/
│   └── browser_api.rs      ← BrowserApi (lazy init facade)
├── src/tools/
│   └── browser/
│       ├── mod.rs
│       └── browser_tool.rs ← 단일 BrowserTool (18 actions)
└── src/tools/
    ├── registration.rs     ← ResourceRef::Browser → BrowserTool
    └── kernel_bridge.rs    ← "browser" in tool_names
```

### Current BrowserTool actions

| Action | Backend |
|--------|---------|
| `browse` | `browser.browse(url)` |
| `goto` | `tab.goto(url)` |
| `back` | `tab.back()` |
| `forward` | `tab.forward()` |
| `reload` | `tab.reload()` |
| `post` | `tab.post(url, body, ct)` |
| `click` | `tab.click(selector)` |
| `type` | `tab.r#type(selector, text)` |
| `press_key` | `tab.press_key(key)` |
| `evaluate` | `tab.evaluate(js)` |
| `evaluate_await` | `tab.evaluate_await(js)` |
| `content` | `tab.content()` |
| `query_all` | `tab.query_all(selector)` |
| `wait_for` | `tab.wait_for(selector, ms)` |
| `load_resources` | `tab.load_resources()` |
| `screenshot` | `tab.screenshot(width)` |
| `run_script` | `oxibrowser_core::script::ScriptRunner` |
| `close` | `tab.close()` |

### SDK version gap

| Package | Current | SDK target |
|---------|---------|------------|
| `oxibrowser-core` | `0.9.1` | `0.11` |

## Target Architecture

```
oxios-kernel
├── Cargo.toml              ← oxibrowser-core 의존 제거
├── src/kernel_handle/
│   └── browser_api.rs      ← Arc<dyn BrowserEngine> 기반 전환
├── src/tools/
│   └── browser/            ← 제거 예정 (SDK 툴 사용)
└── src/tools/
    ├── registration.rs     ← SDK BrowseTool, BrowseExtractTool, BrowseSessionTool 등록
    └── kernel_bridge.rs    ← "browse", "browse_extract", "browse_session" in tool_names
```

## SDK Tools

| Tool | Description | Feature |
|------|-------------|---------|
| `browse` | One-shot page read (markdown/html/text/links) | Always available |
| `browse_extract` | CSS selector extraction (links/text/elements/markdown) | Always available |
| `browse_script` | Multi-step YAML automation | `native-browser` feature |
| `browse_session` | Interactive persistent tab (open → ops → close) | Always available |

## Migration Steps

### Phase 0: Document (current) — ❌ 미완료

- [x] 현재 코드 분석 완료
- [ ] 이 RFC 작성 ← **지금 하는 일**

### Phase 1: oxi-sdk 배포 대기

oxi 측에서 다음을 완료할 때까지 기다림:
1. `BrowseSessionTool` 구현 (`docs/rfc-browser-interactive-sessions.md` 요청서 작성 완료)
2. `oxi-sdk` + `oxi-agent` crates.io 배포
3. `oxibrowser-core` → `0.11` 업데이트

### Phase 2: oxios 마이그레이션

#### 2.1 `oxibrowser-core` 직접 의존 제거

```toml
# crates/oxios-kernel/Cargo.toml

# 제거:
# oxibrowser-core = "0.9.1"

[features]
# 의미 변경: SDK의 native-browser feature를 transitive로 사용
browser = []
```

#### 2.2 `BrowserConfig` 업데이트

현재 `oxibrowser_core::BrowserConfig`를 사용하는 `BrowserConfig`를 SDK의 `BrowseConfig`로 교체하거나, SDK 타입을 그대로 사용.

```rust
// config.rs에서:
// Before:
pub struct BrowserConfig {
    pub enabled: bool,
    pub engine: oxibrowser_core::BrowserConfig,
}

// After: SDK의 BrowseConfig 사용 또는-compatible한 구조
pub struct BrowserConfig {
    pub enabled: bool,
    pub engine: oxi_sdk::BrowseConfig,
}
```

#### 2.3 `BrowserApi` trait 기반 전환

```rust
// kernel_handle/browser_api.rs

// Before: Arc<oxibrowser_core::Browser>
pub struct BrowserApi {
    inner: tokio::sync::OnceCell<Arc<oxibrowser_core::Browser>>,
    config: Option<oxibrowser_core::BrowserConfig>,
}

// After: Arc<dyn BrowserEngine>
pub struct BrowserApi {
    inner: tokio::sync::OnceCell<Arc<dyn oxi_sdk::BrowserEngine>>,
    config: Option<oxi_sdk::BrowseConfig>,
}

impl BrowserApi {
    /// SDK의 OxiBrowserEngine으로 초기화 (native-browser feature)
    #[cfg(feature = "native-browser")]
    pub async fn browser(&self) -> anyhow::Result<&Arc<dyn BrowserEngine>> {
        self.inner.get_or_try_init(|| async {
            let config = self.config.clone().unwrap_or_default();
            let backend = oxi_agent::tools::browse::OxiBrowserEngine::with_config(config)?;
            Ok(Arc::new(backend) as Arc<dyn BrowserEngine>)
        }).await
    }
}
```

#### 2.4 툴 등록 업데이트

```rust
// registration.rs

// Before:
#[cfg(feature = "browser")]
ResourceRef::Browser if cap.rights.contains(Rights::EXECUTE) => {
    registry.register(BrowserTool::from_kernel(kernel));
}

// After:
ResourceRef::Browser if cap.rights.contains(Rights::EXECUTE) => {
    let engine = kernel.browser.engine().await.ok()?;
    registry.register(oxi_sdk::BrowseTool::new(engine.clone()));
    registry.register(oxi_sdk::BrowseExtractTool::new(engine.clone()));
    registry.register(oxi_sdk::BrowseSessionTool::new(engine));
    #[cfg(feature = "native-browser")]
    registry.register(oxi_sdk::BrowseScriptTool::new(engine));
}
```

#### 2.5 `kernel_bridge.rs` tool_names 업데이트

```rust
// Before:
"browser",

// After:
"browse",
"browse_extract",
"browse_session",
// "browse_script" — native-browser feature에서만
```

#### 2.6 기존 `BrowserTool` 파일 제거

```
crates/oxios-kernel/src/tools/browser/
├── mod.rs       ← 제거
└── browser_tool.rs  ← 제거
```

#### 2.7 `tools/mod.rs` 업데이트

```rust
// Before:
#[cfg(feature = "browser")]
pub mod browser;
...
#[cfg(feature = "browser")]
pub use browser::BrowserTool;

// After: SDK에서 re-export되므로 직접 정의 불필요
// 필요시:
pub use oxi_sdk::{BrowseTool, BrowseExtractTool, BrowseSessionTool};
```

## Feature Parity Checklist

현재 `BrowserTool` actions → SDK 툴으로 매핑:

| Current Action | SDK Tool | Status |
|----------------|----------|--------|
| `browse(url)` | `browse` tool | ✅ |
| `goto(url)` | `browse_session` action | ✅ (새로 구현 중) |
| `back` | `browse_session` action | ✅ |
| `forward` | `browse_session` action | ✅ |
| `reload` | `browse_session` action | ✅ |
| `post(url, body, ct)` | — | ❌ SDK에 없음 |
| `click(selector)` | `browse_session` action | ✅ |
| `type(selector, text)` | `browse_session` action | ✅ |
| `press_key(key)` | `browse_session` action | ✅ |
| `evaluate(js)` | `browse_session` action | ✅ |
| `evaluate_await(js)` | `browse_session` action | ✅ |
| `content()` | `browse_session` action | ✅ |
| `query_all(selector)` | `browse_session` action | ✅ |
| `wait_for(selector, ms)` | `browse_session` action | ✅ |
| `load_resources()` | — | ❌ SDK에 없음 |
| `screenshot(width)` | `browse_session` action | ✅ |
| `run_script(yaml)` | `browse_script` tool | ✅ |
| `close` | `browse_session` action | ✅ |

### Actions needing resolution

- **`post`**: HTTP POST 요청. SDK의 `BrowserTab` trait에 없음. 필요하면 SDK에 요청하거나, 별도 HTTPClient tool 사용.
- **`load_resources`**: 리소스 로딩 카운트 반환. SDK에 없음. 필요성 재검토 필요.

## Non-goals

- `oxibrowser-core`를 완전히 제거하지는 않음 — SDK가 내부적으로 사용
- Web channel의 직접 `oxibrowser-core` 사용은 유지 (knowledge UI screenshot 등)
- CLI의 `oxios run --browse` 같은 단독 명령은 유지 (SDK 툴 사용)

## Rollback Plan

SDK 배포 후 문제가 생기면:
1. `crates/oxios-kernel/Cargo.toml`에서 `oxibrowser-core` 다시 추가
2. `tools/browser/` 파일 복원
3. `registration.rs`, `kernel_bridge.rs` 원복
4. `kernel_handle/browser_api.rs` 원복

Git에 모든 변경사항이 있으므로 간단히 revert 가능.