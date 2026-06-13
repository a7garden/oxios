# RFC-023: MCP 기능을 oxi-sdk로 위임

| | |
|---|---|
| **상태** | 제안 (Proposed) |
| **작성일** | 2026-06-13 |
| **관련 버전** | oxi-sdk 0.33.0, oxios 1.3.0 |
| **선행 작업** | RFC-014 (oxi-sdk 0.26 migration)의 연장선 |
| **영향 크레이트** | `oxios-mcp` (폐기), `oxios-kernel`, 바이너리 `oxios` |

---

## 1. 요약

oxios가 자체 구현한 MCP 클라이언트(`crates/oxios-mcp/`)를 **폐기**하고, oxi-sdk 0.33.0이 제공하는 `McpManager`로 MCP 기능을 전적으로 위임한다. 단, **config는 oxios 전용 경로(`~/.oxios/`)에서만 읽고 oxi의 표준 발견 경로(`~/.config/oxi/`)는 건드리지 않는다**. 이는 이미 provider/credential에 적용 중인 패턴(engine.rs `CredentialStore`)과 대칭된다.

## 2. 배경 및 동기

### 2.1 oxi 0.33.0의 MCP 고도화

oxi-sdk 0.33.0이 pi-mcp-adapter 아키텍처 기반의 MCP 시스템을 `oxi_agent::mcp`에 탑재했고, 이를 SDK 소비자가 직접 사용 가능하다:

- **라이프사이클 관리**: `Lazy` / `Eager` / `KeepAlive` 모드 + mpsc 채널 기반 idle disconnect + keep-alive health check + 실패 백오프 (데드락 없는 `Arc::new_cyclic` + `Weak<McpManager>` 설계)
- **`McpTransport` trait**: stdio 구현체 제공, HTTP/SSE 확장 대비
- **표준 Content-Length 프레이밍**: Claude Desktop, Cursor, 공식 TS/Python SDK와 동일 포맷 (`Content-Length: N\r\n\r\n{...}`)
- **디스크 메타데이터 캐시**: 연결 없이도 search/list/describe 동작
- **Consent 시스템**: 툴별 Allow/Deny 사전 승인 (디스크 영속)
- **풀 프로토콜**: tools 외에 resources, prompts, sampling, logging, ping 지원
- **보안**: 위험 환경변수(`LD_PRELOAD`, `DYLD_*` 등) 차단

### 2.2 oxios 현재 구현의 한계

`crates/oxios-mcp/` (약 750 LOC)의 문제:

1. **프레이밍 결함**: JSONL(개행 구분)을 사용한다. Content-Length를 쓰는 표준 MCP 서버(Claude Desktop, Cursor, 공식 SDK 기반 대부분)와 **양방향으로 호환되지 않는다**. 공식 스펙 문서는 newline을 언급하나, 2025-06-18 changelog에 프레이밍 변경이 전혀 기록되지 않았고 배포 생태계는 Content-Length가 de-facto 표준이다.
2. **라이프사이클 없음**: 부팅 시 전체 일괄 연결 (`initialize_all()`). idle disconnect, health check, 자동 재연결 없음 → 서버 프로세스가 리소스 계속 점유.
3. **기능 누락**: 디스크 캐시, consent, resources/prompts/sampling 미지원.
4. **재구현**: AGENTS.md 핵심 원칙 — *"No reimplementation. Never reimplement what oxi-sdk provides"* — 위반. oxi가 이미 더 잘 구현한 것을 oxios가 유지보수할 이유가 없다.

### 2.3 config 경로 분리의 필요성

oxi의 모듈들은 `~/.config/oxi/`와 `~/.oxi/`에 config를 **하드코딩**한다:

| oxi 모듈 | 기본 경로 | oxios가 써야 하는가? |
|----------|-----------|---------------------|
| MCP config | `~/.config/oxi/mcp.json` | ❌ |
| MCP cache | `~/.config/oxi/mcp-cache.json` | ❌ |
| MCP consent | `~/.config/oxi/mcp-consent.json` | ❌ |
| auth store | `~/.oxi/auth.json` | ✅ (oxi CLI와 **공유**, 의도적) |
| catalog overrides | `~/.oxi/catalog/overrides.toml` | ⚠️ provider 영역 |

