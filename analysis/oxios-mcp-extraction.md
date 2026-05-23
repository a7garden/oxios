# oxios-mcp 추출 설계서

> 2026-05-23
> 범위: `crates/oxios-kernel/src/mcp/` (1,536 LOC, 3 files) → `crates/oxios-mcp/`

---

## 1. 현재 상태

### 의존성 방향 (현재)

```
mcp/protocol.rs ──→ program::ToolDef, program::ArgumentDef  (Oxios 종속)
mcp/mod.rs ───────→ program::ToolDef                        (Oxios 종속)
mcp/client.rs ────→ (외부 크레이트만: tokio, anyhow, serde)  (독립)
```

### kernel 내부에서 mcp를 사용하는 곳 (3곳)

| 파일 | 임포트 | 용도 |
|------|--------|------|
| `tools/mcp_tool.rs` | `McpBridge`, `McpContentBlock` | AgentTool 래퍼 |
| `kernel_handle/mcp_api.rs` | `McpBridge`, `McpServer`, `McpToolCallResult` | 커널 API |
| `kernel_handle/mod.rs` | `McpBridge` | 커널 필드 |

### binary crate (`src/kernel.rs`)에서의 사용

```rust
use oxios_kernel::McpBridge;
use oxios_kernel::McpServer;

// 1. init_mcp_bridge() — config에서 McpBridge 생성
// 2. init_mcp_servers() — 모든 서버 초기화
// 3. KernelHandle에 McpApi::new(mcp_bridge) 등록
```

---

## 2. 설계 원칙

1. **`oxios-mcp`는 Oxios에 종속되지 않는 범용 MCP 클라이언트 라이브러리**
2. **kernel은 `oxios-mcp`에 의존한다. `oxios-mcp`는 kernel에 의존하지 않는다.**
3. **기존 `oxios_kernel::mcp::*` 경로는 re-export로 보존**
4. **`ToolDef` 변환은 kernel adapter에서 처리**

---

## 3. 타입 경계: ToolDef 분리

### 문제

```rust
// 현재: mcp/protocol.rs
impl McpTool {
    pub fn to_tool_def(&self) -> ToolDef {  // ← program::ToolDef (Oxios 종속)
        ...
        crate::program::ArgumentDef { ... }
        ToolDef { name, description, arguments, command: String::new() }
    }
}
```

MCP 프로토콜 입장에서 `ToolDef`는 Oxios 개념이야. MCP가 아는 건 `McpTool` (name, description, input_schema)뿐.

### 해결: `to_tool_def()`를 kernel adapter로 이동

```rust
// oxios-mcp (독립 크레이트)
impl McpTool {
    /// 그대로 McpTool 유지. ToolDef 변환은 제공하지 않음.
    pub fn name(&self) -> &str { &self.name }
    pub fn description(&self) -> &str { &self.description }
    pub fn input_schema(&self) -> &serde_json::Value { &self.input_schema }
}
```

```rust
// oxios-kernel (adapter)
impl McpToolExt for oxios_mcp::McpTool {
    fn to_tool_def(&self) -> ToolDef {
        // 기존 로직 그대로 이동
    }
}
```

`McpBridge::list_tools()`와 `cached_tools()`가 `ToolDef`를 반환하는 것도 같은 방식으로 처리:

```rust
// oxios-mcp (독립)
impl McpBridge {
    /// MCP 네이티브 타입 반환
    pub async fn list_mcp_tools(&self) -> Result<Vec<McpTool>> { ... }
    pub async fn cached_mcp_tools(&self, server: &str) -> Option<Vec<McpTool>> { ... }
}

// oxios-kernel (adapter — 기존 API 보존)
// mcp_bridge.rs (kernel 내부에 새로 생성)
pub async fn list_tools_as_tool_defs(bridge: &McpBridge) -> Result<Vec<ToolDef>> {
    let mcp_tools = bridge.list_mcp_tools().await?;
    Ok(mcp_tools.iter().map(|t| mcp_tool_to_tool_def(t)).collect())
}
```

---

## 4. 크레이트 구조

### `crates/oxios-mcp/`

```
crates/oxios-mcp/
├── Cargo.toml
└── src/
    ├── lib.rs          ← 공개 API
    ├── protocol.rs     ← JSON-RPC 2.0 + MCP 도메인 타입
    └── client.rs       ← McpClient (프로세스 관리, stdio 통신)
```

### Cargo.toml

```toml
[package]
name = "oxios-mcp"
version = "0.1.0"
edition = "2021"
description = "Model Context Protocol client — JSON-RPC 2.0 over stdio"
license = "MIT"

[dependencies]
anyhow = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = "1"
parking_lot = { workspace = true }
```

**핵심: Oxios 워크스페이스 크레이트에 대한 의존성이 0개.** 순수하게 tokio + serde만 사용.

### 공개 API (lib.rs)

