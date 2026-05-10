# 공학적으로 아름다운 소프트웨어로의 길

> Oxios 0.2.0-alpha 전체 리팩토링 설계서
> 
> 목표: 모든 계층이 명확한 책임을 가지고, 의존성이 올바른 방향으로 흐르며,
>       Feature flag가 거짓말을 하지 않고, 중복이 없는 아키텍처.

---

## 0. 설계 원칙

이 설계는 다음 원칙을 따른다:

1. **의존성은 항상 아래로** — 상위 계층이 하위 계층을 참조. 역방향 금지.
2. **Feature flag는 진실** — `--no-default-features --features cli`가 실제로 동작해야 함.
3. **리소스는 소유자에게** — 공유 리소스는 어느 채널에도 종속되지 않는다.
4. **단일 진입점** — Kernel은 한 번만 조립된다.
5. **Gateway는 메시징** — 대시보드 쿼리는 Gateway를 거치지 않는다 (이건 버그가 아니다).
6. **중복 제거** — 같은 로직이 두 곳에 존재하면 한 곳은 틀렸다.

---

## 1. 계층 모델 — 무엇이 어디에 속하는가

Oxios는 4개의 명확한 계층으로 구성된다:

```
┌─────────────────────────────────────────────────────────────┐
│ Layer 4: Binary (oxios)                                     │
│   CLI 파싱, 설정 로딩, Kernel 조립, 채널 활성화             │
├─────────────────────────────────────────────────────────────┤
│ Layer 3: Channels (web, cli, telegram)                      │
│   Web = Management Plane (kernel 직접 참조)                  │
│   CLI, Telegram = Messaging Channels (Gateway만 참조)        │
├─────────────────────────────────────────────────────────────┤
│ Layer 2: Gateway (oxios-gateway)                            │
│   메시지 라우팅, Channel trait, ChannelPlugin                │
│   kernel을 모름 (Orchestrator을 callback으로만 받음)         │
├─────────────────────────────────────────────────────────────┤
│ Layer 1: Kernel (oxios-kernel + oxios-ouroboros)            │
│   모든 비즈니스 로직, KernelHandle (7-Facade),              │
│   Supervisor, EventBus, StateStore, Tools                    │
└─────────────────────────────────────────────────────────────┘
```

### 계층 간 의존성 규칙

```
Binary ──→ Kernel, Gateway, Channels (모두)
Web    ──→ Kernel (직접), Gateway (채팅용)
CLI    ──→ Gateway만
Telegram ─→ Gateway만
Gateway ─→ Kernel 없음 (callback만)
Kernel ─→ 외부 크레이트만 (oxi-ai, oxi-agent)
```

### Web의 이원성 — 명시적 모델

Web 채널은 두 가지 면을 가진다. 이것은 결함이 아니라 **올바른 설계**다:

| 면 | 역할 | 경로 | 예시 |
|----|------|------|------|
| **Chat** | 메시징 | WebChannel → Gateway → Orchestrator | `/api/chat` |
| **Management** | 대시보드 | Route handler → KernelHandle | `/api/agents`, `/api/config`, `/api/memory` |

이 이원성을 문서화하고 코드에서도 명시적으로 드러낸다:

```rust
// oxios-web의 Cargo.toml 주석:
# Web channel has dual responsibility:
# 1. Messaging via Gateway (chat, websocket)
# 2. Management dashboard via KernelHandle (agents, config, programs, etc.)
# This is intentional — dashboard queries cannot go through the messaging Channel trait.
```

---

## 2. 리소스 재배치

### 문제

현재 `default-config.toml`, `default-skills/`, `default-programs/`이
`channels/oxios-web/static/`에 존재한다. 이 리소스들은 커널 레벨이며
web 채널 없이도 필요하다.

### 해결: 공유 리소스를 루트 `share/`로 이동