oxios는 `~/.oxios/config.toml`을 단일 진실 소스(single source of truth)로 쓴다. MCP server 정의, provider credential, lifecycle 모드가 모두 이 파일에 있어야 하며, oxi CLI의 `~/.config/oxi/mcp.json`을 **실수로 읽어버리면 안 된다**.

> **주의**: `~/.oxi/auth.json`은 예외적으로 공유한다. 이는 oxi CLI로 `oxi auth login` 한 credential을 oxios가 재사용하기 위함이며, `crates/oxios-kernel/src/credential.rs`에 이미 명시된 의도적 설계다.

## 3. 목표 / 비목표

### 목표

1. `oxios-mcp` 크레이트 폐기, oxi-sdk `McpManager`로 단일화
2. MCP 설정을 `~/.oxios/config.toml [mcp]`에서만 읽고 oxi 경로는 무시
3. cache/consent 디스크 파일을 `~/.oxios/` 아래로 배치
4. oxi의 lifecycle(lazy/eager/keep-alive), 캐시, consent, 풀 프로토콜 기능 확보
5. oxios `AccessManager`(RBAC + Merkle audit)를 oxi `ConsentManager` 위에 추가 보안 계층으로 통합
6. 기존 `OXIOS_MCP_*` 환경변수 DSL 호환성 유지
7. provider credential 패턴(`CredentialStore`)과 대칭되는 구조

### 비목표

- oxi CLI(`~/.config/oxi/`)의 동작 변경
- oxios만의 MCP 프레이밍 구현 유지 (Content-Length 표준으로 통일)
- HTTP/SSE transport 구현 (oxi Phase 5 이후 추진)
- `~/.oxi/auth.json` 경로 변경 (공유 스토어 유지)

## 4. 현재 아키텍처 (As-Is)

```
config.toml [mcp] ─┐
                   ├─→ init_mcp_bridge() ─→ McpBridge (oxios 자체)
OXIOS_MCP_* env ───┘                          ├─→ McpClient (oxios 자체, JSONL) ⚠️ 호환성 결함
                                              └─→ McpToolWrapper → AgentTool (mcp:{server}:{tool})

KernelHandle.mcp: McpApi(McpBridge)
부팅: kernel.init_mcp_servers() → McpBridge::initialize_all()  (전체 즉시 연결)
```

**문제점**: oxi 0.33.0의 `McpManager`와 기능이 중복되면서도 표준 호환성과 라이프사이클 관리가 뒤처진다.

## 5. 제안 아키텍처 (To-Be)

```
~/.oxios/config.toml [mcp] ──┐
                             ├─→ McpConfigBridge ─→ oxi_sdk::McpConfig (메모리 객체)
OXIOS_MCP_* env ─────────────┤                      ├─→ McpManager::spawn_with_paths()
                             │                      │     ├─→ McpClient (oxi, Content-Length) ✓ 표준
                             │                      │     ├─→ MetadataCache (~/.oxios/mcp-cache.json)
                             │                      │     ├─→ ConsentManager (~/.oxios/mcp-consent.json)
                             │                      │     └─→ lifecycle task (mpsc, idle/health)
                             │                      │
                             │                      └─→ ToolRegistry: McpDirectTool(x N) + McpTool(proxy)
                             │
~/.config/oxi/mcp.json ──────┘  ✗ 무시됨 (with_mcp_config 사용 → auto-discovery 비활성)

KernelHandle.mcp: McpApi(Arc<oxi_sdk::McpManager>) + AccessManager 보안 계층
부팅: 별도 init 불필요 — McpManager가 spawn 시점에 Eager/KeepAlive 자동 연결
```

### 5.1 config 계층 구조

