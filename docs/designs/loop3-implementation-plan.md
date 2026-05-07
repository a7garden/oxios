# Loop 3 — 보안/버그 수정 + 품질 개선 설계

> 평가 결과 61/100점. P0(보안/버그) 4건, P1(안정성) 4건, P2(아키텍처) 4건.
> 각 항목에 대해 문제 원인 → 해결 설계 → 수정 위치 → 영향 범위를 명시.

---

## Step 0: P0 보안 수정 (4건)

### 0-1. Path Traversal — workspace tree

**문제:** `GET /api/workspace/tree?dir=../../../etc`가 base path를 벗어남.

**원인:** `dir` 파라미터를 `base.join(d)` 후 canonicalize 없이 `read_dir`에 전달.

**해결:** `handle_workspace_file_get`과 동일한 패턴 적용.

```rust
// AS-IS
let dir = match &query.dir {
    Some(d) => base.join(d),
    None => base.clone(),
};

// TO-BE
let canonical_base = state.state_store.base_path.canonicalize()
    .unwrap_or_else(|_| state.state_store.base_path.clone());
let dir = match &query.dir {
    Some(d) => {
        let candidate = base.join(d);
        let canonical = candidate.canonicalize()
            .map_err(|_| StatusCode::NOT_FOUND)?;
        if !canonical.starts_with(&canonical_base) {
            return Err(StatusCode::FORBIDDEN);
        }
        canonical
    }
    None => canonical_base,
};
```

**수정 파일:** `channels/oxios-web/src/routes.rs` — `handle_workspace_tree` (~L442)

---

### 0-2. Path Traversal — workspace file PUT

**문제:** `create_dir_all` 후 canonicalize 실패 시 순회 검증 스킵 → `write` 실행.

**원인:** `if let Ok(canonical_parent)` 브랜치가 실패하면 폴스루되어 검증 없이 쓰기.

**해결:** canonicalize는 항상 성공해야 함. 실패 시 에러 반환.

```rust
// AS-IS
if parent.canonicalize().is_err()
    && tokio::fs::create_dir_all(parent).await.is_err()
{
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
}
if let Ok(canonical_parent) = parent.canonicalize() {
    if !canonical_parent.starts_with(&canonical_base) {
        return Err(StatusCode::FORBIDDEN);
    }
}
// ← 여기서 검증 없이 write 실행됨

// TO-BE
if !parent.exists() {
    tokio::fs::create_dir_all(parent).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
}
let canonical_parent = parent.canonicalize()
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
if !canonical_parent.starts_with(&canonical_base) {
    return Err(StatusCode::FORBIDDEN);
}
```

**수정 파일:** `channels/oxios-web/src/routes.rs` — `handle_workspace_file_put` (~L501)

---

### 0-3. Auth middleware 정적 파일 우회

**문제:** `path.ends_with(".js")`로 끝나는 모든 요청이 인증 스킵.
`/api/agents?.js` 같은 공격으로 API 전체 우회 가능.

**원인:** URI path 접미사 매칭은 쿼리스트링/경로 조작에 취약.

**해결:** 실제 정적 자산 경로를 prefix로 매칭.

```rust
// AS-IS
if path.starts_with("/dioxus")
    || path.ends_with(".js")
    || path.ends_with(".css")
    || path.ends_with(".html")
{
    return Ok(next.run(request).await);
}

// TO-BE
// 정적 자산은 /assets/, /dioxus/ 하위만 허용
let static_prefixes = ["/assets/", "/dioxus/", "/favicon"];
let is_static = static_prefixes.iter().any(|p| path.starts_with(p))
    || path == "/" || path == "/index.html";
if is_static {
    return Ok(next.run(request).await);
}
```

**수정 파일:** `channels/oxios-web/src/middleware.rs` — `require_auth` (~L38)

**추가:** `routes.rs`에서 `build_routes`에 `DefaultBodyLimit::max(10 * 1024 * 1024)` 레이어 추가.

---

### 0-4. MCP 등록 버그

**문제:** `Arc::get_mut(&mut mcp_bridge.clone())` — clone의 임시 값에 get_mut.
원본 mcp_bridge는 변경되지 않음 → MCP 서버가 브릿지에 등록되지 않음.

**원인:** `McpBridge::register_server`가 `&mut self`를 요구하지만, 이미 `Arc`로 공유된 상태.

**해결:** McpBridge 내부에 `tokio::sync::RwLock<Vec<McpServer>>`를 두어 `&self`로 등록 가능하게 변경.

