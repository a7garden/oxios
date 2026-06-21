# RFC-026: 크레이트 구조 재설계 — 애플리케이션 크레이트를 바이너리로 통합

> **Status:** Proposed
> **Created:** 2026-06-21
> **Supersedes:** AGENTS.md §Release (crates.io 배포 대상 변경)

## Problem

현재 oxios 워크스페이스는 12개 크레이트로 구성되어 있다. 이 중 7개는 재사용 가능한
코어 라이브러리이고, 5개(oxios-web, oxios-cli, oxios-telegram, oxios-bench, oxios
바이너리)는 애플리케이션 고유 코드다. 문제는 후자가 **재사용 불가능한 코드를 재사용
가능한 라이브러리처럼 패키징**하고 있다는 점이다.

### 구체적 증상

1. **oxios-web, oxios-cli, oxios-telegram은 어떤 크레이트도 import하지 않는다.**
   오직 `oxios` 바이너리만 사용한다. crates.io에 배포할 독립적 가치가 없다.

2. **oxi-sdk 버전 충돌.** 게시된 `oxios-kernel 1.5.1`은 `oxi-sdk 0.35.0`에,
   워크스페이스는 `oxi-sdk 0.37.1`에 묶여 있다. oxios-web을 배포하려 하면 두 버전의
   `oxi-ai`가 동시에 참조되어 컴파일 에러가 발생한다. (2026-06-21 발생)

3. **`surface/` vs `channels/` 인위적 구분.** Web은 `surface/`에, CLI/Telegram은
   `channels/`에 있다. 둘 다 "사용자 인터페이스"인데 폴더가 나뉘어 있다.

4. **바이너리가 비정상적으로 얇음.** `src/`는 5.6K LOC. 실제 애플리케이션 코드의
   대부분이 별도 크레이트에 분산되어 있다.

5. **React 프론트엔드(43K LOC TS)가 Rust 크레이트 내부에 위치.** oxios-web 크레이트의
   `web/` 디렉토리에 TypeScript 프로젝트 전체가 들어있다.

### 근본 원인

> **애플리케이션 고유 코드를 재사용 가능한 라이브러리로 오분류했다.**

Feature-gating은 별도 크레이트 없이도 module 수준에서 달성할 수 있다. `surface/`와
`channels/`의 분리는 아키텍처적 필요가 아니라 historical artifact다.

## Design Overview

### 핵심 원칙

> **`crates/` = 재사용 가능한 라이브러리. `src/` = oxios 바이너리(서버).**
> TS 프론트엔드는 루트 `web/`에 독립 프로젝트로 존재한다.

### 범위

이 RFC는 **크레이트 구조 재편**만을 다룬다. rust-embed 제거(에셋 서빙 방식 변경)는
런타임 동작에 영향을 미치는 별개 관심사이므로 **별도 RFC로 분리**한다. 이 설계에서는
rust-embed를 현행 유지한다.

### 제안 구조