```
~/.oxios/                          ← oxios 전용 (단일 진실 소스)
├── config.toml                    ← [mcp], [engine], providers 등 모든 설정
├── workspace/
├── knowledge/
├── mcp-cache.json      ← NEW (oxi MetadataCache 리다이렉트)
└── mcp-consent.json    ← NEW (oxi ConsentManager 리다이렉트)

~/.oxi/auth.json                   ← 공유 (oxi CLI + oxios) — credential.rs 정책 유지

~/.config/oxi/                     ← oxi CLI 전용 — oxios는 접근 금지
├── mcp.json
├── mcp-cache.json
├── mcp-consent.json
└── keys/
```

### 5.2 config 스키마 확장

`crates/oxios-kernel/src/config.rs`의 `[mcp]` 섹션을 oxi `ServerEntry` 필드를 모두 수용하도록 확장한다.

**현재 (oxios `McpServerDef`):**
```toml
[mcp.servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
env = {}
enabled = true
```

**확장 후:**
```toml
[mcp.servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
env = { DEBUG = "true" }
cwd = "/tmp"                          # NEW — 작업 디렉토리
debug = false                         # NEW — stderr를 상속(서버 로그 디버깅)

# NEW — 라이프사이클 (기본: lazy)
lifecycle = "eager"                   # "lazy" | "eager" | "keep-alive"
idle_timeout = 10                     # 분 단위 (idle 후 자동 연결 해제)

# NEW — 개별 툴 직접 노출 (AgentTool로 등록, mcp:{server}:{tool} 대신 prefixed 이름)
direct_tools = ["read_file", "write_file"]   # 또는 true/false (전체)
exclude_tools = ["dangerous_tool"]    # 제외할 툴

[mcp.settings]                        # NEW — 전역 MCP 설정
tool_prefix = "server"                # "server" | "short" | "none" (툴 이름 충돌 회피)
disable_proxy_tool = false            # mcp({}) proxy 툴 숨김 (direct_tools만 쓸 때)
failure_backoff_secs = 30             # 연결 실패 후 백오프
```

**enabled 필드 처리**: oxi `ServerEntry`엔 `enabled`가 없다. `enabled = false`인 서버는 변환 시 `McpConfig.mcp_servers`에서 **제외**한다 (존재하지 않으면 연결/노출되지 않음).

### 5.3 config 변환 브리지

새 모듈 `crates/oxios-kernel/src/mcp_bridge.rs` (oxi SDK 매핑 전용):

```rust
//! Oxios config → oxi_sdk::McpConfig 변환.
//! oxi의 자동 파일 발견(~/.config/oxi/mcp.json)에 의존하지 않고
//! oxios config.toml + OXIOS_MCP_* 환경변수에서만 구성한다.

use oxi_sdk::mcp::{McpConfig, ServerEntry, LifecycleMode, DirectToolsConfig, McpSettings, ToolPrefix};

/// oxios OxiosConfig.mcp → oxi McpConfig 변환.
pub fn build_oxi_mcp_config(oxios_config: &crate::config::McpConfig) -> McpConfig {
    let mut mcp = McpConfig::default();

    for (name, def) in &oxios_config.servers {
        if !def.enabled { continue; }   // 비활성 서버는 제외
        mcp.mcp_servers.insert(name.clone(), ServerEntry {
            command: Some(def.command.clone()),
            args: Some(def.args.clone()),
            env: Some(def.env.clone()),
            cwd: def.cwd.clone(),
            debug: def.debug,
            lifecycle: def.lifecycle.clone().map(Into::into),
            idle_timeout: def.idle_timeout,
            direct_tools: def.direct_tools.clone().map(Into::into),
            exclude_tools: def.exclude_tools.clone(),
        });
    }

    // OXIOS_MCP_* 환경변수 (기존 DSL 호환성)
    for (name, entry) in collect_oxios_mcp_env(&oxios_config.servers) {
        mcp.mcp_servers.entry(name).or_insert(entry);
    }

    mcp.settings = oxios_config.settings.as_ref().map(build_oxi_mcp_settings);
    mcp
}
```

> 자동 발견 경로 무시는 `OxiBuilder::with_mcp_config(cfg)` 사용으로 자동 보장된다. `builder.rs:504-510`에서 `Some(cfg)` 분기는 `spawn_with_config(cfg)`만 호출하고 `~/.config/oxi/mcp.json`을 읽지 않는다.