```rust
// mcp.rs — McpBridge 내부 변경
pub struct McpBridge {
    servers: RwLock<Vec<McpServer>>,  // Vec<McpServer> → RwLock<Vec<McpServer>>
    // ...나머지 필드
}

impl McpBridge {
    pub fn register_server(&self, server: McpServer) {  // &mut self → &self
        // self.servers.push(server); →
        self.servers.write().blocking_push(server);  // 또는 async fn으로
    }

    pub async fn initialize_all(&self) -> Result<()> {  // &mut self → &self
        let servers = self.servers.read().await;
        // ...
    }
}
```

agent_runtime.rs에서는:

```rust
// AS-IS
if let Some(bridge_mut) = Arc::get_mut(&mut mcp_bridge.clone()) {
    bridge_mut.register_server(server);
}

// TO-BE
mcp_bridge.register_server(server);  // 그냥 호출
```

**수정 파일:**
- `crates/oxios-kernel/src/mcp.rs` — McpBridge 내부 동기화
- `crates/oxios-kernel/src/agent_runtime.rs` — Arc::get_mut 제거

---

## Step 1: P1 안정성 수정 (4건)

### 1-1. WebSocket 인증 + 사용자 분리

**문제:**
1. WebSocket에 인증 없음 (`"ws_user"` 하드코딩)
2. 모든 WS 클라이언트가 모든 메시지 수신 (데이터 누출)

**해결:**

**(A) WS 업그레이드 시 토큰 검증:**
쿼리 파라미터로 토큰 전달 (WebSocket은 커스텀 헤더 불가).

```rust
async fn handle_chat_stream(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Query(params): Query<ChatStreamParams>,
) -> impl IntoResponse {
    // 인증 검증
    if state.config.security.auth_enabled {
        let token = params.token.as_deref().unwrap_or("");
        let valid = { state.auth_manager.lock().validate(token) };
        if !valid {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }

    ws.on_upgrade(move |socket| handle_chat_websocket(
        socket,
        state.0,
        params.user_id.unwrap_or_else(|| "web_user".into()),
    ))
}

#[derive(Deserialize)]
struct ChatStreamParams {
    token: Option<String>,
    user_id: Option<String>,
}
```

**(B) 메시지에 user_id 필드 추가하여 필터링:**

```rust
// OutgoingMessage에 target_user 필드 추가
pub struct OutgoingMessage {
    pub channel: String,
    pub user_id: String,        // 송신자
    pub target_user: Option<String>,  // None = broadcast
    pub response: String,
    // ...
}

// WS 핸들러에서 필터링
let user_id = user_id.clone();
let recv_task = tokio::spawn(async move {
    while let Ok(msg) = outgoing_rx.recv().await {
        // 자기 메시지 또는 broadcast만 전달
        if msg.target_user.as_deref() != Some(&user_id)
            && msg.target_user.is_some() {
            continue;
        }
        // ...
    }
});
```

**수정 파일:**
- `channels/oxios-web/src/routes.rs` — WS 핸들러
- `crates/oxios-gateway/src/message.rs` — OutgoingMessage에 target_user 추가
- `crates/oxios-gateway/src/channel.rs` — deliver_response 시 target_user 설정

---

### 1-2. 파일 업로드 크기 제한

**문제:** `body: String`에 크기 제한 없음 → 메모리 소진.

**해결:** Axum 라우터에 기본 바디 제한 설정.

```rust
// routes.rs — build_routes 함수에 추가
use axum::extract::DefaultBodyLimit;

pub fn build_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // ... 모든 라우트
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))  // 10MB
}
```

**수정 파일:** `channels/oxios-web/src/routes.rs` — `build_routes`

---

### 1-3. `expect()` 제거

**문제:** `server.rs`에 3개의 `expect()` → 패닉으로 프로세스 종료.

**해결:** `WebServer::new()`를 `Result` 반환으로 변경.

```rust
// AS-IS
impl WebServer {
    pub fn new(...) -> Self {
        let addr: SocketAddr = format!("{host}:{port}")
            .parse()
            .expect("Invalid bind address");
        // ...
    }
}

// TO-BE
impl WebServer {
    pub fn new(...) -> Result<Self, anyhow::Error> {
        let addr: SocketAddr = format!("{host}:{port}")
            .parse()
            .context("Invalid bind address")?;
        // ...
        Ok(Self { ... })
    }
}
```

호출부(`main.rs`, `server.rs`)에서 `?` 전파.