```rust
//! oxios-mcp — Model Context Protocol client library.
//!
//! Implements MCP over JSON-RPC 2.0 via stdio transport.
//! Independent of the Oxios kernel — usable as a standalone MCP client.

pub mod client;
pub mod protocol;

pub use client::McpClient;
pub use protocol::*;
```

### protocol.rs 변경점

```diff
- use crate::program::ToolDef;
  // (제거)

  impl McpTool {
-     pub fn to_tool_def(&self) -> ToolDef { ... }
+     /// Get the tool name.
+     pub fn name(&self) -> &str { &self.name }
+     /// Get the tool description.
+     pub fn description(&self) -> &str { &self.description }
+     /// Get the tool input JSON Schema.
+     pub fn input_schema(&self) -> &serde_json::Value { &self.input_schema }
  }
```

### mod.rs → McpBridge 변경점

```diff
- use crate::program::ToolDef;
  // (제거)

  impl McpBridge {
-     pub async fn list_tools(&self) -> Result<Vec<ToolDef>> {
+     pub async fn list_tools(&self) -> Result<Vec<McpTool>> {
          // McpTool 직접 반환 (ToolDef 변환 없음)
          let mcp_tools = client.list_tools().await?;
-         let defs: Vec<ToolDef> = mcp_tools.iter().map(|t| t.to_tool_def()).collect();
-         ...
-         all_tools.extend(defs);
+         all_tools.extend(mcp_tools);
      }

-     pub async fn cached_tools(&self, server_name: &str) -> Option<Vec<ToolDef>> {
+     pub async fn cached_tools(&self, server_name: &str) -> Option<Vec<McpTool>> {
      }

-     pub async fn refresh_tools(&self, server_name: &str) -> Result<Vec<ToolDef>> {
+     pub async fn refresh_tools(&self, server_name: &str) -> Result<Vec<McpTool>> {
      }
  }
```

---

## 5. kernel 쪽 변경

### 5.1 `crates/oxios-kernel/Cargo.toml`

```toml
[dependencies]
oxios-mcp = { version = "0.1.0", path = "../oxios-mcp" }
# ... 기존 의존성 동일
```

### 5.2 `crates/oxios-kernel/src/mcp/` → adapter 레이어로 축소

기존 3개 파일 대신 1개 파일로:

```rust
// crates/oxios-kernel/src/mcp/mod.rs (새로 작성, ~60줄)
//! MCP integration — adapters between oxios-mcp and the kernel.

pub use oxios_mcp::{McpBridge, McpClient, McpServer, McpTool, ...};

use crate::program::{ArgumentDef, ToolDef};

/// MCP → Oxios ToolDef 변환.
pub fn mcp_tool_to_tool_def(tool: &oxios_mcp::McpTool) -> ToolDef {
    let arguments = if let Some(properties) = tool.input_schema()
        .get("properties")
        .and_then(|p| p.as_object())
    {
        let required_list: Vec<&str> = tool.input_schema()
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        properties.iter().map(|(name, schema)| {
            ArgumentDef {
                name: name.clone(),
                description: schema.get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("No description")
                    .to_string(),
                required: required_list.iter().any(|r| *r == name)
                    && schema.get("default").is_none(),
                default: schema.get("default")
                    .and_then(|d| d.as_str().map(String::from)),
            }
        }).collect()
    } else {
        Vec::new()
    };

    ToolDef {
        name: tool.name().to_string(),
        description: tool.description().to_string(),
        arguments,
        command: String::new(),
    }
}

/// Bridge에서 ToolDef 리스트 가져오기 (기존 API 호환).
pub async fn list_tool_defs(bridge: &McpBridge) -> anyhow::Result<Vec<ToolDef>> {
    let tools = bridge.list_tools().await?;
    Ok(tools.iter().map(mcp_tool_to_tool_def).collect())
}

/// Bridge에서 캐시된 ToolDef 리스트 가져오기 (기존 API 호환).
pub async fn cached_tool_defs(bridge: &McpBridge, server: &str) -> Option<Vec<ToolDef>> {
    bridge.cached_tools(server).await
        .map(|tools| tools.iter().map(mcp_tool_to_tool_def).collect())
}
```

### 5.3 `kernel_handle/mcp_api.rs` 변경

```diff
- use crate::mcp::{McpBridge, McpServer, McpToolCallResult};
- use crate::program::ToolDef;
+ use crate::mcp::{McpBridge, McpServer, McpToolCallResult};
+ use crate::mcp::{list_tool_defs, cached_tool_defs};
+ use crate::program::ToolDef;

  impl McpApi {
      pub async fn list_tools(&self) -> anyhow::Result<Vec<ToolDef>> {
-         self.mcp_bridge.list_tools().await
+         list_tool_defs(&self.mcp_bridge).await
      }

      pub async fn cached_tools(&self, server: &str) -> Option<Vec<ToolDef>> {
-         self.mcp_bridge.cached_tools(server).await
+         cached_tool_defs(&self.mcp_bridge, server).await
      }
  }
```