```
이동 전:                              이동 후:
channels/oxios-web/static/            share/
├── default-config.toml               ├── default-config.toml
├── default-skills/          →        ├── default-skills/
│   ├── code-review/                   │   ├── code-review/
│   ├── debug/                         │   ├── debug/
│   └── refactor/                      │   └── refactor/
├── default-programs/                  ├── default-programs/
│   ├── code-review/                   │   ├── code-review/
│   ├── debug/                         │   ├── debug/
│   └── refactor/                      │   └── refactor/
├── dioxus/                           └── (web 전용은 그대로)
├── index.html
└── Containerfile                     channels/oxios-web/static/
                                      ├── dioxus/
                                      ├── index.html
                                      └── Containerfile
```

main.rs에서:

```rust
// 변경 전 (web에 종속):
const DEFAULT_CONFIG: &str = include_str!("../channels/oxios-web/static/default-config.toml");

// 변경 후 (독립):
const DEFAULT_CONFIG: &str = include_str!("../share/default-config.toml");

// 기본 스킬:
let defaults_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("share/default-skills");

// 기본 프로그램:
let programs_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("share/default-programs");
```

**효과**: `--no-default-features --features cli` 빌드가 실제로 동작함.

---

## 3. Binary 단순화 — 단일 Kernel 조립

### 문제

`Kernel::builder()...build()`가 11개 서브커맨드에서 반복 호출된다.
매번 전체 서브시스템(LLM provider, MCP, EventBus, Supervisor...)을 새로 초기화한다.

### 해결: Kernel을 main 시작 시 한 번 조립하고 모든 서브커맨드에 전달

```rust
// main.rs 핵심 구조
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // 1. 초기화 (tracing, otel)
    let (config_path, config) = init_config(&cli)?;
    init_tracing(&cli, &config)?;
    init_otel(&config).await?;
    
    // 2. Kernel 조립 — 단 한 번
    let kernel = Kernel::builder()
        .config_path(config_path.clone())
        .model_id(&default_model)
        .build()
        .await?;
    
    // 3. 서브커맨드 실행 (kernel 참조 전달)
    match cli.command {
        Some(Command::Run { prompt }) => cmd_run(&kernel, &prompt).await,
        Some(Command::Chat) => cmd_chat(&kernel).await,
        Some(Command::Status) => cmd_status(&kernel).await,
        Some(Command::Pkg { action }) => cmd_pkg(&kernel, action).await,
        // ... 모든 서브커맨드가 &Kernel을 받음
        None => cmd_serve(&kernel, &config_path).await,
    }
}
```

### Kernel God Object 해소

Kernel의 20개 `pub` 필드를 캡슐화한다. 서브커맨드는 `kernel.handle()` (KernelHandle)을 통해서만 접근한다.

```rust
// kernel.rs
pub struct Kernel {
    // 모든 필드를 private으로 변경
    orchestrator: Arc<Orchestrator>,
    gateway: Gateway,
    event_bus: EventBus,
    config: OxiosConfig,
    // ... 나머지 필드
    
    // KernelHandle을 한 번 생성하여 캐시
    handle: OnceCell<Arc<KernelHandle>>,
}

impl Kernel {
    /// KernelHandle에 대한 단일 접근점
    pub fn handle(&self) -> Arc<KernelHandle> {
        self.handle.get_or_init(|| {
            Arc::new(KernelHandle::from_subsystems(/* ... */))
        }).clone()
    }
    
    /// Gateway에 대한 직접 접근 (채널 활성화에만 필요)
    pub fn gateway(&self) -> &Gateway {
        &self.gateway
    }
    
    /// 설정 읽기
    pub fn config(&self) -> &OxiosConfig {
        &self.config
    }
}
```

예외: `cmd_run`과 `cmd_serve`는 Orchestrator/Gateway에 직접 접근해야 한다.
이들을 위해 제한된 메서드를 노출한다:

```rust
impl Kernel {
    /// 메시지를 Gateway 경유로 처리 (cmd_run용)
    pub async fn route_message(&self, channel: &str, user: &str, content: &str) -> Result<()> {
        let msg = IncomingMessage::new(channel, user, content);
        self.gateway.route(msg).await
    }
    
    /// Orchestrator에 직접 메시지 (cmd_run에서 세션 생성 없이 빠른 실행)
    pub async fn execute_prompt(&self, prompt: &str) -> Result<OrchestrationResult> {
        self.orchestrator.handle_message("cli", prompt, None).await
    }
    
    /// 채널 등록
    pub async fn register_channel(&self, channel: Box<dyn Channel>) {
        self.gateway.register(channel).await;
    }
}
```

