# RFC: Rust 2024 Edition 업그레이드

> **상태:** 완료
> **날짜:** 2026-06-07
> **대상:** workspace 전체 (12개 Cargo.toml)
> **Rust 버전:** 1.88 → 1.96.0, edition 2021 → 2024

---

## 1. 배경

Rust 2024 Edition이 1.85.0에서 안정화. Rust 1.96으로 올리면서 edition 2024 전환.

## 2. 변경 내용

### 2.1 기본 마이그레이션

- **rust-toolchain.toml**: `1.88` → `1.96`
- **12개 Cargo.toml**: `edition = "2024"`
- **Cargo.toml**: `resolver = "2"` 제거 (2024 자동 적용)
- **`gen` 예약어** 충돌 수정 2건 (`r#gen`)
- **Match ergonomics `ref`** 제거 8건

### 2.2 Let Chains (92+곳)

중첩 `if let`을 `&&` 체인으로 변환:

```rust
// Before:
if let Some(x) = opt {
    if x > 0 {
        // ...
    }
}
// After:
if let Some(x) = opt
    && x > 0
{
    // ...
}
```

### 2.3 `#[async_trait]` 제거 → 수동 desugar (16개 파일, 22곳)

`async_trait` 0.1.89가 Rust 1.96 + edition 2024와 호환되지 않아([issue #294](https://github.com/dtolnay/async-trait/issues/294)) 제거.

oxi-agent 0.31.0의 `AgentTool::execute`가 이미 수동 desugared signature이므로, impl도 동일한 방식으로 맞춤:

```rust
// Before (#[async_trait]):
#[async_trait]
impl AgentTool for Foo {
    async fn execute(&self, id: &str, ...) -> Result<AgentToolResult, ToolError> {
        // ...
    }
}

// After (manual desugar):
impl AgentTool for Foo {
    fn execute<'a>(
        &'a self,
        id: &'a str,
        ...
        ctx: &'a ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<AgentToolResult, ToolError>> + Send + 'a>> {
        Box::pin(async move {
            // ...
        })
    }
}
```

### 2.4 RPIT → async fn (2건)

- `crates/oxios-kernel/src/backup.rs`: `Pin<Box<dyn Future + Send + 'a>>` → `async fn`
- `surface/oxios-web/src/routes/workspace.rs`: 동일

### 2.5 Import 정리

- `use std::future::Future` 제거 (2024 prelude)
- `use std::pin::Pin` 제거 (미사용)

## 3. `async_trait` 제거에 대한 평가

| 측면 | `#[async_trait]` | 수동 desugar |
|------|------------------|--------------|
| 가독성 | `async fn` — 자연스러움 | 서명이 길고 복잡 |
| 보일러플레이트 | 없음 | `'a` lifetime, `Box::pin` 매번 필요 |
| 의존성 | `async-trait` crate 필요 | 불필요 |
| edition 호환 | 2024에서 깨짐 | 모든 edition에서 작동 |

이 변경은 **개선이 아닙니다.** `#[async_trait]` 매크로의 역할을 손으로 복사한 것입니다.
Rust가 `async fn in trait`의 `dyn`-safe 지원을 안정화하면, `Pin<Box<...>>` 보일러플레이트를
다시 `async fn`으로 되돌려야 합니다.

## 4. 향후 과제

- `async_trait` crate의 edition 2024 호환 수정 대기 → 복원 가능
- Rust의 `dyn`-safe `async fn in trait` 안정화 대기 → `#[async_trait]` 완전 제거