**수정 파일:**
- `channels/oxios-web/src/server.rs` — `new()` 반환타입 + `expect` 3곳
- `src/main.rs` — 호출부 에러 처리

---

### 1-4. 에러 응답 표준화

**문제:** `Result<T, StatusCode>`와 `Result<T, (StatusCode, String)>` 혼재.

**해결:** 통일된 `AppError` 타입 도입.

```rust
// channels/oxios-web/src/error.rs (신규)
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

pub enum AppError {
    NotFound(String),
    BadRequest(String),
    Internal(String),
    Unauthorized(String),
    Forbidden(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            AppError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
            AppError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m),
            AppError::Forbidden(m) => (StatusCode::FORBIDDEN, m),
        };
        let body = json!({ "error": message });
        (status, axum::Json(body)).into_response()
    }
}
```

이후 모든 핸들러를 `Result<Json<T>, AppError>`로 통일.

**수정 파일:**
- `channels/oxios-web/src/error.rs` — 신규
- `channels/oxios-web/src/lib.rs` — `mod error;`
- `channels/oxios-web/src/routes.rs` — 핸들러 에러 타입 교체 (점진적)

---

## Step 2: P2 아키텍처/품질 (4건)

### 2-1. Dead code 정리

**문제:** 구현되었으나 사용되지 않는 코드가 유지보수 부담.

**정리 대상:**

| 모듈 | 현황 | 조치 |
|------|------|------|
| `context_manager.rs` | 완전 구현, 0 참조 | `#[cfg(feature = "context-mgr")]` 게이트 또는 제거 |
| `RbacManager` | `AccessManager` 내부에 구현, 외부 호출 0 | 동일 |
| `Supervisor::exec()` | 인터페이스만 있고 호출 0 | 제거 (run_with_seed가 대체) |
| `resolve_provider_and_model()` | `#[allow(dead_code)]` | 제거 (EngineProvider가 대체) |
| `OuroborosEngine::execute()` | 항상 `success: false` 반환 | 문서화: "Orchestrator가 직접 실행" |
| `reqwest` 의존성 | Cargo.toml에 있으나 미사용 | 제거 |

**수정 파일:** 각 모듈 + `Cargo.toml`

---

### 2-2. Orchestrator 책임 분리

**문제:** `handle_message`가 300줄, 7개 서브시스템 직접 호출.

**해결:** 3단계로 분리.

**Phase A — AgentLifecycleManager 추출:**

```rust
// crates/oxios-kernel/src/agent_lifecycle.rs (신규)
pub struct AgentLifecycleManager {
    supervisor: Arc<dyn Supervisor>,
    scheduler: Arc<AgentScheduler>,
    access_manager: Arc<PLMutex<AccessManager>>,
    a2a: Arc<A2AProtocol>,
    event_bus: EventBus,
}

impl AgentLifecycleManager {
    /// Fork agent, register permissions, register A2A, submit to scheduler, run.
    pub async fn spawn_and_run(&self, seed: &Seed) -> Result<ExecutionResult> {
        // 1. supervisor.fork(seed)
        // 2. access_manager.get_or_create_permissions(agent_id, tool_list)
        // 3. a2a.register_agent(agent_card)
        // 4. scheduler.submit(task)
        // 5. supervisor.run_with_seed(agent_id, seed)
        // 6. a2a.unregister_agent(agent_id)
        // 7. scheduler.complete(task_id)
    }

    /// Kill agent and clean up all registered state.
    pub async fn terminate(&self, agent_id: AgentId) -> Result<()> {
        // scheduler.fail(task_id)
        // a2a.unregister_agent(agent_id)
        // supervisor.kill(agent_id)
    }
}
```

Orchestrator의 `handle_message`에서:

```rust
// AS-IS: 50줄의 fork/register/submit/run/unregister
// TO-BE:
let result = self.lifecycle.spawn_and_run(&seed).await?;
```

**Phase B — InterviewSessionStore 추출:**
`RwLock<HashMap<String, InterviewSession>>`을 별도 타입으로.

**Phase C — ToolAccessPolicy 추출:**
하드코딩된 `["bash","read","write","edit","grep","find"]`를 설정 기반으로.

**수정 파일:**
- `crates/oxios-kernel/src/agent_lifecycle.rs` — 신규
- `crates/oxios-kernel/src/orchestrator.rs` — handle_message 단순화
- `crates/oxios-kernel/src/lib.rs` — 모듈 등록