### 서브커맨드 시그니처 변경

```rust
// 변경 전: 각각 Kernel 새로 생성
async fn cmd_run(prompt: &str, config_path: &Path, model_id: &str) -> Result<()> {
    let kernel = Kernel::builder()...build().await?;
    // ...
}

// 변경 후: 공유 Kernel 참조
async fn cmd_run(kernel: &Kernel, prompt: &str) -> Result<()> {
    let result = kernel.execute_prompt(prompt).await?;
    // ...
}
```

### cmd_run의 Gateway 우회 문제

현재 `cmd_run`이 `kernel.orchestrator.handle_message("cli", ...)`를 직접 호출한다.
이 문제를 두 가지 선택지로 해결한다:

**선택지 A (권장): Gateway 경유**

```rust
async fn cmd_run(kernel: &Kernel, prompt: &str) -> Result<()> {
    // CLI 채널을 임시로 등록하여 Gateway 경유
    let cli_channel = CliChannel::new(256);
    let handle = cli_channel.handle();
    kernel.register_channel(Box::new(cli_channel)).await;
    
    // 메시지를 Gateway로 라우팅
    handle.send_user_message(prompt.to_string()).await?;
    
    // Gateway가 처리하고 CliChannel.send()로 출력
    // (하지만 이건 동기적 대기를 필요로 함...)
}
```

**선택지 B (실용적): 직접 실행, audit 보장**

```rust
async fn cmd_run(kernel: &Kernel, prompt: &str) -> Result<()> {
    // audit trail에 기록 (Gateway 우회 보상)
    kernel.handle().security.audit("cli", AuditAction::Other {
        detail: format!("run: {}", prompt.chars().take(100).collect::<String>()),
    }, "cli-user");
    
    let result = kernel.execute_prompt(prompt).await?;
    // ...
}
```

**결정: 선택지 B** — `cmd_run`은 일회성 빠른 실행이며, Gateway 메시지 루프가 돌지 않는다.
대신 audit 기록으로 추적성을 보장한다. Gateway 메시지 루프가 필요한 경우 `cmd_serve` 또는 `cmd_chat`을 사용한다.

---

## 4. Gateway 이벤트 드리븐 전환

### 문제

현재 `Gateway::run()`이 100ms마다 모든 채널을 폴링한다:
- CPU 낭비
- 순차 처리 (한 채널이 느리면 전체 지연)
- lock contention
- 메시지 손실 가능 (한 iteration에 하나만 처리)

### 해결: Stream 기반 Channel trait + select!

#### 4.1 Channel trait 재설계

```rust
// gateway/src/channel.rs

/// 채널 수명 주기.
#[derive(Debug, Clone, PartialEq)]
pub enum ChannelState {
    /// 정상 동작 중.
    Active,
    /// 일시적 장애. 재시도 로직 동작 중.
    Degraded { reason: String },
    /// 복구 불가. Gateway에서 제거 대상.
    Failed { reason: String },
}

/// 통신 채널 트레이트.
///
/// 채널은 `messages()`로 incoming 메시지 스트림을 제공하고,
/// `send()`로 outgoing 메시지를 수신한다.
#[async_trait]
pub trait Channel: Send + Sync {
    /// 채널 식별자 (예: "web", "cli", "telegram").
    fn name(&self) -> &str;
    
    /// Incoming 메시지 스트림.
    ///
    /// Gateway는 이 스트림을 `select!`로 병합하여 동시에 수신한다.
    /// 채널이 닫히면 스트림이 종료(`None`)된다.
    fn messages(&self) -> Pin<Box<dyn Stream<Item = IncomingMessage> + Send + '_>>;
    
    /// Outgoing 메시지 전송.
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;
    
    /// 현재 채널 상태 (선택적, 기본: Active).
    fn state(&self) -> ChannelState {
        ChannelState::Active
    }
}
```

기존 `receive()` 메서드를 `messages()` 스트림으로 교체.
스트림은 `tokio::sync::mpsc::Receiver`를 `ReceiverStream`으로 감싸면 간단하다.