```
oxios/
├── crates/                       재사용 가능한 라이브러리 (crates.io 배포)
│   ├── oxios-kernel/             48K LOC  — 에이전트 커널
│   ├── oxios-memory/             13K LOC  — 계층형 메모리
│   ├── oxios-markdown/           8K LOC   — 지식베이스
│   ├── oxios-ouroboros/          2K LOC   — 스펙 프로토콜
│   ├── oxios-calendar/           2K LOC   — 캘린더
│   ├── oxios-gateway/            2K LOC   — 메시지 라우팅
│   └── oxios-mcp/                1K LOC   — MCP 클라이언트
│
├── src/                          oxios 바이너리 (서버)
│   ├── main.rs                   진입점, CLI 파싱, 모듈 선언
│   ├── kernel.rs                 커널 어셈블러
│   ├── surface.rs                서피스 활성화 (기존 유지)
│   ├── web_dist.rs               프론트엔드 다운로드 (기존 유지)
│   ├── commands/                 CLI 서브커맨드 (기존 유지)
│   ├── otel.rs                   OpenTelemetry (기존 유지)
│   │
│   ├── api/                      ← was surface/oxios-web/src/ (HTTP API 서버)
│   │   ├── mod.rs                ← lib.rs 변환 (모듈 선언 + re-exports)
│   │   ├── server.rs             AppState
│   │   ├── plugin.rs             WebSurface 구현 + 정적 파일 서빙
│   │   ├── middleware.rs         Rate limiter
│   │   ├── bridge.rs             WebBridge (메시지 브릿지)
│   │   ├── error.rs              AppError
│   │   ├── format.rs             포매터
│   │   ├── api_docs.rs           OpenAPI/Swagger
│   │   ├── persona_routes.rs     페르소나 API
│   │   └── routes/               REST API 라우트 (20개 모듈)
│   │       ├── mod.rs
│   │       ├── chat.rs
│   │       ├── engine_routes.rs
│   │       ├── knowledge_routes.rs
│   │       ├── events.rs
│   │       └── ... (기존 파일 그대로)
│   │
│   └── channels/                 ← was channels/oxios-{cli,telegram}/
│       ├── mod.rs
│       ├── cli/                  ← was oxios-cli/src/ (7 files, 923 LOC)
│       │   ├── mod.rs
│       │   ├── channel.rs
│       │   ├── commands.rs
│       │   ├── format.rs
│       │   ├── interactive.rs
│       │   ├── plugin.rs
│       │   └── session.rs
│       └── telegram/             ← was oxios-telegram/src/ (3 files, 846 LOC)
│           ├── mod.rs
│           ├── format.rs
│           └── plugin.rs
│
├── web/                          ← was surface/oxios-web/web/ (React 프로젝트)
│   ├── package.json
│   ├── bun.lock
│   ├── vite.config.ts
│   ├── biome.json
│   ├── index.html
│   ├── public/
│   ├── src/                      43K LOC TypeScript
│   ├── e2e/
│   └── dist/                     빌드 산출물 (gitignored, CI가 생성)
│
├── share/                        기본 리소스 (config, skills)
├── benchmarks/                   벤치마크 (oxios-bench, 독립 크레이트)
├── docs/
└── tests/
```

### 사라지는 것

| 대상 | 운명 |
|------|------|
| `surface/` 디렉토리 | 삭제. `src/api/`로 흡수 |
| `channels/` 디렉토리 | 삭제. `src/channels/`로 흡수 |
| `oxios-web` 크레이트 | `src/api/` module로 흡수 |
| `oxios-cli` 크레이트 | `src/channels/cli/` module로 흡수 |
| `oxios-telegram` 크레이트 | `src/channels/telegram/` module로 흡수 |

### 유지되는 것

| 대상 | 이유 |
|------|------|
| 7개 코어 라이브러리 크레이트 | 재사용 가능, crates.io 배포 가치 있음 |
| `oxios-bench` | 독립 도구, oxios 의존성 없음 |
| Feature-gating | `#[cfg(feature = "web")]` module로 동일 동작 |
| rust-embed | 현행 유지. 에셋 서빙 방식은 별도 RFC |
| `Surface` / `Channel` trait | `oxios-gateway`에 정의, 그대로 사용 |
| `otel`, `embedding-gguf`, `browser` feature | 재편에 영향 없음 |

## Detailed Design

### 1. Cargo.toml 변경

#### Workspace members 축소

```toml
# Before (11 members)
[workspace]
members = [
    "crates/oxios-mcp",
    "crates/oxios-memory",
    "crates/oxios-calendar",
    "crates/oxios-kernel",
    "crates/oxios-markdown",
    "crates/oxios-ouroboros",
    "crates/oxios-gateway",
    "surface/oxios-web",
    "channels/oxios-cli",
    "channels/oxios-telegram",
    "benchmarks/oxios-bench",
]

# After (8 members)
[workspace]
members = [
    "crates/oxios-mcp",
    "crates/oxios-memory",
    "crates/oxios-calendar",
    "crates/oxios-kernel",
    "crates/oxios-markdown",
    "crates/oxios-ouroboros",
    "crates/oxios-gateway",
    "benchmarks/oxios-bench",
]
```

#### Features 재구성

optional crate dependency(`dep:oxios-web`) 대신 optional regular dependency로 전환.