---

### 2-3. Graceful shutdown 시퀀스

**문제:** Ctrl+C 시 컨테이너/릴레이/에이전트 정리 없이 종료.

**해결:** 구조화된 셧다운 시퀀스.

```rust
// src/main.rs — 인터랙티브 모드 셧다운
async fn graceful_shutdown(kernel: &Kernel) {
    tracing::info!("Starting graceful shutdown...");

    // 1. MCP 서버 종료
    if let Err(e) = kernel.mcp_bridge.lock().await.shutdown_all().await {
        tracing::warn!(error = %e, "MCP shutdown error");
    }

    // 2. 실행 중인 에이전트 정리
    if let Ok(agents) = kernel.supervisor.list().await {
        for agent in &agents {
            if let Err(e) = kernel.supervisor.kill(agent.id).await {
                tracing::warn!(agent = %agent.id, error = %e, "Failed to kill agent");
            }
        }
    }

    // 3. 컨테이너 정지 (선택적 — 실행 중인 것만)
    // 컨테이너는 독립 프로세스이므로 강제 정지는 보류

    // 4. 세션 플러시 (StateStore는 파일 기반이라 자동)

    tracing::info!("Shutdown complete");
}
```

**수정 파일:** `src/main.rs` — 인터랙티브 모드 셧다운 블록

---

### 2-4. spawn_blocking + block_on 아키텍처 개선

**문제:** `AgentRuntime::execute()`가 `spawn_blocking` 내에서 `block_on`을 중첩 호출.
다중 에이전트 동시 실행 시 스레드 풀 고갈.

**해결:** `run_agent_loop`를 비동기로 재작성.

핵심 제약: `oxi-agent::AgentLoop::run()`이 `!Send`인지 확인 필요.

```
현재 구조:
execute() → spawn_blocking {           // blocking 스레드
    block_on {                          // tokio 런타임 재진입
        agent_loop.run(prompt, cb)      // !Send일 수 있음
        block_on { pm.list_enabled() }  // 중첩 block_on
        block_on { mcp_bridge.init() }  // 또 중첩
    }
}

목표 구조:
execute() → async {
    let programs = pm.list_enabled().await;     // 직접 await
    let mcp_tools = mcp_bridge.list_tools().await;  // 직접 await
    // AgentLoop::run()만 spawn_blocking (필요한 경우)
    let result = tokio::task::spawn_blocking(move || {
        agent_loop.run(prompt, cb)
    }).await?;
}
```

**제약 확인:** `oxi-agent`의 `AgentLoop`가 `Send`인지 조사 후 결정.
- `Send`면 → 완전 async로 전환
- `!Send`면 → `spawn_blocking`은 유지하되 I/O 준비만 밖으로 빼기

**수정 파일:** `crates/oxios-kernel/src/agent_runtime.rs` — `execute` + `run_agent_loop`

---

## Step 순서 및 예상 점수

```
Step 0 (P0 보안) → 45 → 58 (Path Traversal + Auth + MCP 버그)
Step 1 (P1 안정성) → 58 → 66 (WS 인증, 에러 표준화, body limit)
Step 2 (P2 아키텍처) → 66 → 72 (Orchestrator 분리, dead code, shutdown)
```

| Step | 커밋 | 파일 수 | 위험도 |
|------|-------|---------|--------|
| 0-1~0-2 | Path Traversal 수정 | 1 (routes.rs) | 낮음 |
| 0-3 | Auth middleware | 1 (middleware.rs) | 낮음 |
| 0-4 | MCP 버그 | 2 (mcp.rs, agent_runtime.rs) | 중간 |
| 1-1 | WS 인증 + 필터링 | 3 (routes, message, channel) | 중간 |
| 1-2 | Body limit | 1 (routes.rs) | 낮음 |
| 1-3 | expect 제거 | 2 (server.rs, main.rs) | 낮음 |
| 1-4 | AppError | 2+ (error.rs 신규, routes.rs) | 중간 |
| 2-1 | Dead code | 5+ (각 모듈) | 낮음 |
| 2-2 | Orchestrator 분리 | 3 (agent_lifecycle 신규, orchestrator, lib) | 높음 |
| 2-3 | Graceful shutdown | 1 (main.rs) | 낮음 |
| 2-4 | spawn_blocking 개선 | 1 (agent_runtime.rs) | 높음 |

**권장 실행 순서:** Step 0 → Step 1 → Step 2 (각 Step 내부는 병렬 가능)