```rust
// WebChannel 구현 예시
impl Channel for WebChannel {
    fn messages(&self) -> Pin<Box<dyn Stream<Item = IncomingMessage> + Send + '_>> {
        let rx = self.incoming_rx.clone(); // broadcast/mpsc receiver
        Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx))
    }
    // ...
}
```

#### 4.2 Gateway::run() 이벤트 드리븐

```rust
// gateway/src/gateway.rs

impl Gateway {
    /// 이벤트 드리븐 게이트웨이 루프.
    ///
    /// 모든 등록된 채널의 스트림을 `select_all!`로 병합하여
    /// 동시에 수신하고, 메시지가 도착하면 즉시 라우팅한다.
    pub async fn run(&self) -> Result<()> {
        tracing::info!("Gateway event loop started (event-driven)");
        
        // 채널이 등록될 때마다 스트림을 갱신하는 메커니즘
        let mut stream = self.build_merged_stream().await;
        
        loop {
            tokio::select! {
                // 메시지 수신
                Some(msg) = stream.next() => {
                    if let Err(e) = self.route(msg).await {
                        tracing::error!(error = %e, "Failed to route message");
                    }
                }
                // 채널 변경 알림
                _ = self.channel_changed.notified() => {
                    stream = self.build_merged_stream().await;
                }
                // 종료 신호
                _ = self.shutdown.recv() => {
                    tracing::info!("Gateway shutting down");
                    return Ok(());
                }
            }
        }
    }
    
    /// 등록된 모든 채널의 스트림을 병합.
    async fn build_merged_stream(&self) -> SelectAll<Pin<Box<dyn Stream<Item = IncomingMessage> + Send>>> {
        let channels = self.channels.read().await;
        let streams: Vec<_> = channels.values()
            .map(|ch| ch.messages())
            .collect();
        futures::stream::select_all(streams)
    }
}
```

**효과**:
- 메시지가 없을 때 CPU 0% (폴링 제거)
- 모든 채널 동시 수신 (순차 제거)
- lock 없이 메시지 수신 (lock은 등록/해제 시만)

#### 4.3 Telegram 무한 루프 해결

현재 `TelegramChannel::receive()` 안에 무한 루프가 있고
cancellation이 불가능하다. `messages()` 스트림으로 전환하면서
background polling task를 분리한다:

```rust
// telegram/lib.rs
pub struct TelegramChannel {
    incoming_tx: mpsc::Sender<IncomingMessage>,
    // ...
}

impl TelegramChannel {
    pub fn new(bot_token: String, allowed_users: Vec<i64>) -> Self {
        let (tx, rx) = mpsc::channel(256);
        Self {
            incoming_tx: tx,
            incoming_rx: Mutex::new(rx),
            // ...
        }
    }
    
    /// Background polling task. Spawned by TelegramPlugin.
    pub async fn run_polling(&self, cancel: CancellationToken) {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Telegram polling stopped");
                    return;
                }
                result = self.poll_updates() => {
                    match result {
                        Ok(updates) => {
                            for msg in self.process_updates(updates) {
                                if self.incoming_tx.send(msg).await.is_err() {
                                    return; // Channel closed
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Telegram poll error");
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                    }
                }
            }
        }
    }
}

impl Channel for TelegramChannel {
    fn messages(&self) -> Pin<Box<dyn Stream<Item = IncomingMessage> + Send + '_>> {
        let rx = self.incoming_rx.lock().await.take()
            .expect("messages() called twice");
        Box::pin(ReceiverStream::new(rx))
    }
    // ...
}
```

---

## 5. 중복 제거

### 5.1 OtelConfig 이중 정의 해결

```
src/otel.rs:OtelConfig ←→ crates/oxios-kernel/src/config.rs:OtelConfig
```

`otel.rs`에서 커널의 `OtelConfig`를 재사용:

```rust
// src/otel.rs
use oxios_kernel::config::OtelConfig;  // 재사용

pub async fn init_otel(config: &OtelConfig) -> Result<OtelGuard> {
    // 기존 로직 동일
}
```

### 5.2 expand_path 중복 제거