### 5.4 cache/consent 경로 문제 (oxi upstream 기여 필요)

**핵심 제약**: oxi `McpManager::spawn_with_config()`는 내부적으로 `MetadataCache::new()` / `ConsentManager::new()`를 호출하여 `~/.config/oxi/` 경로로 **하드코딩**한다. config 객체는 주입 가능하지만 디스크 경로는 불가하다.

`MetadataCache::with_path()` / `ConsentManager::with_path()`는 존재하나 `spawn_with_config`가 이걸 쓰지 않는다. `McpManager`의 모든 필드와 `lifecycle::channel()` / `lifecycle_event_loop`가 `pub`이긴 하나, `Arc::new_cyclic` + lifecycle spawn 로직을 oxios가 재구현하면 oxi 업데이트마다 깨진다.

**해결: oxi에 생성자 추가 기여** (a7garden/oxi — 같은 저자라 자연스러움):

```rust
// oxi-agent/src/mcp/mod.rs — 추가 제안
impl McpManager {
    /// spawn_with_config + 커스텀 cache/consent 경로.
    /// SDK 소비자(oxios)가 자체 디렉토리(~/.oxios/)를 쓸 수 있게 한다.
    pub fn spawn_with_paths(
        mcp_config: McpConfig,
        cache_path: PathBuf,
        consent_path: PathBuf,
    ) -> Arc<Self> {
        // spawn_with_config과 동일하되 cache/consent만 with_path로 생성
    }
}

// oxi-sdk/src/builder.rs — 추가 제안
impl OxiBuilder {
    pub fn with_mcp_paths(mut self, cache: PathBuf, consent: PathBuf) -> Self { ... }
}
```

**전환 전 임시 조치** (oxi 기여 병합 전까지):
- `[patch.crates-io]`로 oxi-agent를 로컬 path 패치 (Cargo.toml에 이미 주석 처리된 패치 활성화)
- 또는 cache/consent를 임시로 `~/.config/oxi/`에 두고 기여 병합 후 `~/.oxios/`로 이전

### 5.5 KernelHandle::McpApi 재구현

`crates/oxios-kernel/src/kernel_handle/mcp_api.rs`를 `McpBridge` → `oxi_sdk::McpManager` 래핑으로 교체:

```rust
pub struct McpApi {
    manager: Arc<oxi_sdk::mcp::McpManager>,
    access: Arc<parking_lot::Mutex<AccessManager>>,  // 보안 계층 추가
}

impl McpApi {
    pub async fn call_tool(&self, server: &str, tool: &str, args: Value)
        -> Result<McpCallResult>
    {
        // 1. oxios AccessManager RBAC + Merkle audit (기존 정책 유지)
        self.access.lock().audit(AuditAction::ToolCall { ... })?;

        // 2. oxi consent (Allow/Deny 디스크 정책)
        //    oxi McpManager::call_tool 내부에서 자동 적용

        // 3. oxi McpManager로 실행
        self.manager.call_tool(tool, args, Some(server)).await
    }
    // list_servers, list_tools, client_status 등 oxi manager API로 위임
}
```

oxios 기존 `McpApi`의 public 메서드 시그니처를 유지하여 `main.rs`, `supervisor.rs` 호출부 변경을 최소화한다. 차이점: `McpToolCallResult` → `oxi_sdk::mcp::McpCallResult`.

### 5.6 툴 등록 변경

`crates/oxios-kernel/src/tools/builtin/mod.rs::register_all_kernel_tools()`:

```rust
// AS-IS: 단일 McpToolWrapper (빈 이름으로 등록 — 사실상 비활성)
registry.register(McpToolWrapper::from_kernel(kernel, "", "", ...));

// TO-BE: oxi SDK 패턴 — mcp_tools() factory 또는 직접 등록
let mcp_manager = kernel.mcp.manager();   // Arc<oxi_sdk::McpManager>
for def in mcp_manager.direct_tools_from_cache() {
    registry.register(oxi_sdk::mcp::McpDirectTool::new(mcp_manager.clone(), def));
}
if !mcp_manager.should_disable_proxy() {
    registry.register(oxi_sdk::mcp::McpTool::new(mcp_manager.clone()));
}
```