```toml
# Before
[features]
default = ["web", "cli", "browser", "sqlite-memory"]
web = ["dep:oxios-web"]
cli = ["dep:oxios-cli"]
telegram = ["dep:oxios-telegram"]
browser = []
sqlite-memory = ["oxios-kernel/sqlite-memory"]
embedding-gguf = ["oxios-kernel/embedding-gguf"]
otel = ["oxios-kernel/otel", "dep:opentelemetry", "dep:opentelemetry_sdk"]

# After — 동일한 feature 이름, module 수준 cfg gate
[features]
default = ["web", "cli", "browser", "sqlite-memory"]
web = ["dep:axum", "dep:tower", "dep:tower-http", "dep:rust-embed",
       "dep:mime_guess", "dep:futures-util", "dep:tokio-stream",
       "dep:utoipa", "dep:utoipa-swagger-ui"]
cli = ["dep:reedline"]
telegram = []
browser = []
sqlite-memory = ["oxios-kernel/sqlite-memory"]
embedding-gguf = ["oxios-kernel/embedding-gguf"]
otel = ["oxios-kernel/otel", "dep:opentelemetry", "dep:opentelemetry_sdk"]
```

#### Dependencies 이관

oxios-web, oxios-cli의 의존성을 바이너리 `[dependencies]`로 이관. oxios-telegram은
의존성이 이미 바이너리에 존재 (`reqwest`, `tokio`, `serde` 등 workspace 공통).

```toml
[dependencies]
# ── web feature (was oxios-web deps) ──────────────────────────────
axum = { version = "0.8", features = ["ws"], optional = true }
tower = { version = "0.5", optional = true }
tower-http = { version = "0.6", features = ["cors"], optional = true }
rust-embed = { version = "8", features = ["mime-guess"], optional = true }
mime_guess = { version = "2", optional = true }
futures-util = { version = "0.3", optional = true }
tokio-stream = { version = "0.1", features = ["sync"], optional = true }
utoipa = { version = "5", features = ["axum_extras", "uuid", "chrono"], optional = true }
utoipa-swagger-ui = { version = "9", features = ["axum"], optional = true }

# ── cli feature (was oxios-cli deps) ──────────────────────────────
reedline = { version = "0.38", optional = true }

# ── telegram feature (was oxios-telegram deps) ────────────────────
# reqwest, tokio, serde, serde_json, async-trait, chrono, uuid, tracing
# → 이미 바이너리 [dependencies]에 존재. 추가 이관 불필요.

# ── 기존 의존성 (변경 없음) ───────────────────────────────────────
oxios-kernel = { version = "1.5.1", path = "crates/oxios-kernel", default-features = false }
oxios-calendar = { version = "1.5.1", path = "crates/oxios-calendar" }
oxios-markdown = { version = "1.5.1", path = "crates/oxios-markdown" }
oxios-ouroboros = { version = "1.5.1", path = "crates/oxios-ouroboros" }
oxios-gateway = { version = "1.5.1", path = "crates/oxios-gateway" }
oxi-sdk = { workspace = true }
# ... (나머지 기존 의존성 그대로)
```

#### `[patch.crates-io]` 변경 없음

현재 활성 패치는 `oxios-kernel = { path = "crates/oxios-kernel" }` 하나뿐이며,
oxios-web/cli/telegram 패치는 존재하지 않았다. 따라서 제거할 것이 없다.

#### `[dev-dependencies]` 병합

oxios-web의 `tempfile` dev-dep이 바이너리 dev-dep에 이미 존재하므로 추가 이관 불필요.

#### oxios-web/web 경로 이동에 따른 rust-embed 폴더 경로

```rust
// Before: surface/oxios-web/ 기준 → web/dist/
#[derive(Embed)]
#[folder = "web/dist/"]
struct EmbeddedAssets;

// After: 워크스페이스 루트 기준 → web/dist/ (경로 동일)
// rust-embed의 #[folder]는 Cargo.toml 위치 기준이므로
// web/ 이 루트로 이동하면 "web/dist/" 경로가 그대로 유효
```

### 2. Module 구조

#### `src/main.rs` — 모듈 선언 추가