```
src/main.rs:expand_path() ←→ src/kernel.rs:expand_path()
```

`oxios_kernel`의 공개 유틸리티로 이동:

```rust
// crates/oxios-kernel/src/utils.rs (새 파일)
/// Expand `~/` in paths to the user's home directory.
pub fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(format!("{home}/{rest}"));
        }
    }
    PathBuf::from(path)
}
```

### 5.3 WebServer Dead Code 제거

`channels/oxios-web/src/server.rs`의 `WebServer` 구조체는
플러그인 아키텍처 도입 후 사용되지 않는다.

`AppState`만 남기고 `WebServer`와 `serve()` 메서드는 제거.
`AppState` 생성 로직은 `WebPlugin::setup()`으로 통합.

CORS 오리진도 하드코딩에서 config 기반으로 전환:

```rust
// plugin.rs
let cors_origins: Vec<_> = ctx.config.read()
    .security.cors_origins.iter()
    .map(|o| o.parse::<HeaderValue>().expect("invalid cors origin"))
    .collect();
let cors = tower_http::cors::CorsLayer::new()
    .allow_origin(cors_origins)
    .allow_methods(tower_http::cors::Any)
    .allow_headers(tower_http::cors::Any);
```

### 5.4 default-config.toml 중복 섹션 수정

```toml
# 변경 전 (중복):
[channels]
enabled = ["web"]

[channels]       # ← 두 번째!
enabled = ["web"]

# 변경 후:
[channels]
enabled = ["web"]
```

---

## 6. Kernel 내부 모듈 정리

### 6.1 대형 파일 모듈 내 분리

현재 4개 파일이 1000줄을 초과한다. 크레이트 분리 없이 파일 내 sub-module로 재구성:

#### `access_manager.rs` (1,785줄) → `access_manager/` 디렉토리

```
crates/oxios-kernel/src/access_manager/
├── mod.rs              (~200줄) AccessManager 오케스트레이터
├── rbac.rs             (~400줄) Role, Subject, Action, RbacPolicy, RbacManager
├── permissions.rs      (~200줄) AgentPermissions, PermissionUpdate
├── workspace.rs        (~300줄) 워크스페이스 샌드박스, 경로 검증
└── approval.rs         (~200줄) PendingApproval, ApprovalStatus
```

#### `mcp.rs` (1,227줄) → `mcp/` 디렉토리

```
crates/oxios-kernel/src/mcp/
├── mod.rs              (~200줄) McpBridge (서버 관리, 라우팅)
├── client.rs           (~400줄) McpClient (stdio JSON-RPC)
└── protocol.rs         (~400줄) JSON-RPC 타입, McpRequest/Response, McpTool
```

#### `program.rs` (1,151줄) → `program/` 디렉토리

```
crates/oxios-kernel/src/program/
├── mod.rs              (~300줄) ProgramManager (공개 API)
├── parser.rs           (~200줄) TOML 파싱, ProgramMeta, ToolDef
├── installer.rs        (~400줄) InstallSource별 설치 (Local, Git, Tarball)
└── types.rs            (~100줄) Program, ProgramMeta, ToolDef, HostRequirementsCheck
```

#### `memory.rs` (1,054줄) → `memory/` 디렉토리

```
crates/oxios-kernel/src/memory/
├── mod.rs              (~200줄) MemoryManager (공개 API, 큐레이션)
├── store.rs            (~300줄) 파일 저장/로드, 인덱스 관리
├── embedding.rs        (기존 embedding.rs와 통합 또는 그대로)
└── budget.rs           (~150줄) MemoryBudget, CurationReport
```

### 6.2 ExecTool 접근 제어 활성화

```rust
// tools/exec_tool.rs
impl ExecTool {
    pub async fn execute(&self, req: ExecRequest) -> Result<ExecResult> {
        // 추가: 접근 제어 확인
        if let Some(access) = self.access.as_ref() {
            let mgr = access.lock();
            if !mgr.can_access_path_in_workspace(
                &req.agent_id,  // 실제 agent_id 사용
                req.working_dir.as_deref().unwrap_or("."),
            ) {
                return Err(anyhow::anyhow!("Access denied: path outside workspace"));
            }
        }
        // 기존 실행 로직...
    }
}
```