### 5.4 `tools/mcp_tool.rs` — 변경 없음

`McpBridge`, `McpContentBlock`은 re-export로 동일 경로 유지.

### 5.5 binary crate (`src/kernel.rs`) — 변경 없음

```rust
use oxios_kernel::McpBridge;  // re-export로 동일
use oxios_kernel::McpServer;  // re-export로 동일
```

---

## 6. 영향 범위 정리

### oxios-mcp (새 크레이트)

| 파일 | 변경 |
|------|------|
| `client.rs` | `use super::protocol::*` → `use crate::protocol::*` (경로만) |
| `protocol.rs` | `ToolDef`/`ArgumentDef` 제거, `to_tool_def()` → getter 메서드 |
| `mod.rs` | `use crate::program::ToolDef` 제거, `list_tools`가 `Vec<McpTool>` 반환 |

### oxios-kernel

| 파일 | 변경 |
|------|------|
| `mcp/` (3파일) | 삭제 → `mcp/mod.rs` 1개로 교체 (adapter + re-export) |
| `kernel_handle/mcp_api.rs` | `list_tools`, `cached_tools` 호출을 adapter 함수로 |
| `Cargo.toml` | `oxios-mcp` 의존성 추가 |
| `lib.rs` | 변경 없음 (re-export 경로 동일) |

### 외부 크레이트

| 크레이트 | 변경 |
|----------|------|
| `oxios-web` | 없음 |
| `oxios-cli` | 없음 |
| `oxios-telegram` | 없음 |
| `oxios` (binary) | 없음 |

---

## 7. 테스트

### 이동되는 테스트 (oxios-mcp)

- `McpServer` 빌더 테스트
- `McpRequest` 직렬화/역직렬화 테스트
- `McpResponse` 결과/에러 테스트
- `McpError` 코드 테스트
- `McpClient` 수명 주기 테스트 (비존재 명령어, 셧다운, 타임아웃)
- `McpBridge` 등록/초기화 테스트
- JSON-RPC 에코 테스트

### kernel에 남는 테스트

- `McpTool → ToolDef` 변환 테스트 (adapter)
- `list_tool_defs` 통합 테스트

### 체크리스트

```bash
# 1. oxios-mcp 독립 빌드
cargo build -p oxios-mcp

# 2. oxios-mcp 독립 테스트
cargo test -p oxios-mcp

# 3. 전체 워크스페이스
cargo build --workspace
cargo test --workspace

# 4. clippy
cargo clippy --workspace

# 5. feature gates
cargo build -p oxios --features web,cli,browser
```

---

## 8. 추출 후 구조

```
crates/
├── oxios-mcp/          ← NEW (1,536 LOC, 범용 MCP 클라이언트)
│   ├── Cargo.toml      ← Oxios 의존성 0개
│   └── src/
│       ├── lib.rs
│       ├── protocol.rs ← JSON-RPC 2.0 + MCP 타입 (ToolDef 변환 없음)
│       └── client.rs   ← McpClient (프로세스 관리)
│
├── oxios-kernel/
│   ├── src/mcp/
│   │   └── mod.rs      ← adapter (~60줄: re-export + ToolDef 변환)
│   ├── ... (나머지 동일)
│
├── oxios-ouroboros/    ← 변경 없음
├── oxios-markdown/     ← 변경 없음
└── oxios-gateway/      ← 변경 없음

channels/               ← 변경 없음
```

### 의존성 그래프 (변경 후)

```
oxios-mcp (독립, Oxios 의존 0)
    ↑
oxios-kernel (oxios-mcp에 의존)
    ↑
oxios-gateway → channels → oxios (binary)
```

---

## 9. 실행 단계

```
Step 1: crates/oxios-mcp/ 생성
  - client.rs, protocol.rs, mod.rs → lib.rs 복사
  - use crate::program::* 제거
  - to_tool_def() → getter 메서드로 교체
  - list_tools()가 Vec<McpTool> 반환하도록 변경
  - Cargo.toml 작성 (Oxios 의존성 없음)

Step 2: crates/oxios-kernel/src/mcp/ 교체
  - client.rs, protocol.rs 삭제
  - mod.rs를 adapter로 재작성 (re-export + 변환 함수)
  - Cargo.toml에 oxios-mcp 의존성 추가

Step 3: kernel_handle/mcp_api.rs 수정
  - list_tools → list_tool_defs
  - cached_tools → cached_tool_defs

Step 4: 검증
  - cargo build --workspace
  - cargo test --workspace
  - cargo clippy --workspace

Step 5: workspace Cargo.toml에 members 추가
  - "crates/oxios-mcp" 추가

예상 소요: 1-2시간
```