`McpToolWrapper`(`crates/oxios-kernel/src/tools/mcp_tool.rs`)와 `mcp.rs`(oxios-mcp 재export + 변환)는 폐기. `mcp_tool_to_tool_def()` 변환 로직은 oxi가 이미 `McpToolDef` → `format_tool_name()`으로 처리하므로 불필요.

### 5.7 부팅 시퀀스 단순화

**AS-IS** (`src/kernel.rs`):
```rust
async fn init_mcp_bridge(config) -> McpBridge { ... }  // 폐기
pub async fn init_mcp_servers(&self) {
    self.mcp_bridge.initialize_all().await?;            // 전체 즉시 연결 (폐기)
}
```

**TO-BE**: `McpManager::spawn_with_paths()`가 이미 `start_eager_servers()`로 lifecycle mode에 따라 자동 연결/해제 관리. 별도 `init_mcp_servers()` 호출 불필요 — `OxiBuilder::build()` 또는 엔진 구성 시점에 McpManager 생성과 동시에 백그라운드 연결 시작.

## 6. 마이그레이션 계획 (4단계)

### Phase 1 — oxi-sdk 업그레이드 + 패치 준비
- `Cargo.toml`: `oxi-sdk = "0.33.0"`
- oxi upstream에 `spawn_with_paths` / `OxiBuilder::with_mcp_paths` PR
- 병합 전까지 `[patch.crates-io]`로 로컬 oxi path 패치

### Phase 2 — config 스키마 확장 + 변환 브리지
- `config.rs`: `McpServerDef`에 `cwd`, `debug`, `lifecycle`, `idle_timeout`, `direct_tools`, `exclude_tools` 추가
- `McpConfig`에 `settings: Option<McpSettings>` 추가
- `mcp_bridge.rs` 신규: `build_oxi_mcp_config()`
- 단위 테스트: TOML → `oxi_sdk::McpConfig` 직렬화 검증

### Phase 3 — 커널 교체
- `kernel_handle/mcp_api.rs`: `McpBridge` → `McpManager` 래핑
- `tools/builtin/mod.rs`: `McpToolWrapper` → `McpDirectTool` + `McpTool`
- `tools/mcp_tool.rs`, `mcp.rs` 폐기
- `src/kernel.rs`: `init_mcp_bridge`, `init_mcp_servers` 제거, McpManager를 엔진 구성에 통합
- `supervisor.rs`, `main.rs` 호출부 정리

### Phase 4 — oxios-mcp 크레이트 폐기
- `Cargo.toml` workspace members에서 제거
- `oxios-kernel/Cargo.toml`에서 `oxios-mcp` 의존성 제거
- crates.io에서 `oxios-mcp` 1.2.0을 마지막으로 deprecate (README에 "use oxi-sdk MCP instead" 표시)
- AGENTS.md "Release" 섹션 publish 순서에서 ① 그룹 제거

## 7. oxi upstream 기여 명세

| 추가 API | 위치 | 용도 |
|----------|------|------|
| `McpManager::spawn_with_paths(config, cache, consent)` | `oxi-agent/src/mcp/mod.rs` | 커스텀 디스크 경로 |
| `OxiBuilder::with_mcp_paths(cache, consent)` | `oxi-sdk/src/builder.rs` | SDK 소비자용 |
| `OxiBuilder::with_mcp_config` (기존) | — | config 객체 주입 (이미 존재) |

변경은 비침습적 — 기존 `spawn()` / `spawn_with_config()` 시그니처 유지, 새 생성자만 추가.

## 8. 트레이드오프 및 대안