```rust
// 기존 선언
mod commands;
mod kernel;
mod otel;
mod surface;
mod web_dist;

// 추가
#[cfg(feature = "web")]
mod api;

mod channels;  // 내부에서 #[cfg(feature = "cli/telegram")]으로 gate
```

#### `src/api/mod.rs` — lib.rs 변환

oxios-web의 `lib.rs`를 module 선언으로 변환. **부모에서 `#[cfg(feature = "web")]`로
gate하므로 내부 모듈은 개별 gate 불필요.**

```rust
//! Web dashboard HTTP API server.

pub mod api_docs;
pub mod bridge;
pub mod error;
pub mod format;
pub mod middleware;
pub mod persona_routes;
pub mod plugin;
pub mod routes;
pub mod server;

pub use bridge::{WebBridge, WebBridgeHandle};
pub use plugin::WebSurface;
pub use server::AppState;
```

#### `src/surface.rs` — import 경로 변경

```rust
// Before
#[cfg(feature = "web")]
let surfaces: Vec<Box<dyn Surface>> = vec![Box::new(oxios_web::WebSurface::new())];

// After
#[cfg(feature = "web")]
let surfaces: Vec<Box<dyn Surface>> = vec![Box::new(crate::api::WebSurface::new())];
```

#### `src/channels/mod.rs`

```rust
//! In-process communication channels.

#[cfg(feature = "cli")]
pub mod cli;

#[cfg(feature = "telegram")]
pub mod telegram;
```

#### `src/main.rs` — 채널 인스턴스화 경로 변경

```rust
// Before (line 25-28)
#[cfg(feature = "cli")]
use oxios_cli::CliPlugin;
#[cfg(feature = "telegram")]
use oxios_telegram::TelegramPlugin;

// After
#[cfg(feature = "cli")]
use crate::channels::cli::CliPlugin;
#[cfg(feature = "telegram")]
use crate::channels::telegram::TelegramPlugin;
```

```rust
// Before (line 1989-1994) — CLI interactive mode
let cli_channel = oxios_cli::CliChannel::new(256);
let handle = cli_channel.handle();
let mut loop_ = oxios_cli::InteractiveLoop::new(handle);

// After
let cli_channel = crate::channels::cli::CliChannel::new(256);
let handle = cli_channel.handle();
let mut loop_ = crate::channels::cli::InteractiveLoop::new(handle);
```

`build_channel_plugins()` (line 2903-2911)는 `CliPlugin::new()` / `TelegramPlugin::new()`
참조만 있고 이는 위 use 선언 변경으로 자동 해결됨.

### 3. `static/` 디렉토리 처리

`surface/oxios-web/static/`은 **Rust 코드에서 참조되지 않는다** (코드 검증 완료).
rust-embed 대상도 아니다 (`#[folder = "web/dist/"]`만 임베드).

내용 분석:

| 파일 | 실체 | 처리 |
|------|------|------|
| `Containerfile` | Docker/Container 빌드 정의 | 프로젝트 루트로 이동 |
| `default-config.toml` | `share/default-config.toml`과 동일 | 삭제 (중복) |
| `default-skills/` | `share/default-skills/`과 동일 | 삭제 (중복) |
| `default-programs/` | 과거 program.toml 형식 (RFC-009 이전) | 삭제 (사용 안 함) |
| `knowledge/` | files.md 정적 앱. `/knowledge/` 경로로 서빙됨 (middleware.rs:111) | `share/knowledge/`로 이동 |

`Containerfile`은 `COPY . .` 후 `cargo build --release -p oxios`를 수행하므로, `static/`
위치와 무관하게 작동한다.

### 4. crates.io 배포 변경

#### Before (수동 + CI 혼합, 불완전)

```
① oxios-markdown, oxios-mcp, oxios-ouroboros, oxios-memory     (CI publish.yml)
② oxios-calendar → oxios-markdown                               (CI publish.yml)
③ oxios-kernel → {ouroboros, markdown, calendar, mcp, memory}   (CI publish.yml)
④ oxios-gateway → oxios-kernel                                  (CI publish.yml)
⑤ oxios-web, oxios-cli, oxios-telegram, oxios, oxios-bench     (수동, 1.2.0에서 정체)
```