또한 `can_access_path_in_workspace()`가 새 `AgentId::new_v4()`를
생성하는 버그를 수정 — 실제 agent_id를 매개변수로 받아야 함.

### 6.3 KernelHandle from_subsystems() 개선

18개 매개변수를 각 Facade가 독립적으로 생성하도록 변경:

```rust
// kernel.rs에서 KernelHandle 생성
impl Kernel {
    fn build_handle(&self) -> KernelHandle {
        KernelHandle::new(
            StateApi::new(self.state_store.clone()),
            AgentApi::new(
                self.supervisor.clone(),
                self.budget_manager.clone(),
                self.memory_manager.clone(),
            ),
            SecurityApi::new(
                self.auth_manager.clone(),
                self.audit_trail.clone(),
                self.access_manager.clone(),
            ),
            PersonaApi::new(self.persona_manager.clone()),
            ExtensionApi::new(
                self.program_manager.clone(),
                Arc::new(self.skill_store.clone()),
                Arc::new(self.host_tool_validator.clone()),
            ),
            McpApi::new(self.mcp_bridge.clone()),
            InfraApi::new(
                self.git_layer.clone(),
                self.scheduler.clone(),
                self.cron_scheduler.clone(),
                self.resource_monitor.clone(),
                self.event_bus.clone(),
                self.config.clone(),
                self.start_time,
            ),
        )
    }
}
```

`from_subsystems()` 평탄한 18개 매개변수 버전은 deprecated 처리.
`new()` (7개 Facade)만 공개.

---

## 7. config.rs 정리

### 7.1 채널 이름 하드코딩 제거

```rust
// 변경 전:
let valid = ["web", "cli", "telegram"];

// 변경 후:
// ChannelPlugin이 등록 시 self.name()을 반환하므로
// 커널은 채널 이름을 모른다.
// validate에서는 존재 여부만 경고:
for name in &self.channels.enabled {
    // 채널 이름은 런타임에 플러그인 등록으로 결정됨.
    // 여기서는 빈 문자열이나 특수문자만 거부.
    if name.is_empty() || name.contains('/') {
        errors.push(format!("channels.enabled: invalid channel name '{}'", name));
    }
}
```

### 7.2 GatewayConfig 위치

현재 `GatewayConfig` (host, port)가 `oxios-kernel/src/config.rs`에 있다.
이것은 web 채널의 서버 설정이지 커널 설정이 아니다.

하지만 GatewayConfig을 별도로 분리하면 설정 파일이 분산되고
사용자 경험이 나빠진다. **단일 config.toml을 유지**하되
개념적으로 GatewayConfig이 커널에 속하는 것을 문서화:

```rust
/// Gateway/Server configuration.
///
/// Lives in kernel config for single-file convenience.
/// The web channel reads this for server binding.
/// Other channels (CLI, Telegram) ignore it.
pub struct GatewayConfig {
    pub host: String,
    pub port: u16,
}
```

---

## 8. 구현 순서

의존성 순서대로. 이전 Phase가 완료되어야 다음 Phase가 의미 있다.

### Phase 1: 리소스 독립 (기반 작업)

**목표**: web 없이도 컴파일/실행 가능

| 작업 | 파일 | 설명 |
|------|------|------|
| 1-1 | `share/` 디렉토리 생성 | default-config, skills, programs 이동 |
| 1-2 | `main.rs` | `include_str!` 경로를 `share/`로 변경 |
| 1-3 | `main.rs` | 기본 스킬/프로그램 경로를 `share/`로 변경 |
| 1-4 | `default-config.toml` | 중복 `[channels]` 섹션 제거 |
| 1-5 | `server.rs` → `AppState`만 남기고 `WebServer` 제거 | dead code 정리 |
| 1-6 | `plugin.rs` | AppState 생성 로직 통합, CORS를 config에서 읽기 |
| 1-7 | `.gitignore` | `channels/oxios-web/static/default-*` 대신 `share/` 참조 |

**검증**: `cargo check --no-default-features --features cli` 성공

### Phase 2: 단일 진입점