### 채택: oxi 완전 위임 + config 경로 분리 (본 RFC)
- ✅ AGENTS.md "No reimplementation" 원칙 100% 준수
- ✅ 표준 Content-Length 호환성, lifecycle/캐시/consent/풀 프로토콜 확보
- ✅ provider 패턴(`CredentialStore`)과 대칭
- ⚠️ oxi upstream 기여 필요 (cache/consent 경로)
- ⚠️ `oxios-mcp` 크레이트 crates.io deprecate

### 기각: oxios 자체 구현 유지 + oxi 기능 이식 (이전 제안)
- ❌ 프레이밍이 비표준(JSONL) — 표준 서버와 호환 불가 (본질적 결함)
- ❌ "재구현 금지" 원칙 위반 지속
- ❌ oxi 업데이트를 수동 추적해야 함

### 기각: oxios-mcp + oxi McpManager 병행 (하이브리드)
- ❌ 두 MCP 경로 유지 → 복잡도 폭발, 디버깅 지옥

## 9. 파일 변경 요약

| 파일 | 변경 |
|------|------|
| `Cargo.toml` | `oxi-sdk = "0.33.0"`, workspace members에서 `oxios-mcp` 제거, `[patch.crates-io]` 임시 활성화 |
| `crates/oxios-kernel/Cargo.toml` | `oxios-mcp` 의존성 제거 |
| `crates/oxios-kernel/src/config.rs` | `McpServerDef`, `McpConfig` 확장 (lifecycle, direct_tools, settings) |
| `crates/oxios-kernel/src/mcp_bridge.rs` | **신규** — `build_oxi_mcp_config()` 변환 |
| `crates/oxios-kernel/src/kernel_handle/mcp_api.rs` | `McpManager` 래핑 + AccessManager 보안 계층 |
| `crates/oxios-kernel/src/tools/builtin/mod.rs` | `McpDirectTool` + `McpTool` 등록 |
| `crates/oxios-kernel/src/tools/mcp_tool.rs` | **폐기** |
| `crates/oxios-kernel/src/mcp.rs` | **폐기** (oxios-mcp 재export 제거) |
| `crates/oxios-kernel/src/lib.rs` | `pub mod mcp` 제거 |
| `src/kernel.rs` | `init_mcp_bridge`, `init_mcp_servers` 제거, 엔진 구성에 McpManager 통합 |
| `src/main.rs` | `init_mcp_servers()` 호출 제거 |
| `crates/oxios-mcp/` | **폐기** (crates.io deprecate) |
| `share/default-config.toml` | `[mcp.settings]`, lifecycle 필드 문서화 |
| `docs/ARCHITECTURE.md` | MCP 섹션 업데이트 |
| `AGENTS.md` | Release publish 순서 수정, "Two knowledge systems" 인접 항목 정리 |

## 10. 리스크

1. **oxi upstream 기여 지연**: `spawn_with_paths` PR이 늦어지면 Phase 1이 막힘. 완화: 로컬 patch로 즉시 시작, 병합 후 patch 제거.
2. **API breaking**: `McpApi` 시그니처 변경이 `main.rs`/web/CLI에 영향. 완화: 기존 메서드명 유지, 반환 타입만 `McpCallResult`로.
3. **crates.io 호환성**: `oxios-mcp`를 쓰는 외부 소비자(있다면) 영향. 현재 oxios-kernel만 path dep이므로 외부 영향 없을 것으로 추정 (사전 확인 필요).
4. **테스트 커버리지**: 실제 표준 MCP 서버(`@modelcontextprotocol/server-filesystem` 등)로 통합 테스트 추가 필수 — 기존 JSONL 결함이 교정되었는지 검증.

---

## 부록 A: 프레이밍 사실관계

공식 스펙 문서(modelcontextprotocol.io)는 2025-03-26/2025-06-18 모두 stdio를 "newline-delimited"로 서술하나, **2025-06-18 changelog에 전송 프레이밍 변경이 전혀 기록되지 않았다**. 배포된 생태계(Claude Desktop, Cursor, 공식 TS/Python SDK, pi-mcp-adapter)는 **전부 Content-Length**를 사용한다. oxios의 현재 JSONL 구현은 이 표준 서버들과 호환되지 않는다 — 본 RFC의 핵심 동기 중 하나.
