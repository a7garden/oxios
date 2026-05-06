# Oxios Production Roadmap — Full Design Document

> **버전:** 0.2.0-alpha → 1.0.0  
> **작성일:** 2026-05-06  
> **목표:** 프로덕션 등급 Agent OS로의 전환

---

## 목차

1. [Phase 1: 보안 하드닝](#phase-1-보안-하드닝-1-2주)
2. [Phase 2: 품질 & 테스트](#phase-2-품질--테스트-2-3주)
3. [Phase 3: DevOps & 인프라](#phase-3-devops--인프라-2-3주)
4. [Phase 4: 기능 완성도](#phase-4-기능-완성도-3-4주)
5. [TUI 설계](#tui-설계-ratatui)
6. [버그 수정 목록](#즉시-수정-버그-목록)

---

## Phase 1: 보안 하드닝 (1-2주)

### 1.1 API 인증 미들웨어

**문제:** HTTP API에 인증이 없음. 포트 4200에 도달 = 완전 관리자 권한.

**해결:** Bearer Token 기반 API 키 인증.

#### 설계

```
 crates/oxios-kernel/src/auth.rs          (신규)
 channels/oxios-web/src/middleware.rs      (신규)
```

**`auth.rs` — 인증 코어:**

```rust
/// Oxios API 인증 모듈
pub struct AuthManager {
    /// 활성 API 키 해시집합 (SHA-256)
    keys: HashSet<[u8; 32]>,
    /// 키 메타데이터 (이름, 생성일, 마지막 사용)
    key_meta: HashMap<[u8; 32], KeyMeta>,
}

pub struct KeyMeta {
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
}

impl AuthManager {
    /// 설정에서 인증 매니저 생성
    pub fn from_config(config: &SecurityConfig) -> Self;
    
    /// 새 API 키 생성 (접두사 `oxios_` + 32바이트 랜덤)
    pub fn generate_key(&mut self, name: &str) -> Result<String>;
    
    /// Bearer 토큰 검증
    pub fn validate(&self, token: &str) -> bool;
    
    /// 키 폐기
    pub fn revoke_key(&mut self, name: &str) -> Result<()>;
    
    /// 활성 키 목록 (메타데이터만, 키 자체는 노출 안함)
    pub fn list_keys(&self) -> Vec<&KeyMeta>;
}
```

**`middleware.rs` — Axum 미들웨어:**

```rust
use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};

/// Bearer Token 인증 미들웨어
pub async fn require_auth<B>(
    State(state): State<Arc<AppState>>,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    // 1. Authorization 헤더 추출
    let auth_header = req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    // 2. "Bearer <token>" 형식 검증
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    // 3. AuthManager에 검증 위임
    let auth = state.auth_manager.lock();
    if !auth.validate(token) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    // 4. 통과 → 다음 핸들러로
    Ok(next.run(req).await)
}
```

**설정 추가 (`config.toml`):**

```toml
[security]
# API 인증 활성화 (프로덕션에서는 필수)
auth_enabled = true
# API 키 파일 경로
api_keys_path = "~/.oxios/api-keys.json"
```

**CLI 통합:**

```bash
# 키 생성
oxios auth generate-key --name "my-laptop"

# 출력: oxios_a8f3e2... (이 토큰을 API 호출 시 사용)

# 키 목록
oxios auth list-keys

# 키 폐기
oxios auth revoke-key --name "my-laptop"
```

**적용 전략:**
- `auth_enabled = false` → 인증 스킵 (개발용, 기본값)
- `auth_enabled = true` → 모든 `/api/*` 경로에 미들웨어 적용
- `/health` 엔드포인트는 인증 제외

---

### 1.2 CORS 수정

**문제:** `CorsLayer::permissive()` — 모든 오리진 허용.

**해결:**

```rust
// server.rs / main.rs — CorsLayer 교체
use tower_http::cors::{CorsLayer, Any};

let cors = if config.security.auth_enabled {
    // 프로덕션: 명시적 오리진만 허용
    let allowed: Vec<&str> = config.security.cors_origins.iter()
        .map(|s| s.as_str())
        .collect();
    // TODO: parse origins properly
    CorsLayer::new()
        .allow_origin(
            config.security.cors_origins.iter()
                .filter_map(|o| o.parse::<Url>().ok())
                .collect::<Vec<_>>()
        )
        .allow_methods(Any)
        .allow_headers(Any)
} else {
    // 개발: localhost만 허용 (permissive 대신)
    CorsLayer::new()
        .allow_origin([
            "http://localhost:4200".parse().unwrap(),
            "http://127.0.0.1:4200".parse().unwrap(),
        ])
        .allow_methods(Any)
        .allow_headers(Any)
};
```

**설정 추가:**

```toml
[security]
cors_origins = ["http://localhost:4200"]
```

---

### 1.3 Health Endpoint

```rust
// routes.rs에 추가
.route("/health", get(handle_health))

async fn handle_health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": state.start_time.elapsed().as_secs(),
        "backend": {
            "container": state.container_manager.is_backend_available(),
            "agents": state.supervisor.list_agents().await.len(),
        }
    }))
}
```

**AppState에 추가:**

```rust
pub start_time: std::time::Instant,
```

---

### 1.4 Rate Limiting

```toml
# Cargo.toml에 추가
tower-governor = "0.4"
```

```rust
// middleware.rs에 추가
use tower_governor::{GovernorConfig, GovernorLayer};

/// 분당 60요청 제한 (설정 가능)
pub fn rate_limit_layer(config: &SecurityConfig) -> GovernorLayer {
    let gc = GovernorConfig::default()
        .per_second(config.rate_limit_per_second)
        .burst_size(config.rate_limit_burst);
    GovernorLayer { config: &gc }
}
```

---

### 1.5 입력 검증 미들웨어

```rust
// 입력 길이 제한
const MAX_CHAT_LENGTH: usize = 32_000;     // 채팅 메시지
const MAX_NAME_LENGTH: usize = 128;        // 리소스 이름
const MAX_PATH_DEPTH: usize = 16;          // 경로 깊이

fn validate_chat_request(req: &ChatRequest) -> Result<(), StatusCode> {
    if req.content.is_empty() || req.content.len() > MAX_CHAT_LENGTH {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.content.contains('\0') {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}
```

---

## Phase 2: 품질 & 테스트 (2-3주)

### 2.1 Typed Error 도입

**문제:** `thiserror`가 의존성에 있으나 사용 안됨. `anyhow` 전용.

**해결:** 커널 공개 API에 typed error, 애플리케이션은 `anyhow` 유지.

```
crates/oxios-kernel/src/error.rs    (신규)
```

```rust
use thiserror::Error;

/// Oxios 커널 에러 타입
#[derive(Debug, Error)]
pub enum KernelError {
    #[error("Agent {id} not found")]
    AgentNotFound { id: AgentId },

    #[error("Permission denied: {reason}")]
    PermissionDenied { reason: String },

    #[error("Container '{name}' is unavailable: {detail}")]
    ContainerUnavailable { name: String, detail: String },

    #[error("Container backend not available on this platform")]
    BackendUnavailable,

    #[error("Program '{name}' not found")]
    ProgramNotFound { name: String },

    #[error("Program '{name}' already installed")]
    ProgramAlreadyExists { name: String },

    #[error("Invalid configuration: {detail}")]
    InvalidConfig { detail: String },

    #[error("Seed '{id}' not found")]
    SeedNotFound { id: String },

    #[error("Session '{id}' not found")]
    SessionNotFound { id: String },

    #[error("State store error: {0}")]
    StateStore(#[from] std::io::Error),

    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}

// HTTP 상태 코드 매핑
impl KernelError {
    pub fn http_status(&self) -> axum::http::StatusCode {
        match self {
            Self::AgentNotFound(_) => StatusCode::NOT_FOUND,
            Self::PermissionDenied(_) => StatusCode::FORBIDDEN,
            Self::ContainerUnavailable { .. } => StatusCode::SERVICE_UNAVAILABLE,
            Self::BackendUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::ProgramNotFound(_) => StatusCode::NOT_FOUND,
            Self::ProgramAlreadyExists(_) => StatusCode::CONFLICT,
            Self::InvalidConfig(_) => StatusCode::BAD_REQUEST,
            Self::SeedNotFound(_) => StatusCode::NOT_FOUND,
            Self::SessionNotFound(_) => StatusCode::NOT_FOUND,
            Self::StateStore(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

// axum IntoResponse 구현
impl axum::response::IntoResponse for KernelError {
    fn into_response(self) -> axum::response::Response {
        let status = self.http_status();
        let body = serde_json::json!({
            "error": self.to_string(),
            "kind": format!("{:?}", self),
        });
        (status, axum::Json(body)).into_response()
    }
}
```

**적용 전략:**
- `lib.rs`에서 `pub mod error;` 추가, `pub use error::KernelError;`
- `supervisor.rs`, `container_manager.rs`, `program.rs` 등 공개 API에 `Result<T, KernelError>` 적용
- 내부 구현과 웹 핸들러는 계속 `anyhow` 사용 가능 (`KernelError::Internal` 로 래핑)

---

### 2.2 routes.rs 분할

**문제:** 1,798줄 단일 파일.

**해결:** 도메인별 모듈 분할

```
channels/oxios-web/src/routes/
├── mod.rs              (~50줄 — 라우터 조합)
├── chat.rs             (~150줄 — POST /api/chat, WebSocket)
├── control.rs          (~100줄 — status, agents, kill)
├── config_routes.rs    (~120줄 — config get/put)
├── workspace.rs        (~200줄 — tree, file get/put)
├── seeds.rs            (~150줄 — seeds list/get/evolution)
├── skills.rs           (~100줄 — skills CRUD)
├── memory.rs           (~80줄 — memory list/get)
├── gardens.rs          (~250줄 — gardens CRUD + exec)
├── scheduler.rs        (~60줄 — stats, tasks)
├── security.rs         (~120줄 — audit, permissions)
├── programs.rs         (~200줄 — programs CRUD)
├── host_tools.rs       (~40줄 — host tools check)
├── events.rs           (~60줄 — SSE stream)
├── sessions.rs         (~80줄 — sessions CRUD)
├── approvals.rs        (~100줄 — HitL approvals)
└── persona_routes.rs   (~220줄 — 기존 파일 이동)
```

**`routes/mod.rs`:**

```rust
use axum::routing::*;
use axum::Router;
use crate::server::AppState;
use std::sync::Arc;

pub mod chat;
pub mod control;
// ...

pub fn build_routes() -> Router<Arc<AppState>> {
    Router::new()
        .merge(chat::routes())
        .merge(control::routes())
        .merge(config_routes::routes())
        .merge(workspace::routes())
        .merge(seeds::routes())
        .merge(skills::routes())
        .merge(memory::routes())
        .merge(gardens::routes())
        .merge(scheduler::routes())
        .merge(security::routes())
        .merge(programs::routes())
        .merge(host_tools::routes())
        .merge(events::routes())
        .merge(sessions::routes())
        .merge(approvals::routes())
        .merge(persona_routes::routes())
        .route("/health", get(health_handler))
}
```

---

### 2.3 테스트 전략

#### Ouroboros 프로토콜 테스트

```
crates/oxios-ouroboros/tests/
├── protocol_test.rs    — Interview → Seed → Evaluate → Evolve 전체 흐름
├── seed_test.rs        — Seed 생성, 진화 계보, 모호성 점수
└── evaluation_test.rs  — 3단계 평가 (mechanical, semantic, consensus)
```

```rust
// protocol_test.rs — 핵심 테스트 예시
#[tokio::test]
async fn test_interview_reduces_ambiguity() {
    let engine = MockOuroborosEngine::new();
    let result = engine.interview("코드 리뷰해줘").await.unwrap();
    // 인터뷰 후 모호성이 감소해야 함
    assert!(result.ambiguity_score.ambiguity() < 0.5);
    // 질문이 최소 1개 이상 생성되어야 함
    assert!(!result.questions.is_empty());
}

#[tokio::test]
async fn test_seed_is_immutable() {
    let seed = Seed::new("goal-123", "목표", vec![], vec![]);
    let evolved = seed.evolve("개선된 목표", vec![]);
    // 원본 seed는 변하지 않음
    assert_eq!(seed.generation(), 0);
    assert_eq!(evolved.generation(), 1);
    assert_eq!(evolved.evolved_from(), Some(seed.id()));
}

#[tokio::test]
async fn test_evolution_improves_score() {
    // 낮은 점수 → evolve → 높은 점수
}
```

#### HTTP 라우트 테스트

```rust
// channels/oxios-web/tests/routes_test.rs
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

#[tokio::test]
async fn test_health_endpoint() {
    let app = create_test_app().await;
    let resp = app.oneshot(
        Request::builder().uri("/health").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_chat_requires_body() {
    let app = create_test_app().await;
    let resp = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/chat")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_unauthorized_with_auth_enabled() {
    let app = create_test_app_with_auth().await;
    let resp = app.oneshot(
        Request::builder().uri("/api/agents").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
```

---

### 2.4 init_kernel() 리팩토링 — Builder 패턴

**문제:** 16-튜플 반환, 중복 생성 버그.

**해결:**

```rust
/// Oxios 커널 빌더
pub struct Kernel {
    pub orchestrator: Arc<Orchestrator>,
    pub gateway: Gateway,
    pub event_bus: EventBus,
    pub state_store: Arc<StateStore>,
    pub container_manager: Arc<ContainerManager>,
    pub config: OxiosConfig,
    pub skill_store: SkillStore,
    pub supervisor: Arc<dyn Supervisor>,
    pub scheduler: Arc<AgentScheduler>,
    pub access_manager: Arc<Mutex<AccessManager>>,
    pub program_manager: Arc<ProgramManager>,
    pub host_tool_validator: HostToolValidator,
    pub persona_manager: PersonaManager,
    pub a2a_protocol: Arc<A2AProtocol>,
    pub mcp_bridge: Arc<Mutex<McpBridge>>,
}

impl Kernel {
    /// 커널 빌더 생성
    pub fn builder() -> KernelBuilder {
        KernelBuilder::default()
    }
}

#[derive(Default)]
pub struct KernelBuilder {
    config_path: Option<PathBuf>,
    model_id: Option<String>,
}

impl KernelBuilder {
    pub fn config_path(mut self, path: PathBuf) -> Self { self.config_path = Some(path); self }
    pub fn model_id(mut self, model: &str) -> Self { self.model_id = Some(model.to_string()); self }
    
    pub async fn build(self) -> Result<Kernel> {
        let config_path = self.config_path.unwrap_or_else(|| {
            expand_path("~/.oxios/config.toml")
        });
        let model_id = self.model_id.as_deref().unwrap_or("anthropic/claude-sonnet-4-20250514");
        
        // ... init_kernel 로직을 여기로 이동 ...
        // persona_manager는 한 번만 생성
        // a2a_protocol도 한 번만 생성
        
        Ok(Kernel { /* ... */ })
    }
}
```

**main.rs 사용:**

```rust
let kernel = Kernel::builder()
    .config_path(config_path)
    .model_id(default_model)
    .build()
    .await?;
```

---

## Phase 3: DevOps & 인프라 (2-3주)

### 3.1 CI 파이프라인

**`.github/workflows/ci.yml`:**

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  check:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
        with:
          path: oxios
      - uses: actions/checkout@v4
        with:
          repository: a7garden/oxi
          path: oxi
          ref: v0.5.0
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - name: Check formatting
        working-directory: oxios
        run: cargo fmt --all -- --check
      - name: Clippy
        working-directory: oxios
        run: cargo clippy --workspace -- -D warnings
      - name: Test
        working-directory: oxios
        run: cargo test --workspace
      - name: Audit
        working-directory: oxios
        run: |
          cargo install cargo-audit
          cargo audit

  release:
    needs: check
    if: startsWith(github.ref, 'refs/tags/v')
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
        with:
          path: oxios
      - uses: actions/checkout@v4
        with:
          repository: a7garden/oxi
          path: oxi
      - name: Build release
        working-directory: oxios
        run: cargo build --release
      - uses: actions/upload-artifact@v4
        with:
          name: oxios-macos-arm64
          path: oxios/target/release/oxios
```

### 3.2 Justfile (Makefile 대체)

```just
# Oxios 개발 명령어

# 기본: 빌드 + 테스트
default: build test

# 빌드
build:
    cargo build --workspace

# 릴리즈 빌드
release:
    cargo build --release

# 테스트
test:
    cargo test --workspace

# 클리피
lint:
    cargo clippy --workspace -- -D warnings

# 포맷
fmt:
    cargo fmt --all

# 포맷 체크
fmt-check:
    cargo fmt --all -- --check

# 감사
audit:
    cargo audit

# 전체 체크 (CI와 동일)
ci: fmt-check lint test audit

# 실행
run:
    cargo run

# 프론트엔드 빌드
frontend:
    cd channels/oxios-web/frontend && dx build --release

# 클린
clean:
    cargo clean
```

### 3.3 Metrics (Prometheus)

```toml
# Cargo.toml 추가
metrics = "0.24"
metrics-exporter-prometheus = "0.16"
```

```rust
// crates/oxios-kernel/src/metrics.rs (신규)
use metrics::{counter, gauge, histogram};

/// 커널 메트릭 등록
pub fn register_metrics() {
    counter!("oxios_agents_forked_total").increment(0);
    counter!("oxios_messages_processed_total").increment(0);
    counter!("oxios_orchestration_cycles_total").increment(0);
    gauge!("oxios_active_agents").set(0.0);
    gauge!("oxios_active_containers").set(0.0);
    histogram!("oxios_orchestration_duration_secs");
}
```

**routes.rs에 `/metrics` 엔드포인트:**

```rust
.route("/metrics", get(handle_metrics))

async fn handle_metrics() -> String {
    // Prometheus exporter가 제공하는 텍스트 포맷
    metrics_exporter_prometheus::encode_to_string()
        .unwrap_or_default()
}
```

### 3.4 oxi 의존성 관리

**현재 문제:** `path = "../oxi/oxi-ai"` — 빌드 재현 불가.

**해결:** Git 태그 기반 의존성

```toml
# Cargo.toml
[workspace.dependencies]
oxi-ai = { git = "https://github.com/a7garden/oxi", tag = "v0.5.0" }
oxi-agent = { git = "https://github.com/a7garden/oxi", tag = "v0.5.0" }
oxi-tui = { git = "https://github.com/a7garden/oxi", tag = "v0.5.0" }
```

또는 crates.io 배포 후:

```toml
oxi-ai = "0.5"
oxi-agent = "0.5"
oxi-tui = "0.5"
```

### 3.5 macOS 배포 자동화

**`scripts/install.sh`:**

```bash
#!/bin/bash
set -euo pipefail

echo "Installing Oxios Agent OS..."

# 바이너리 다운로드
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
    BINARY="oxios-macos-arm64"
else
    echo "Unsupported architecture: $ARCH"
    exit 1
fi

INSTALL_DIR="${HOME}/.oxios/bin"
mkdir -p "$INSTALL_DIR"

curl -sL "https://github.com/a7garden/oxios/releases/latest/download/${BINARY}" \
    -o "${INSTALL_DIR}/oxios"
chmod +x "${INSTALL_DIR}/oxios"

# PATH에 추가
if ! grep -q ".oxios/bin" "${HOME}/.zshrc" 2>/dev/null; then
    echo 'export PATH="$HOME/.oxios/bin:$PATH"' >> "${HOME}/.zshrc"
    echo "Added ~/.oxios/bin to PATH (restart shell or source ~/.zshrc)"
fi

# 워크스페이스 초기화
oxios status

echo "✅ Oxios installed successfully!"
echo "   Run: oxios"
```

---

## Phase 4: 기능 완성도 (3-4주)

### 4.1 Ouroboros execute() 실구현

```rust
// ouroboros_engine.rs — execute() 교체
async fn execute(&self, seed: &Seed) -> Result<ExecutionResult> {
    let start = std::time::Instant::now();
    
    // Seed의 steps를 Supervisor에 전달하여 실행
    // (Orchestrator가 직접 Supervisor를 호출하는 방식과 통합 필요)
    
    Ok(ExecutionResult {
        success: true,
        steps_completed: seed.steps().len(),
        steps_total: seed.steps().len(),
        output: Some("Executed via Supervisor".into()),
        duration: start.elapsed(),
    })
}
```

### 4.2 Audit Log 영속화

```rust
// access_manager.rs 수정
pub struct AccessManager {
    // ... 기존 필드 ...
    audit_log_path: Option<PathBuf>,
}

impl AccessManager {
    /// 감사 로그를 파일에 append
    fn persist_audit_entry(&self, entry: &AuditEntry) {
        if let Some(path) = &self.audit_log_path {
            let line = serde_json::to_string(entry).unwrap_or_default();
            // append to file (non-blocking: fire-and-forget)
            let path = path.clone();
            std::thread::spawn(move || {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                {
                    let _ = writeln!(f, "{}", line);
                }
            });
        }
    }
}
```

**설정:**

```toml
[security]
audit_log_path = "~/.oxios/workspace/audit.log"
audit_log_max_entries = 10000   # 인메모리 버퍼
```

### 4.3 Persona 영속성

```rust
// persona_store.rs 수정
pub struct PersonaStore {
    personas: RwLock<HashMap<String, Persona>>,
    path: PathBuf,  // ~/.oxios/workspace/personas.json
}

impl PersonaStore {
    /// 디스크에서 로드
    pub fn load(path: &Path) -> Result<Self>;
    
    /// 디스크에 저장 (CRUD 후 자동 호출)
    fn save(&self) -> Result<()>;
}
```

### 4.4 Config `set` 명령

```rust
// main.rs — ConfigAction::Set 구현
ConfigAction::Set { key, value } => {
    let mut config = load_config(&config_path)?;
    set_config_value(&mut config, &key, &value)?;
    let toml_str = toml::to_string_pretty(&config)?;
    std::fs::write(&config_path, toml_str)?;
    println!("Set {} = {}", key, value);
}

fn set_config_value(config: &mut OxiosConfig, key: &str, value: &str) -> Result<()> {
    match key.split('.').collect::<Vec<_>>().as_slice() {
        ["gateway", "port"] => config.gateway.port = value.parse().context("Invalid port")?,
        ["gateway", "host"] => config.gateway.host = value.to_string(),
        ["scheduler", "max_concurrent"] => config.scheduler.max_concurrent = value.parse()?,
        ["security", "network_access"] => config.security.network_access = value.parse()?,
        _ => bail!("Unknown config key: {}", key),
    }
    Ok(())
}
```

---

## TUI 설계 (ratatui)

### 개요

현재 CLI는 `clap` 서브커맨드 + `println!` 이고, 대시보드는 Dioxus 웹 프론트엔드뿐.  
로컬 개발자 경험을 위해 **ratatui 기반 TUI 레이어**를 추가.

**핵심 전략:** `oxi-tui` 위젯 재사용 + Oxios 전용 패널 추가

### 아키텍처

```
oxios (binary)
├── src/
│   ├── main.rs           — CLI 진입점 (기존)
│   ├── tui/               (신규 — TUI 레이어)
│   │   ├── mod.rs         — TUI 앱 진입점, 이벤트 루프
│   │   ├── app.rs         — OxiosApp 상태 관리
│   │   ├── panels/        — TUI 패널들
│   │   │   ├── mod.rs
│   │   │   ├── chat.rs        — 에이전트와 채팅 (oxi-tui::Chat 위젯 재사용)
│   │   │   ├── dashboard.rs   — 시스템 상태 대시보드
│   │   │   ├── agents.rs      — 에이전트 모니터링
│   │   │   ├── gardens.rs     — 컨테이너 가든 관리
│   │   │   ├── seeds.rs       — Ouroboros Seed 열람
│   │   │   ├── logs.rs        — 실시간 이벤트 스트림
│   │   │   └── programs.rs    — 프로그램 관리
│   │   └── theme.rs       — Oxios 테마 (oxi-tui::Theme 확장)
│   └── ...
```

**Cargo.toml 의존성 추가:**

```toml
[dependencies]
# ... 기존 ...
oxi-tui = { workspace = true }  # 또는 git dependency
crossterm = "0.28"
ratatui = "0.30"
```

### TUI 앱 흐름

```
┌─ Oxios Agent OS ──────────────────────────────────────┐
│  [1:Chat] [2:Dashboard] [3:Agents] [4:Gardens] [5:Logs│
│ ───────────────────────────────────────────────────── │
│                                                        │
│  (현재 선택된 패널의 콘텐츠)                            │
│                                                        │
│                                                        │
│                                                        │
│                                                        │
│ ───────────────────────────────────────────────────── │
│  🟢 3 agents │ 🟢 2 gardens │ CPU 12% │ Mem 340MB    │
│  > _                                                   │
└───────────────────────────────────────────────────────┘
```

### `app.rs` — 핵심 상태 관리

```rust
use ratatui::Frame;
use crossterm::event::{KeyEvent, KeyCode};

/// Oxios TUI 애플리케이션
pub struct OxiosApp {
    /// 현재 활성 패널
    active_panel: Panel,
    /// 패널 목록
    panels: Vec<Box<dyn Panel>>,
    /// 입력 모드 (명령 vs 채팅)
    input_mode: InputMode,
    /// 커널 참조 (API 클라이언트)
    kernel: KernelHandle,
    /// 종료 플래그
    should_quit: bool,
}

#[derive(Clone, Copy)]
enum Panel {
    Chat,
    Dashboard,
    Agents,
    Gardens,
    Seeds,
    Logs,
    Programs,
}

enum InputMode {
    /// 명령어 입력 모드 (숫자 키로 패널 전환, `:`으로 명령)
    Normal,
    /// 채팅 입력 모드
    Insert,
    /// 명령 팔레트 (oxi-tui::CommandPalette 재사용)
    Command,
}

impl OxiosApp {
    pub fn new(kernel: KernelHandle) -> Self { /* ... */ }
    
    /// 이벤트 처리
    pub fn handle_event(&mut self, event: &crossterm::event::Event) {
        match event {
            crossterm::event::Event::Key(key) => self.handle_key(key),
            crossterm::event::Event::Resize(w, h) => { /* 리사이즈 */ }
            _ => {}
        }
    }
    
    /// 렌더링
    pub fn render(&self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),   // 탭 바
                Constraint::Min(10),     // 메인 콘텐츠
                Constraint::Length(2),   // 상태 바 + 입력
            ])
            .split(frame.area());
        
        self.render_tabs(frame, chunks[0]);
        self.panels[self.active_panel].render(frame, chunks[1]);
        self.render_status_bar(frame, chunks[2]);
    }
}
```

### `panels/chat.rs` — 에이전트 채팅

```rust
use oxi_tui::widgets::chat::ChatWidget;

pub struct ChatPanel {
    /// oxi-tui의 Chat 위젯 재사용
    widget: ChatWidget,
    /// 세션 ID
    session_id: Option<String>,
}

impl PanelTrait for ChatPanel {
    fn render(&self, frame: &mut Frame, area: Rect) {
        self.widget.render(frame, area);
    }
    
    fn handle_key(&mut self, key: &KeyEvent) -> Action {
        match key.code {
            KeyCode::Enter => {
                let msg = self.widget.take_input();
                // 커널에 메시지 전송 (비동기 채널)
                Action::SendMessage(msg)
            }
            _ => {
                self.widget.handle_key(key);
                Action::None
            }
        }
    }
}
```

### `panels/dashboard.rs` — 시스템 대시보드

```rust
pub struct DashboardPanel {
    stats: DashboardStats,
}

struct DashboardStats {
    active_agents: usize,
    active_gardens: usize,
    total_seeds: usize,
    total_programs: usize,
    events_per_minute: f64,
}

impl PanelTrait for DashboardPanel {
    fn render(&self, frame: &mut Frame, area: Rect) {
        // 상단: 스탯 카드 (3x2 그리드)
        // 하단: 최근 이벤트 로그 (5줄)
        
        let cards = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(20),
                Constraint::Length(20),
                Constraint::Length(20),
            ])
            .split(area);
        
        Self::render_stat_card(frame, cards[0], "Agents", self.stats.active_agents);
        Self::render_stat_card(frame, cards[1], "Gardens", self.stats.active_gardens);
        Self::render_stat_card(frame, cards[2], "Seeds", self.stats.total_seeds);
    }
}
```

### CLI와 TUI 통합

**main.rs 수정:**

```rust
// 서브커맨드 추가
#[derive(Debug, Subcommand)]
enum Command {
    // ... 기존 ...
    
    /// Launch interactive TUI dashboard.
    Tui,
}

// None (기본) → TUI 모드로 실행
match cli.command {
    Some(Command::Tui) | None => {
        // 커널 초기화 후 TUI 실행
        let kernel = Kernel::builder()
            .config_path(config_path)
            .model_id(default_model)
            .build()
            .await?;
        
        // 백그라운드에서 웹 서버도 함께 실행
        let web_handle = spawn_web_server(&kernel).await?;
        
        // TUI 실행 (포그라운드)
        let tui = oxios::tui::OxiosApp::new(kernel.handle());
        tui.run().await?;
        
        web_handle.abort();
    }
    
    // 다른 서브커맨드는 기존처럼 CLI 모드
    Some(Command::Run { prompt }) => { /* ... */ }
    // ...
}
```

### 커널-프론트엔드 통신

TUI와 커널 사이의 통신은 `mpsc` 채널 사용:

```rust
/// 커널 핸들 (TUI에서 사용)
pub struct KernelHandle {
    /// 채팅 메시지 전송
    chat_tx: mpsc::Sender<ChatRequest>,
    /// 채팅 응답 수신
    chat_rx: mpsc::Receiver<ChatResponse>,
    /// 시스템 상태 폴링
    stats_tx: mpsc::Sender<StatsRequest>,
    stats_rx: watch::Receiver<DashboardStats>,
    /// 이벤트 스트림
    events_rx: broadcast::Receiver<KernelEvent>,
}
```

---

## 즉시 수정 버그 목록

다음은 Phase와 무관하게 **지금 당장** 수정해야 할 버그:

### BUG-1: 중복 persona_manager (🔴 Critical)

**위치:** `main.rs:255, 332`  
**문제:** `init_kernel()`에서 `PersonaManager::new()`를 두 번 호출. 첫 번째는 persona를 설정하지만 두 번째(반환값)는 빈 상태.  
**영향:** 웹 API `/api/personas`가 빈 목록 반환.  
**수정:** 하나만 생성해서 양쪽에 사용.

```rust
// BEFORE (버그):
let persona_manager = PersonaManager::new();
// ... persona 설정 로직 ...
let persona_manager = PersonaManager::new();  // ← 빈 인스턴스로 덮어씀

// AFTER (수정):
let persona_manager = PersonaManager::new();
if let Some(p) = persona_manager.first_enabled() {
    ouroboros.set_persona_prompt(Some(p.system_prompt));
}
// persona_manager를 그대로 반환
```

### BUG-2: 중복 a2a_protocol (⚪ Minor)

**위치:** `main.rs:264, 267`  
**수정:** 한 번만 생성.

### BUG-3: 중복 `/api/events` 라우트 (🔴 Critical)

**위치:** `routes.rs:110, 128`  
**문제:** `/api/events`가 두 번 등록됨. Axum에서 동일 경로 중복 등록 시 패닉 가능.  
**수정:** 두 번째 등록 제거.

### BUG-4: `serde_json::to_value().unwrap()` (🟡 Medium)

**위치:** `routes.rs:1381`  
**수정:** `?` 또는 `map_err`로 교체.

### BUG-5: 5개 production `unwrap()` (🟡 Medium)

**위치:** `agent_runtime.rs:83-84`, `program.rs:295, 360, 448`  
**수정:** `context()` 체인으로 교체.

---

## 마일스톤 요약

| Phase | 기간 | 산출물 | 프로덕션 등급 |
|-------|------|--------|--------------|
| **Phase 1** | 1-2주 | 인증, CORS, Health, Rate Limit, 버그 수정 | 프라이빗 베타 가능 |
| **Phase 2** | 2-3주 | Typed errors, routes 분할, 테스트 100+ 추가 | 내부 QA 가능 |
| **Phase 3** | 2-3주 | CI/CD, Metrics, Justfile, 배포 스크립트 | 스테이징 배포 가능 |
| **Phase 4** | 3-4주 | execute 실구현, 영속성, config set | **퍼블릭 베타** |
| **TUI** | 2-3주 | ratatui 대시보드, 채팅, 모니터링 | **v1.0.0 출시** |

**총 예상 기간:** 10-14주 (TUI 포함 시)

---

## 우선순위 정리

```
1. [지금] BUG-1, BUG-2, BUG-3 수정 (1일)
2. [이번 주] Phase 1 — 보안 하드닝
3. [다음 주] Phase 2 시작 — typed errors + routes 분할
4. [3주차] Phase 2 완료 — 테스트 + Phase 3 시작
5. [5주차] Phase 3 완료 — CI/CD + Metrics
6. [6주차] Phase 4 + TUI 설계 착수
7. [10주차] v1.0.0 릴리즈
```