**목표**: main.rs 단순화, Kernel God Object 해소

| 작업 | 파일 | 설명 |
|------|------|------|
| 2-1 | `kernel.rs` | Kernel 필드 `pub` → private, `OnceCell<KernelHandle>` 캐시 |
| 2-2 | `kernel.rs` | 제한된 공개 메서드 추가 (`handle()`, `gateway()`, `config()`, `execute_prompt()`) |
| 2-3 | `main.rs` | `Kernel::builder()`를 main 시작 시 한 번만 호출 |
| 2-4 | `main.rs` | 모든 서브커맨드 시그니처를 `(&Kernel, ...)`로 변경 |
| 2-5 | `main.rs` | `cmd_run`에 audit 기록 추가 |
| 2-6 | `otel.rs` | `OtelConfig`를 `oxios_kernel::config::OtelConfig`로 통일 |
| 2-7 | `oxios-kernel` | `utils.rs`에 `expand_home()` 추가, main/kernel에서 재사용 |
| 2-8 | `kernel_handle/mod.rs` | `from_subsystems()` deprecated, `new()` (7개 Facade) 사용 |

**검증**: `cargo test --workspace` 기존 테스트 통과

### Phase 3: Gateway 이벤트 드리븐

**목표**: 폴링 제거, 채널 생명주기 관리

| 작업 | 파일 | 설명 |
|------|------|------|
| 3-1 | `gateway/Cargo.toml` | `futures`, `tokio-stream` 의존성 추가 |
| 3-2 | `channel.rs` | `receive()` → `messages()` Stream으로 trait 변경 |
| 3-3 | `channel.rs` | `ChannelState` enum 추가, `state()` 기본 구현 |
| 3-4 | `gateway.rs` | `run()`을 `select_all!` 기반으로 재작성 |
| 3-5 | `gateway.rs` | `shutdown: tokio::sync::watch` 추가 (우아한 종료) |
| 3-6 | `web/channel.rs` | `messages()` 구현 (ReceiverStream) |
| 3-7 | `cli/channel.rs` | `messages()` 구현 (ReceiverStream) |
| 3-8 | `telegram/lib.rs` | `messages()` + background polling task 분리 |
| 3-9 | `telegram/plugin.rs` | CancellationToken을 ChannelBundle에 포함 |
| 3-10 | `plugin.rs` (gateway) | `ChannelBundle`에 `CancellationToken` 필드 추가 |

**검증**: 대시보드 접속 + 채팅 동작 확인

### Phase 4: 커널 모듈 정리

**목표**: 대형 파일 분리, 보안 강화

| 작업 | 파일 | 설명 |
|------|------|------|
| 4-1 | `access_manager.rs` → `access_manager/` | rbac, permissions, workspace, approval로 분리 |
| 4-2 | `mcp.rs` → `mcp/` | client, protocol로 분리 |
| 4-3 | `program.rs` → `program/` | parser, installer, types로 분리 |
| 4-4 | `memory.rs` → `memory/` | store, budget으로 분리 |
| 4-5 | `tools/exec_tool.rs` | access 필드 활성화, agent_id 매개변수 추가 |
| 4-6 | `access_manager/` | `can_access_path_in_workspace`에 실제 agent_id 전달 |

**검증**: `cargo test --workspace` + `cargo clippy --workspace`

---

## 9. 변경 전후 비교

### main.rs (핵심 로직)