#### After (CI 전 자동, 9개)

```
① oxios-markdown, oxios-mcp, oxios-ouroboros, oxios-memory
② oxios-calendar → oxios-markdown
③ oxios-kernel → {ouroboros, markdown, calendar, mcp, memory}
④ oxios-gateway → oxios-kernel
⑤ oxios (binary) → {kernel, gateway, markdown, ouroboros, calendar}
⑥ oxios-bench (독립, oxios 의존성 없음)
```

- oxios-web/cli/telegram은 별도 크레이트가 아니므로 배포 불필요
- 동기화 대상 감소: 12 → 9. 버전 충돌 위험 **감소** (완전 제거 아님 — 커널의
  oxi-sdk 의존성은 별도 크레이트이므로 잔여 위험 존재)
- 바이너리(`oxios`)는 코어 크레이트가 모두 crates.io에 보인 후 배포

#### 바이너리 배포 시 `web/dist/` 제외

rust-embed가 `web/dist/`를 참조하므로, 바이너리 패키지에서 `web/` 디렉토리 자체를
제외해야 한다. 이미 `web/dist/`는 `.gitignore`에 있으므로, `cargo publish`는
git-tracked 파일만 포함한다. 단 `web/node_modules/` 등도 `.gitignore`에 있는지 확인.

### 5. CI/CD 변경

#### release.yml — web dist 빌드 경로

```yaml
# Before
- name: Build web assets
  working-directory: surface/oxios-web/web
  run: bun install && bun run build

- name: Package web assets
  run: |
    cd surface/oxios-web/web/dist
    zip -r $GITHUB_WORKSPACE/web-dist.zip .

# After
- name: Build web assets
  working-directory: web
  run: bun install && bun run build

- name: Package web assets
  run: |
    cd web/dist
    zip -r $GITHUB_WORKSPACE/web-dist.zip .
```

#### publish.yml — 배포 대상 확장

```yaml
# matrix에 oxios, oxios-bench 추가
matrix:
  crate:
    - oxios-markdown
    - oxios-mcp
    - oxios-ouroboros
    - oxios-memory
    - oxios-calendar
    - oxios-kernel
    - oxios-gateway
    - oxios        # ← 추가 (binary)
    - oxios-bench  # ← 추가 (독립)
```

바이너리는 워크스페이스 루트에 있으므로 `working-directory: crates/${{ matrix.crate }}`가
작동하지 않는다. `publish` step에서 경로 분기 필요:

```yaml
- name: Publish ${{ matrix.crate }}
  working-directory: ${{ matrix.crate == 'oxios' && '.' || format('crates/{0}', matrix.crate) }}
  env:
    CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_TOKEN || secrets.CARGO_REGISTRY_TOKEN }}
  run: |
    # ... (기존 publish 로직)
```

`WAIT` 매핑에 추가:

```
oxios)        WAIT="oxios-gateway" ;;
oxios-bench)  WAIT="" ;;  # 독립, 대기 불필요
```

#### package-check — 검증 대상 확장

```yaml
for crate in oxios-markdown oxios-mcp oxios-ouroboros oxios-memory \
             oxios-calendar oxios-kernel oxios-gateway \
             oxios oxios-bench; do
```

바이너리는 `cargo package -p oxios`로 패키징 검증.

## Migration Plan

> **중요:** Phase 2(Cargo.toml)와 Phase 3(import 경로)는 상호 의존한다.
> Phase 2 후 Phase 3 전에는 컴파일이 불가능하다. 두 Phase를 **한 단위로** 진행하고
> 완료 후 `cargo check`로 검증해야 한다.

### Phase 1: 파일 이동 (구조 변경만, 코드 수정 없음)

1. `surface/oxios-web/src/` → `src/api/` (git mv)
2. `surface/oxios-web/web/` → `web/` (git mv)
3. `channels/oxios-cli/src/` → `src/channels/cli/` (git mv)
4. `channels/oxios-telegram/src/` → `src/channels/telegram/` (git mv)
5. `surface/oxios-web/static/Containerfile` → 프로젝트 루트 (git mv)
6. `surface/oxios-web/static/knowledge/` → `share/knowledge/` (git mv)
7. `surface/oxios-web/static/` 나머지 (중복) 삭제
8. `surface/oxios-web/Cargo.toml`, `channels/oxios-cli/Cargo.toml`,
   `channels/oxios-telegram/Cargo.toml` 삭제
9. `surface/`, `channels/` 빈 디렉토리 삭제

### Phase 2-3: Cargo.toml 재구성 + Import 경로 수정 (동시 진행)

1. **Workspace members**에서 3개 크레이트 제거
2. **`[features]`** 재구성 (optional dep → optional regular dep)
3. **`[dependencies]`**에 web/cli 의존성 이관 (optional = true)
4. **`src/main.rs`**: `mod api;`, `mod channels;` 선언 추가
5. **`src/api/mod.rs`**: `lib.rs`를 module 선언으로 변환, `#![warn(missing_docs)]` 제거
   (바이너리는 공개 라이브러리가 아니므로)
6. **`src/surface.rs`**: `oxios_web::` → `crate::api::`
7. **`src/main.rs`**: `oxios_cli::` / `oxios_telegram::` → `crate::channels::cli::` /
   `crate::channels::telegram::`
8. **`src/api/`** 내부: `crate::xxx` (oxios-web 기준) → `crate::api::xxx` 참조 확인
9. `cargo check --workspace --all-features` 로 전수 검사

### Phase 4: 정리

1. `src/channels/cli/mod.rs`, `src/channels/telegram/mod.rs`: 각 crate의 `lib.rs`를
   module 선언으로 변환, `#![warn(missing_docs)]` 제거
2. `pub` 가시성 조정: 외부 크레이트가 없으므로 `pub(crate)` 권장
3. 통합 테스트 (`tests/`)에서 `oxios_web` / `oxios_cli` / `oxios_telegram` crate 이름
   참조 확인 및 수정
4. `.gitignore`에 `web/node_modules/`, `web/dist/` 포함 확인

### Phase 5: CI/CD 업데이트

1. `release.yml`: web dist 빌드 경로 변경 (`surface/oxios-web/web` → `web`)
2. `publish.yml`: matrix에 `oxios`, `oxios-bench` 추가, `working-directory` 분기
3. `ci.yml`: workspace 구조 변경 반영 (member 감소)

### Phase 6: 검증

```bash
# Feature 조합별 빌드
cargo build                                    # default (web, cli, browser, sqlite-memory)
cargo build --no-default-features --features cli          # web 없이
cargo build --no-default-features --features web          # cli 없이
cargo build --features telegram                           # telegram 추가
cargo build --all-features                                # 전체

# 테스트
cargo test --workspace

# 배포 검증
cargo publish -p oxios --dry-run              # 바이너리 패키징
cargo publish -p oxios-bench --dry-run        # 벤치마크 패키징

# 프론트엔드
cd web && bun install && bun run build        # 빌드 확인

# 수동 E2E
oxios --foreground                            # 데몬 시작
# → http://localhost:7878 접속 → 웹 UI 렌더링 → 채팅
# → oxios chat                                 # CLI 인터페이스
```

## Impact Analysis

### 의존성 그래프 단순화

```
Before:                              After:
  oxios binary                        oxios binary
  ├── oxios-web (crate)               ├── api (module, 같은 크레이트)
  │   ├── oxios-gateway               ├── channels (module, 같은 크레이트)
  │   ├── oxios-kernel                │   ├── cli
  │   └── oxi-sdk                     │   └── telegram
  ├── oxios-cli (crate)               ├── oxios-kernel (crate)
  │   └── oxios-gateway               ├── oxios-gateway (crate)
  ├── oxios-telegram (crate)          └── oxi-sdk
  │   └── oxios-gateway
  ├── oxios-kernel
  ├── oxios-gateway
  └── oxi-sdk
```

크레이트 수: 12 → 9 (7 라이브러리 + 바이너리 + bench)

### 크기 변화