```rust
// ═══════════════════════════════════════════════════════════
// 변경 전 (~350줄, Kernel 11번 생성, web 하드코딩)
// ═══════════════════════════════════════════════════════════

const DEFAULT_CONFIG: &str = include_str!("../channels/oxios-web/static/default-config.toml");
// ...
async fn cmd_run(prompt: &str, config_path: &Path, model_id: &str) -> Result<()> {
    let kernel = Kernel::builder().config_path(...).model_id(...).build().await?;
    let result = kernel.orchestrator.handle_message("cli", prompt, None).await?;
    // ...
}
// ... 10번 더 반복 ...

None => {
    let kernel = Kernel::builder()...build().await?;
    let web_channel = oxios_web::WebChannel::new(256);
    let channel_handle = WebChannelHandle::from_channel(&web_channel);
    let _web_server = WebServer::new(host, port, channel_handle, ...)?;
    let app = axum::Router::new().merge(routes)...  // 40줄의 웹 설정
    // ...
}


// ═══════════════════════════════════════════════════════════
// 변경 후 (~150줄, Kernel 1번 생성, 플러그인 기반)
// ═══════════════════════════════════════════════════════════

const DEFAULT_CONFIG: &str = include_str!("../share/default-config.toml");

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let (config_path, config) = init_config(&cli)?;
    init_tracing(&cli, &config)?;
    init_otel(&config).await?;

    let kernel = Kernel::builder()
        .config_path(config_path.clone())
        .build()
        .await?;

    match cli.command {
        Some(Command::Run { prompt }) => cmd_run(&kernel, &prompt).await,
        Some(Command::Chat) => cmd_chat(&kernel).await,
        // ... 모든 서브커맨드가 &Kernel을 받음 ...
        None => cmd_serve(&kernel, &config_path).await,
    }
}

async fn cmd_serve(kernel: &Kernel, config_path: &Path) -> Result<()> {
    init_defaults(kernel).await?;
    let channel_tasks = activate_channels(kernel.gateway(), kernel.config(), config_path).await?;
    kernel.start_guardian();
    kernel.run_gateway().await?;
    shutdown(kernel, channel_tasks).await
}
```

### Gateway

```
변경 전: 100ms polling, 순차 처리, lock 2회/iteration
변경 후: 이벤트 드리븐 (select_all!), 동시 처리, lock 없음
```

### Kernel

```
변경 전: 20개 pub 필드, handle() 매번 새 인스턴스
변경 후: private 필드, OnceCell 캐시, 제한된 공개 메서드
```

### 의존성 그래프

```
변경 전:
  oxios-web → kernel (직접)     ✅ 올바름 (Management Plane)
  oxios-cli → gateway만         ✅ 올바름
  oxios-telegram → gateway만    ✅ 올바름
  main.rs → oxios-web (절대 경로) ❌ Feature flag 거짓말

변경 후:
  oxios-web → kernel (직접)     ✅ 동일
  oxios-cli → gateway만         ✅ 동일
  oxios-telegram → gateway만    ✅ 동일
  main.rs → share/ (독립 리소스) ✅ Feature flag 진실
```

---

## 10. 리스크와 완화

| 리스크 | 영향 | 완화 |
|--------|------|------|
| Channel trait 변경이 기존 구현 깨짐 | 높음 | Phase 3에서 세 채널 동시 수정 |
| Kernel 필드 private화가 서브커맨드 깨짘 | 중간 | Phase 2에서 모든 서브커맨드 시그니처 동시 변경 |
| 모듈 분리 시 circular import | 낮음 | 분리 전 import 그래프 분석 |
| Gateway select_all! 버그 | 중간 | 기존 폴링 코드를 삭제하지 않고 deprecated 유지 |

---

## 11. 하지 않는 것

이 설계에서 **명시적으로 제외**하는 것:

1. **커널 크레이트 분리** — oxios-kernel은 monolithic 유지. 모듈 내 분리만.
2. **Web route handler의 kernel 직접 접근 제거** — Management Plane으로서 올바름.
3. **Ouroboros execute() 스텁 해결** — LLM 통합 이슈로 별도 작업 필요.
4. **프론트엔드 (Dioxus) 변경** — 백엔드 아키텍처에 영향 없음.
5. **새 기능 추가** — 오직 구조 개선만.

---

## 12. 완료 기준

- [ ] `cargo check --no-default-features --features cli` 성공
- [ ] `cargo check --no-default-features --features web` 성공
- [ ] `cargo check --features telegram` 성공
- [ ] `cargo test --workspace` 기존 테스트 모두 통과
- [ ] `cargo clippy --workspace` 새 warning 없음
- [ ] 대시보드 접속 + 채팅 동작 확인
- [ ] `oxios run "hello"` 동작 확인
- [ ] `oxios chat` 동작 확인
- [ ] Gateway CPU 사용률 idle 시 0% (폴링 제거 확인)