| 항목 | Before | After |
|------|--------|-------|
| `src/` (binary) | 5.6K LOC (8 files) | ~20K LOC (~70 files) |
| Workspace members | 11 + binary | 7 + binary + bench |
| crates.io 배포 대상 | 7 (CI) + 5 (수동, 정체) | 9 (CI 전 자동) |

### 호환성

- **KernelHandle API**: 변경 없음. 모든 라이브러리 크레이트가 그대로 유지됨.
- **Surface/Channel trait**: 변경 없음. `oxios-gateway`에 정의됨.
- **Web API 엔드포인트**: 변경 없음. 모든 라우트가 그대로 이동됨.
- **rust-embed 동작**: 변경 없음. `#[folder = "web/dist/"]` 경로가 루트 이동 후에도 동일.
- **Config 파일 (`~/.oxios/config.toml`)**: 변경 없음.
- **`cargo install oxios`**: 여전히 동작. optional dep가 줄어 컴파일 시간 감소 기대.

### 위험

| 위험 | 완화 |
|------|------|
| 대량 파일 이동으로 git history 단절 | `git mv` 사용, `git log --follow`로 추적 가능 |
| Import 경로 누락 | Phase 2-3 통합 진행, `cargo check --all-features` 전수 검사 |
| CI publish 순서 오류 | publish.yml WAIT 매핑 + `working-directory` 분기 |
| oxi-sdk 버전 충돌 잔여 | 동기화 대상 12→9로 감소. 커널 재배포 시 워크스페이스 oxi-sdk 버전 확인 필요 |

## Alternatives Considered

### A. 현상 유지 (별도 크레이트)

**기각.** oxi-sdk 버전 충돌이 반복적으로 발생하며, 재사용 불가능한 코드를 별도
크레이트로 유지하는 비용이 이점을 초과한다.

### B. oxios-web만 바이너리로, CLI/Telegram은 별도 크레이트 유지

**기각.** 900 LOC짜리 크레이트 2개를 유지하는 것은 오버엔지니어링이다. Feature-gated
module로 동일한 격리를 달성할 수 있다.

### C. oxios-web의 Rust 코드를 oxios-kernel에 통합

**기각.** 커널은 전송 계층 무관(transport-agnostic)해야 한다. HTTP (axum, serde) 의존성이
커널에 들어가면 단일 책임 원칙 위반이다. AGENTS.md: "Kernel is intentionally monolithic" —
이는 *비즈니스 로직*의 통합을 의미하며, HTTP 계층 포함이 아니다.

## Out of Scope (별도 RFC)

### rust-embed 제거

현재 `plugin.rs`의 `EmbeddedAssets`는 `web/dist/`를 컴파일 타임에 임베드한다.
이는 런타임 GitHub Releases 다운로드의 폴백이다. 제거 시:

- 바이너리 크기 감소 (~20MB)
- `ActiveWebDist`가 `None`일 때 503 에러로 동작 변경
- crates.io 배포 시 TS 에셋 번들 문제 해결

이는 **런타임 동작 변경**이므로 구조 재편과 분리하여 별도 RFC로 다룬다.

## Open Questions

1. **바이너리 crates.io 배포 시 `web/dist/` 포함 여부.** rust-embed가 `web/dist/`를
   참조하므로, `cargo publish` 시 이 디렉토리가 존재해야 컴파일 가능. 하지만
   `web/dist/`는 `.gitignore`에 있으므로 git-tracked 파일만 포함하는 `cargo publish`에서
   자동 제외됨. 빌드 시 `web/dist/`가 없으면 rust-embed가 빈 디렉토리를 임베드하는지
   확인 필요 (별도 테스트).

2. **`share/knowledge/` 서빙 경로.** files.md 정적 앱이 `share/knowledge/`로 이동한 후,
   런타임에 어떤 경로에서 서빙하는가? 현재 `/knowledge/` prefix로 서빙되는데, 이 경로가
   `ActiveWebDist` 기반인지 별도 파일시스템 경로인지 확인 필요.

3. **`oxios-bench` 워크스페이스 디렉토리명.** 현재 `benchmarks/`이지만 Cargo 관례는
   `benches/`. 이 RFC와 무관하게 결정 가능.
