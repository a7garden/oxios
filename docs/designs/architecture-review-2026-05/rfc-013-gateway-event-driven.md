# RFC-013: Gateway Event-Driven 마이그레이션

> **상태:** 📝 설계
> **날짜:** 2026-05-27
> **우선순위:** P0
> **범위:** `crates/oxios-gateway/`, `channels/`
> **선행:** 없음
> **후행:** RFC-014 (채널 UX 통일)

---

## 1. 동기

### 1.1 현재 아키텍처

현재 Gateway는 **동기식 폴링 루프**다:

```rust
// 현재 gateway.rs — 실제 코드
pub struct Gateway {
    channels: RwLock<HashMap<String, Box<dyn Channel>>>,
    orchestrator: Arc<oxios_kernel::Orchestrator>,
    shutdown: Arc<AtomicBool>,
}

pub async fn run(&self) -> Result<()> {
    loop {
        let channel_names = {
            let channels = self.channels.read().await;
            channels.keys().cloned().collect::<Vec<_>>()
        };

        for name in &channel_names {
            loop {
                let msg = {
                    let mut channels = self.channels.write().await;  // ← WRITE LOCK
                    if let Some(ch) = channels.get_mut(name) {
                        ch.receive().await.ok().flatten()            // ← 최대 30초 블로킹
                    } else { break }
                };
                if let Some(msg) = msg { /* route */ } else { break }
            }
        }

        if received_any {
            tokio::task::yield_now().await;
        } else if self.shutdown.load(Ordering::Acquire) {
            break Ok(());
        } else {
            tokio::time::sleep(Duration::from_millis(50)).await;     // ← idle CPU 낭비
        }
    }
}
```

현재 `Channel` trait:

```rust
// 현재 channel.rs — 실제 코드
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn receive(&self) -> Result<Option<IncomingMessage>>;
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;
}
```

현재 `ChannelBundle`:

```rust
// 현재 plugin.rs — 실제 코드
pub struct ChannelBundle {
    pub channel: Box<dyn Channel>,
    pub tasks: Vec<JoinHandle<()>>,
}
```

### 1.2 문제

현재 코드를 실제로 분석한 결과, 다섯 가지 구조적 결함이 확인된다:

| # | 문제 | 원인 | 실제 코드 위치 |
|---|------|------|---------------|
| 1 | **전체 채널 블로킹** | Telegram `receive()`가 `poll_updates()`를 30초 timeout으로 호출. 이 동안 Gateway 루프의 `for name` 반복이 멈춤 → Web/CLI 수신 차단 | `telegram/lib.rs:280` 무한 `loop` + `poll_updates().await` |
| 2 | **불필요한 write lock** | `receive()`가 `&self`인데 `channels.get_mut(name)`을 위해 전체 HashMap에 write lock | `gateway.rs:195` `self.channels.write().await` |
| 3 | **순차 채널 처리** | 한 채널의 버스트(예: Telegram 10개 업데이트)가 다른 채널 지연 | `gateway.rs:190` `for name in &channel_names` 직렬 루프 |
| 4 | **idle CPU 낭비** | 메시지가 없을 때 50ms마다 무의미한 루프 반복 | `gateway.rs:207` `sleep(50ms)` |
| 5 | **순차 메시지 처리** | `route()`가 `orchestrator.handle_message()`를 await. 한 요청 3~10초(LLM 호출) 동안 다른 요청 대기 | `gateway.rs:157` `self.route(msg).await` — 직렬 |

문제 1~4는 폴링 구조의 결함이고, **문제 5는 수신을 병렬화해도 해결되지 않는다** — 현재 제안이 수신만 병렬로 만들고 처리는 여전히 직렬이기 때문이다. Oxios은 에이전트 OS이며, 여러 사용자/에이전트가 동시에 실행되는 게 핵심이다. 메시지 처리 직렬화는 아키텍처 목적과 모순된다.

**추가 문제 — 전용 스레드 강제:** Gateway가 `std::thread::spawn`으로 분리된 단일 스레드 tokio runtime에서 실행된다 (`main.rs:1564`). `parking_lot` guard가 `Send`가 아니기 때문인데, event-driven 전환 후에는 `tokio::spawn`으로 메인 런타임에서 실행 가능하다.

---

## 2. 설계

### 2.1 핵심 원칙: Push + 동시 처리

두 가지를 동시에 해결한다:

1. **Pull → Push**: 각 채널이 자체 백그라운드 태스크에서 메시지를 수집해 공유 mpsc로 push
2. **직렬 → 동시**: 수신한 메시지마다 독립 태스크로 처리. Semaphore로 동시성 제한

```
현재 (Pull + 직렬 처리):
┌────────────────────────────────────────────────────────────┐
│ Gateway 루프                                                │
│   Web → CLI → Telegram → ...     ← 순차 수신, write lock   │
│         │                                                   │
│         ▼                                                   │
│   route(msg1) → route(msg2) → ... ← 순차 처리, 각 3~10초   │
│         ↕ 50ms sleep when idle                              │
└────────────────────────────────────────────────────────────┘

변경 (Push + 동시 처리):
┌────────────┐  ┌────────────┐  ┌─────────────┐
│ Web 태스크  │  │ CLI 태스크  │  │Telegram태스크│   ← 독립 수신
└─────┬──────┘  └─────┬──────┘  └──────┬──────┘
      └───────────────┼────────────────┘
                      ▼
             ┌────────────────┐
             │  Gateway rx    │  ← tokio::select!
             │  (mpsc 1024)   │
             └───────┬────────┘
                     │  각 메시지마다 tokio::spawn
              ┌──────┼──────┐
              ▼      ▼      ▼
           ┌─────┐┌─────┐┌─────┐
           │route││route││route│   ← Semaphore로 동시성 제한
           │(F1) ││(F2) ││(F3) │      (기본: 32 동시 요청)
           └──┬──┘└──┬──┘└──┬──┘
              └──────┼──────┘
                     ▼
              채널별 send()
```

### 2.2 데이터 구조

```rust
// 새 gateway.rs
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{mpsc, watch, Mutex, RwLock, Semaphore};
use tokio::task::JoinHandle;

/// Gateway 수신 버퍼.
/// 모든 채널의 메시지가 모이는 공통 큐.
/// 1024 = 대략 100 동시 세션 × 10메시지/세션 여유.
const GATEWAY_BUFFER: usize = 1024;

/// 최대 동시 오케스트레이션 수.
/// LLM 호출이 I/O-bound이므로 CPU 코어 수보다 높게 설정.
/// 32 = 4코어 × 8배 여유 (I/O 대기 중 다른 태스크 실행).
const MAX_CONCURRENT_ROUTES: usize = 32;

/// 통합 수신구의 메시지 타입.
///
/// `Channel` trait의 `start()` 시그니처에 사용되므로
/// `oxios_gateway` 크레이트의 공개 타입이어야 한다.
/// `lib.rs`에서 정의하거나 `channel.rs`에서 정의 후 re-export.
pub type GatewayInbox = (String, IncomingMessage);

/// Gateway: 채널-커널 메시지 라우터.
pub struct Gateway {
    /// 채널별 레지스트리 (등록/해제/송신용).
    channels: Arc<RwLock<HashMap<String, ChannelEntry>>>,

    /// 모든 채널의 수신 메시지가 모이는 통합 수신구.
    rx: Mutex<mpsc::Receiver<GatewayInbox>>,

    /// 송신구 복제본 — 새 채널 start()에 전달.
    tx: mpsc::Sender<GatewayInbox>,

    /// Orchestrator 참조.
    orchestrator: Arc<oxios_kernel::Orchestrator>,

    /// 전체 Gateway 종료 신호.
    shutdown: watch::Sender<bool>,

    /// 동시 처리 제한.
    concurrency: Arc<Semaphore>,
}

/// 등록된 채널의 핸들.
struct ChannelEntry {
    /// 채널 trait 객체 — send() 호출용.
    channel: Arc<dyn Channel>,
    /// 채널 고유 종료 신호.
    shutdown_tx: watch::Sender<bool>,
    /// start()가 반환한 태스크 핸들 — unregister 시 종료 대기.
    task: JoinHandle<()>,
}
```

**설계 결정: `OrchestratorApi` trait 도입하지 않음**

현재 코드 베이스에 해당 trait이 없고 `Orchestrator` 단일 구현체만 존재한다. 불필요한 추상화를 피하고 `Arc<Orchestrator>`를 그대로 사용한다. 향후 mock 주입이 필요해지면 그때 trait을 도입한다 (YAGNI).

**설계 결정: `channels`를 `Arc<RwLock<>>`로 감싼다**

동시 처리를 위해 `route()`가 별도 태스크에서 실행된다. `route()`는 응답 전송을 위해 채널 레지스트리에 접근해야 하므로, `Arc`로 감싸 여러 태스크가 공유한다. `RwLock`은 읽기만 필요하므로 contention이 없다.

### 2.3 Channel Trait 변경

```rust
// 변경 전 (현재 — 실제 코드)
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn receive(&self) -> Result<Option<IncomingMessage>>;
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;
}

// 변경 후
#[async_trait]
pub trait Channel: Send + Sync {
    /// 채널 이름 (예: "web", "cli", "telegram").
    fn name(&self) -> &str;

    /// 채널을 백그라운드에서 시작.
    ///
    /// 수신 메시지가 들어오면 `tx`로 push.
    /// `shutdown`이 변경되면 루프를 종료.
    ///
    /// 구현체는 `tokio::spawn`으로 내부 루프를 돌리고
    /// 그 `JoinHandle<()>`을 반환한다.
    /// Gateway는 이 핸들로 태스크 수명을 추적한다.
    async fn start(
        &self,
        tx: mpsc::Sender<GatewayInbox>,
        shutdown: watch::Receiver<bool>,
    ) -> Result<JoinHandle<()>>;

    /// 응답 메시지 전송. Gateway의 route() 완료 후 호출.
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;
}
```

| 측면 | `receive()` | `start()` |
|------|-------------|-----------|
| 호출 주체 | Gateway 루프가 매번 호출 | Gateway가 최초 1회만 호출 |
| 블로킹 | 구현체가 블로킹 가능 (Telegram 30초) | 내부 태스크가 블로킹, Gateway는 영향 없음 |
| 수명 관리 | 없음 | `shutdown` watch로 우아한 종료 |
| 메시지 전달 | 리턴 값 | `tx.send()` push |
| 태스크 추적 | 불가 | `JoinHandle<()>` 반환으로 추적 가능 |

### 2.4 ChannelPlugin — 변경 없음

현재 `ChannelBundle`을 그대로 유지한다:

```rust
// 변경 없음
pub struct ChannelBundle {
    pub channel: Box<dyn Channel>,
    pub tasks: Vec<JoinHandle<()>>,
}
```

이유:
1. Web 채널의 axum 서버 태스크는 채널 수신과 무관하게 독립 동작하므로 `ChannelBundle.tasks`로 반환하는 것이 자연스럽다
2. `main.rs::activate_channels`에서 `all_tasks.extend(bundle.tasks)` 기존 패턴 유지
3. `Channel::start()`는 채널 수신 루프만 담당하고, 인프라 태스크(서버 등)는 여전히 `setup()`에서 반환

### 2.5 Gateway 코어

```rust
impl Gateway {
    pub fn new(orchestrator: Arc<oxios_kernel::Orchestrator>) -> Self {
        let (tx, rx) = mpsc::channel(GATEWAY_BUFFER);
        let (shutdown, _) = watch::channel(false);
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            rx: Mutex::new(rx),
            tx,
            orchestrator,
            shutdown,
            concurrency: Arc::new(Semaphore::new(MAX_CONCURRENT_ROUTES)),
        }
    }

    // ── 채널 관리 ──────────────────────────────────────

    /// 채널 등록 + 백그라운드 시작.
    pub async fn register(&self, channel: Box<dyn Channel>) -> Result<()> {
        let name = channel.name().to_owned();

        let (ch_shutdown, ch_shutdown_rx) = watch::channel(false);

        // Arc로 보관 — route() 태스크에서 send() 호출용
        let ch_arc: Arc<dyn Channel> = Arc::from(channel);

        // start() → 내부에서 tokio::spawn → JoinHandle 반환
        let task = ch_arc.start(self.tx.clone(), ch_shutdown_rx).await?;

        self.channels.write().await.insert(name.clone(), ChannelEntry {
            channel: ch_arc,
            shutdown_tx: ch_shutdown,
            task,
        });

        tracing::info!(channel = %name, "Channel registered and started");
        Ok(())
    }

    /// 채널 등록 해제 (우아한 종료).
    pub async fn unregister(&self, name: &str) -> Result<()> {
        let entry = self.channels.write().await.remove(name);
        if let Some(entry) = entry {
            let _ = entry.shutdown_tx.send(true);
            let _ = tokio::time::timeout(
                Duration::from_secs(5),
                entry.task,
            ).await;
            tracing::info!(channel = %name, "Channel unregistered");
        }
        Ok(())
    }

    /// 등록된 채널 이름 목록.
    pub async fn channel_names(&self) -> Vec<String> {
        self.channels.read().await.keys().cloned().collect()
    }

    // ── 이벤트 루프 ────────────────────────────────────

    /// Gateway 메인 이벤트 루프.
    ///
    /// 수신: tokio::select!로 어떤 채널에서든 즉시 수신 (논블로킹).
    /// 처리: 각 메시지를 독립 태스크로 spawn → 동시 처리.
    pub async fn run(&self) -> Result<()> {
        tracing::info!("Gateway event loop started");
        let mut rx = self.rx.lock().await;
        let mut shutdown = self.shutdown.subscribe();

        loop {
            tokio::select! {
                inbox = rx.recv() => {
                    match inbox {
                        Some((channel_name, msg)) => {
                            self.dispatch(channel_name, msg);
                        }
                        None => {
                            tracing::info!("All channels disconnected, exiting");
                            break;
                        }
                    }
                }

                _ = shutdown.changed() => {
                    tracing::info!("Gateway shutting down");
                    let channels = self.channels.read().await;
                    for (name, entry) in channels.iter() {
                        let _ = entry.shutdown_tx.send(true);
                        tracing::info!(channel = %name, "Shutdown signal sent");
                    }
                    break;
                }
            }
        }

        Ok(())
    }

    /// 메시지를 독립 태스크로 dispatch.
    ///
    /// 이벤트 루프를 블로킹하지 않고 즉시 다음 메시지 수신.
    /// Semaphore로 동시 처리 수를 MAX_CONCURRENT_ROUTES로 제한.
    fn dispatch(&self, channel_name: String, msg: IncomingMessage) {
        let orchestrator = self.orchestrator.clone();
        let channels = self.channels.clone();
        let semaphore = self.concurrency.clone();

        tokio::spawn(async move {
            // 동시성 제한 — 초과 요청은 대기
            let _permit = match semaphore.acquire().await {
                Ok(p) => p,
                Err(_) => {
                    tracing::warn!("Semaphore closed, dropping message");
                    return;
                }
            };

            // ── route 본문 (기존 route()와 동일) ──

            tracing::info!(
                channel = %msg.channel,
                user = %msg.user_id,
                content_len = msg.content.len(),
                "Routing incoming message"
            );

            let session_id = msg.metadata.get("session_id").cloned();
            let result = orchestrator
                .handle_message(&msg.user_id, &msg.content, session_id.as_deref())
                .await;

            let guard = channels.read().await;
            let entry = guard.get(&channel_name);

            match (result, entry) {
                (Ok(orchestration), Some(entry)) => {
                    let mut response_metadata = HashMap::new();
                    if let Some(ref sid) = orchestration.session_id {
                        response_metadata.insert("session_id".to_owned(), sid.clone());
                    }
                    if let Some(ref vid) = orchestration.space_id {
                        response_metadata.insert("space_id".to_owned(), vid.to_string());
                    }
                    response_metadata
                        .insert("phase".to_owned(), orchestration.phase_reached.to_string());
                    response_metadata.insert(
                        "evaluation_passed".to_owned(),
                        orchestration.evaluation_passed.to_string(),
                    );

                    let outgoing = OutgoingMessage::with_id_and_metadata(
                        msg.id,
                        &msg.channel,
                        &msg.user_id,
                        &orchestration.response,
                        response_metadata,
                    );
                    if let Err(e) = entry.channel.send(outgoing).await {
                        tracing::error!(error = %e, "Failed to send response");
                    }
                }
                (Err(e), Some(entry)) => {
                    tracing::error!(error = %e, "Orchestration failed");
                    let outgoing = OutgoingMessage::with_id(
                        msg.id,
                        &msg.channel,
                        &msg.user_id,
                        format!("An error occurred: {e}"),
                    );
                    if let Err(e) = entry.channel.send(outgoing).await {
                        tracing::error!(error = %e, "Failed to send error response");
                    }
                }
                (_, None) => {
                    tracing::warn!(channel = %channel_name, "Channel no longer registered");
                }
            }
        });
    }

    // ── 공개 유틸리티 ──────────────────────────────────

    /// 명명된 채널로 메시지 전송 (외부 호출용).
    pub async fn send_to(&self, channel_name: &str, msg: OutgoingMessage) -> Result<()> {
        let channels = self.channels.read().await;
        if let Some(entry) = channels.get(channel_name) {
            entry.channel.send(msg).await?;
        } else {
            tracing::warn!(channel = %channel_name, "No such channel registered");
        }
        Ok(())
    }

    /// Gateway 전체 종료 신호.
    pub fn signal_shutdown(&self) {
        let _ = self.shutdown.send(true);
        tracing::info!("Gateway shutdown signal sent");
    }

    pub fn is_shutdown(&self) -> bool {
        *self.shutdown.borrow()
    }
}
```

**`dispatch()` vs `route()`**

기존 `route()`는 `self.route(msg).await`로 이벤트 루프를 블로킹했다. 새 `dispatch()`는 필요한 상태를 Arc로 복제해 `tokio::spawn`으로 넘긴 뒤 즉시 리턴한다. 이벤트 루프는 다음 메시지를 즉시 수신할 수 있다.

Semaphore는 동시 처리 수를 제한한다. LLM 호출이 I/O-bound이므로 32는 보수적 설정이다. 프로덕션에서 `OxiosConfig`로 조정 가능하다.

### 2.6 Gateway 실행 — 전용 스레드에서 메인 런타임으로

현재 `main.rs:1564`에서 별도 스레드로 실행한다:

```rust
// 현재 main.rs — 실제 코드
let gateway_handle = std::thread::spawn(move || {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("gateway thread runtime");
    rt.block_on(gateway.run()).expect("gateway run error");
});
```

새 설계에서는 `tokio::spawn`으로 메인 런타임에서 실행한다:

```rust
// 변경 후 main.rs
let gateway = kernel.gateway();
let gateway_task = tokio::spawn(async move {
    gateway.run().await.expect("gateway run error");
});
```

별도 스레드 생성 오버헤드가 사라지고, 메인 런타임의 워커 풀을 공유한다. `parking_lot` guard가 `Send`가 아니어서 생긴 제약이 더 이상 해당하지 않는다.

---

## 3. 채널별 마이그레이션

WebChannel과 CliChannel은 이미 내부적으로 `mpsc`를 사용하므로 `start()` 구현이 동일한 패턴을 따른다. TelegramChannel만 실제 로직 이동이 필요하다.

### 3.1 공통 패턴: mpsc 브릿지

WebChannel과 CliChannel은 둘 다 이 구조를 공유한다:

```
외부 소스 (HTTP/readline) → incoming_tx (mpsc) → incoming_rx → start() 브릿지 → Gateway tx
```

`start()`는 `incoming_rx`에서 읽어 Gateway의 `tx`로 전달하는 브릿지 태스크를 띄운다.

### 3.2 WebChannel

현재 구조 — 이미 `mpsc::Sender/Receiver` 기반:

```rust
// 현재 — 실제 코드
pub struct WebChannel {
    incoming_rx: Mutex<mpsc::Receiver<IncomingMessage>>,
    incoming_tx: mpsc::Sender<IncomingMessage>,
    outgoing_tx: broadcast::Sender<OutgoingMessage>,
    responses: Arc<RwLock<HashMap<Uuid, oneshot::Sender<OutgoingMessage>>>>,
}
```

```rust
// 변경 후 — start()만 추가, send() 변경 없음
#[async_trait]
impl Channel for WebChannel {
    fn name(&self) -> &str { "web" }

    async fn start(
        &self,
        tx: mpsc::Sender<GatewayInbox>,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<JoinHandle<()>> {
        let mut internal_rx = self.incoming_rx.lock().await;
        let channel_name = self.name().to_owned();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = internal_rx.recv() => {
                        match msg {
                            Some(msg) => {
                                if tx.send((channel_name.clone(), msg)).await.is_err() {
                                    break; // Gateway 수신구 닫힘
                                }
                            }
                            None => break,
                        }
                    }
                    _ = shutdown.changed() => break,
                }
            }
            tracing::info!(channel = %channel_name, "Web channel stopped");
        });

        Ok(handle)
    }

    // send() — 기존 로직 그대로 (oneshot correlation + broadcast)
    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        {
            let mut responses = self.responses.write().await;
            if let Some(sender) = responses.remove(&msg.id) {
                let _ = sender.send(msg.clone());
            }
        }
        let _ = self.outgoing_tx.send(msg);
        Ok(())
    }
}
```

**WebPlugin 조정:** `ChannelBundle.tasks`에 axum 서버 핸들을 그대로 반환:

```rust
// WebPlugin — 변경 없음
Ok(ChannelBundle {
    channel: Box::new(web_channel),
    tasks: vec![server_handle],
})
```

**변경 없는 것:** `WebChannelHandle`, `send_and_wait()`, `subscribe()`, HTTP route handlers.

### 3.3 CliChannel

현재 구조 — WebChannel과 동일한 mpsc 브릿지:

```rust
// 현재 — 실제 코드
pub struct CliChannel {
    incoming_rx: Mutex<mpsc::Receiver<IncomingMessage>>,
    incoming_tx: mpsc::Sender<IncomingMessage>,
    session: Arc<std::sync::Mutex<Session>>,
}
```

```rust
// 변경 후 — start()만 추가
#[async_trait]
impl Channel for CliChannel {
    fn name(&self) -> &str { "cli" }

    async fn start(
        &self,
        tx: mpsc::Sender<GatewayInbox>,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<JoinHandle<()>> {
        let mut internal_rx = self.incoming_rx.lock().await;
        let channel_name = self.name().to_owned();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = internal_rx.recv() => {
                        match msg {
                            Some(msg) => {
                                if tx.send((channel_name.clone(), msg)).await.is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    _ = shutdown.changed() => break,
                }
            }
            tracing::info!(channel = %channel_name, "CLI channel stopped");
        });

        Ok(handle)
    }

    // send() — 기존과 동일
    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        println!("{}", msg.content);
        Ok(())
    }
}
```

**변경 없는 것:** `CliChannelHandle`, `InteractiveLoop`, `Session`.

### 3.4 TelegramChannel

현재 `receive()`는 **무한 폴링 루프 + 명령 처리 + 권한 확인 + 세션 관리**를 모두 포함한다:

```rust
// 현재 — 실제 코드 (간략화)
async fn receive(&self) -> Result<Option<IncomingMessage>> {
    loop {                                              // ← 무한 루프
        let updates = self.poll_updates().await?;       // ← 30초 long poll
        for update in updates {
            // 권한 확인, /new, /session 명령 처리, 세션 관리
            return Ok(Some(incoming));                  // ← 메시지 하나 반환
        }
    }
}
```

이 로직 전체가 `start()`의 백그라운드 태스크로 이동한다. **핵심 변경:** `self`를 `Arc<Self>`로 공유하기 위해 `#[derive(Clone)]` 추가가 필요하다. 모든 필드가 이미 `Arc<RwLock<>>`, `String`, `Vec`, `reqwest::Client` (Clone)이므로 derive가 가능하다.

```rust
// 변경 후 — TelegramChannel에 #[derive(Clone)] 추가 필요
#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str { "telegram" }

    async fn start(
        &self,
        tx: mpsc::Sender<GatewayInbox>,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<JoinHandle<()>> {
        // Clone → Arc로 래핑하여 태스크로 이동
        let this = Arc::new(self.clone());
        let channel_name = this.name().to_owned();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    updates_result = this.poll_updates() => {
                        match updates_result {
                            Ok(updates) => {
                                for update in updates {
                                    // ── 메시지 추출 (기존 receive() 그대로) ──
                                    let message = update
                                        .get("message")
                                        .or_else(|| update.get("channel_post"))
                                        .or_else(|| update.get("edited_message"));
                                    let Some(msg) = message else { continue };

                                    let chat_id = msg.get("chat")
                                        .and_then(|c| c.get("id"))
                                        .and_then(|id| id.as_i64());
                                    let user_id = msg.get("from")
                                        .and_then(|f| f.get("id"))
                                        .and_then(|id| id.as_i64());
                                    let text = msg.get("text")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("");
                                    let message_id = msg.get("message_id")
                                        .and_then(|id| id.as_i64())
                                        .unwrap_or(0);

                                    if text.is_empty() { continue; }

                                    // ── 권한 확인 (기존 그대로) ──
                                    if let Some(uid) = user_id {
                                        if !this.is_user_allowed(uid) {
                                            tracing::warn!(user_id = uid, "Unauthorized");
                                            if let Some(cid) = chat_id {
                                                let _ = this.send_text(
                                                    cid, "Unauthorized.", None,
                                                ).await;
                                            }
                                            continue;
                                        }
                                    }

                                    let Some(cid) = chat_id else { continue };
                                    let user_id_str = user_id
                                        .map(|id| id.to_string())
                                        .unwrap_or_else(|| "unknown".into());

                                    // ── /new 명령 (기존 그대로) ──
                                    let trimmed = text.trim();
                                    if trimmed == "/new" || trimmed == "/new@me" {
                                        let sid = this.force_new_session(cid).await;
                                        let _ = this.send_text(
                                            cid,
                                            &format!("🔄 새 세션을 시작합니다.\\n`{}`", &sid[..8]),
                                            Some(message_id),
                                        ).await;
                                        continue;
                                    }

                                    // ── /session 명령 (기존 그대로) ──
                                    if trimmed == "/session" || trimmed == "/session@me" {
                                        let sessions = this.chat_sessions.read().await;
                                        let info = if let Some(s) = sessions.get(&cid) {
                                            format!(
                                                "📋 현재 세션\\n• ID: `{}`\\n• 메시지: {}개\\n• 시작: {}\\n• 마지막 활동: {}",
                                                &s.session_id[..8],
                                                s.message_count,
                                                s.created_at.format("%m/%d %H:%M"),
                                                s.last_active_at.format("%m/%d %H:%M"),
                                            )
                                        } else {
                                            "📋 활성 세션이 없습니다.".to_string()
                                        };
                                        drop(sessions);
                                        let _ = this.send_text(cid, &info, Some(message_id)).await;
                                        continue;
                                    }

                                    // 기타 /명령 스킵
                                    if text.starts_with('/') { continue; }

                                    // ── 세션 관리 + Gateway로 push ──
                                    let session_id = this.get_or_create_session(cid).await;

                                    let mut metadata = HashMap::new();
                                    metadata.insert("chat_id".to_string(), cid.to_string());
                                    metadata.insert("message_id".to_string(), message_id.to_string());
                                    metadata.insert("session_id".to_string(), session_id);

                                    let incoming = IncomingMessage {
                                        channel: "telegram".to_string(),
                                        user_id: user_id_str,
                                        content: text.to_string(),
                                        metadata,
                                        ..Default::default()
                                    };

                                    tracing::info!(
                                        chat_id = cid,
                                        text = %text.chars().take(50).collect::<String>(),
                                        "Telegram message received"
                                    );

                                    if tx.send((channel_name.clone(), incoming)).await.is_err() {
                                        break; // Gateway 수신구 닫힘
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "Telegram poll error");
                                tokio::time::sleep(Duration::from_secs(5)).await;
                            }
                        }
                    }

                    _ = shutdown.changed() => {
                        tracing::info!(channel = %channel_name, "Telegram channel stopped");
                        break;
                    }
                }
            }
        });

        Ok(handle)
    }

    // send() — 기존과 동일
    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        let chat_id: i64 = msg.metadata.get("chat_id")
            .and_then(|id| id.parse().ok())
            .or_else(|| msg.user_id.parse().ok())
            .ok_or_else(|| anyhow::anyhow!("No chat_id for Telegram message"))?;

        let reply_to = msg.metadata.get("message_id")
            .and_then(|id| id.parse().ok());

        self.send_text(chat_id, &msg.content, reply_to).await?;
        tracing::debug!(chat_id = chat_id, "Telegram response sent");
        Ok(())
    }
}
```

**TelegramChannel 마이그레이션 체크리스트:**

1. `#[derive(Clone)]` 추가 — 모든 필드가 Clone 가능함을 확인
2. `receive()` 제거, `start()`로 로직 이동
3. `return Ok(Some(incoming))` → `tx.send((channel_name, incoming)).await`
4. `self` → `Arc::new(self.clone())` 으로 태스크에 전달
5. `send()` 변경 없음

### 3.5 `LegacyChannelAdapter` (Phase 1 전환용)

새 trait으로 전환하는 동안 아직 `start()`를 구현하지 않은 채널을 위한 어댑터. Phase 3에서 제거.

```rust
/// 기존 receive()-기반 채널을 새 start() 패턴으로 래핑.
///
/// `C`를 `Arc`로 래핑하여 `'static` 태스크로 이동 가능하게 한다.
pub struct LegacyChannelAdapter<C: ChannelV1> {
    inner: Arc<C>,
}

impl<C: ChannelV1> LegacyChannelAdapter<C> {
    pub fn new(channel: C) -> Self {
        Self { inner: Arc::new(channel) }
    }
}

/// 기존 Channel trait (receive 기반) — Phase 3까지 임시 보존
#[async_trait]
pub trait ChannelV1: Send + Sync {
    fn name(&self) -> &str;
    async fn receive(&self) -> Result<Option<IncomingMessage>>;
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;
}

#[async_trait]
impl<C: ChannelV1 + 'static> Channel for LegacyChannelAdapter<C> {
    fn name(&self) -> &str { self.inner.name() }

    async fn start(
        &self,
        tx: mpsc::Sender<GatewayInbox>,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<JoinHandle<()>> {
        let channel_name = self.inner.name().to_owned();
        let inner = self.inner.clone(); // Arc clone → 'static 이동 가능

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = inner.receive() => {
                        match msg {
                            Ok(Some(msg)) => {
                                if tx.send((channel_name.clone(), msg)).await.is_err() {
                                    break;
                                }
                            }
                            Ok(None) => break,
                            Err(e) => {
                                tracing::warn!(error = %e, "Legacy channel receive error");
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                        }
                    }
                    _ = shutdown.changed() => break,
                }
            }
        });

        Ok(handle)
    }

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        self.inner.send(msg).await
    }
}
```

---

## 4. Backpressure 전략

메시지 흐름에 두 가지 제어점이 있다:

### 4.1 수신 backpressure: mpsc bounded 채널

| 상황 | 동작 | 이유 |
|------|------|------|
| `tx.send().await` 블로킹 | 채널 수신 태스크가 일시 정지 | 자연스러운 backpressure |
| 버퍼 1024 충족 | 새 메시지 대기 | Orchestrator 처리 속도가 수신 속도를 따라잡을 때까지 |

1024 버퍼는 충분하다. 오케스트레이션 평균 3초, 최대 32 동시 처리 시 초당 ~10개 처리. 1024면 100초분의 버퍼.

### 4.2 처리 backpressure: Semaphore

| 상황 | 동작 |
|------|------|
| 32개 이하 동시 처리 | `acquire()` 즉시 반환 |
| 32개 초과 | `acquire()` 대기 → 이벤트 루프는 계속 수신하지만 dispatch가 느려짐 |
| mpsc 버퍼 + Semaphore 대기 동시 발생 | 수신 태스크의 `tx.send()`도 블로킹 → 전체 흐름 자동 조절 |

별도 드롭 정책은 도입하지 않는다. `send().await` + `Semaphore`의 이중 backpressure로 충분하다.

---

## 5. 마이그레이션 계획

### Phase 1: Gateway 코어 + LegacyChannelAdapter (1-2일)

**목표:** 새 Gateway 구조를 갖추고 기존 채널이 여전히 동작하게 유지.

| # | 작업 | 파일 | 상세 |
|---|------|------|------|
| 1 | `GatewayInbox` 타입 추가 | `lib.rs` | `pub type GatewayInbox = (String, IncomingMessage)` |
| 2 | `Channel` trait v2 | `channel.rs` | `start()` 추가, 기존 `receive()`에 `#[deprecated]` |
| 3 | `Gateway` 재작성 | `gateway.rs` | `Arc<RwLock<HashMap>>`, `mpsc`, `Semaphore`, `dispatch()` |
| 4 | `LegacyChannelAdapter` 추가 | `channel.rs` | 기존 `receive()` 채널 임시 지원 |
| 5 | `register()` 변경 | `gateway.rs` | `start()` 호출 + `Arc<dyn Channel>` 보관 |
| 6 | `main.rs` 실행 변경 | `main.rs` | `std::thread::spawn` → `tokio::spawn` |
| 7 | `kernel.rs` 조정 | `kernel.rs` | `register_channel()` → `Result<()>` 반환 |
| 8 | 통합 테스트 업데이트 | `tests/` | 새 Gateway API |

### Phase 2: 채널 마이그레이션 (2-3일)

| 채널 | 난이도 | 상세 |
|------|--------|------|
| WebChannel | 낮음 | `receive()` → `start()` 브릿지. `send()` 변경 없음. `WebChannelHandle` 변경 없음 |
| CliChannel | 낮음 | Web과 동일. `InteractiveLoop` 변경 없음 |
| TelegramChannel | **중간** | `receive()` 전체 로직을 `start()`로 이동. `#[derive(Clone)]` 추가. `Arc<Self>`로 태스크에 전달 |

### Phase 3: 정리 (1일)

| # | 작업 |
|---|------|
| 1 | `LegacyChannelAdapter` 제거 |
| 2 | `ChannelV1` trait 제거 |
| 3 | `receive()` 메서드 완전 제거 |
| 4 | `#[deprecated]` 속성 제거 |
| 5 | `main.rs` 전용 스레드 코드 완전 삭제 |
| 6 | `cargo test --workspace` pass 확인 |

---

## 6. 영향 범위

| 컴포넌트 | 변경 | 상세 |
|----------|------|------|
| `oxios-gateway` | **대폭** | `gateway.rs`, `channel.rs` 핵심 재작성. `lib.rs` 타입 추가. `plugin.rs` 변경 없음. `message.rs` 변경 없음 |
| `channels/oxios-web` | 소폭 | `channel.rs`에 `start()` 구현. `plugin.rs` 변경 없음 |
| `channels/oxios-cli` | 소폭 | `channel.rs`에 `start()` 구현. `plugin.rs`, `interactive.rs` 변경 없음 |
| `channels/oxios-telegram` | 중간 | `lib.rs`에 `start()` 구현 + `#[derive(Clone)]`. `plugin.rs` 변경 없음 |
| `src/main.rs` | 소폭 | Gateway 실행 방식 (thread → spawn) |
| `src/kernel.rs` | 소폭 | `register_channel()` 시그니처 |
| `oxios-kernel` | 없음 | Gateway는 kernel 독립 |
| `oxios-web/web` | 없음 | HTTP/WebSocket API 변경 없음 |

---

## 7. 위험 및 완화

| 위험 | 확률 | 영향 | 완화 |
|------|------|------|------|
| Telegram long poll 태스크 충돌 | 낮음 | 중간 | `tokio::select!`로 poll + shutdown 동시 대기. 에러 시 exponential backoff |
| Semaphore 병목 | 낮음 | 중간 | 기본 32 = I/O-bound 작업에 충분. `OxiosConfig`로 런타임 조정 가능 |
| Telegram `self`를 태스크로 이동 | 중간 | 중간 | `#[derive(Clone)]` + `Arc::new(self.clone())`. 모든 필드 Clone 가능 확인 완료 |
| 메시지 순서 | 낮음 | 중간 | 채널 내 순서는 mpsc FIFO로 보장. 채널 간 + 동시 처리 시 순서 미보장 (기존과 동일) |
| 마이그레이션 중 회귀 | 중간 | 높음 | Phase 1에서 `LegacyChannelAdapter`로 기존 동작 보존. 통합 테스트로 방어 |
| thread → spawn 전환 | 낮음 | 낮음 | `parking_lot` 의존 코드가 Gateway에 없으면 문제 없음 |

---

## 8. 성공 기준

- [ ] Telegram long poll이 Web/CLI 수신을 블로킹하지 않음 (독립 태스크)
- [ ] 여러 메시지가 동시에 처리됨 (Semaphore 제한 내)
- [ ] Gateway 이벤트 루프에 write lock 없음
- [ ] idle 시 CPU 사용률 ≈ 0% (`tokio::select!` 이벤트 대기)
- [ ] 기존 기능 회귀 없음 (통합 테스트 전부 pass)
- [ ] 각 채널 독립 start/stop 가능 (`register`/`unregister`)
- [ ] Gateway가 메인 tokio 런타임에서 실행 (별도 스레드 불필요)
- [ ] `cargo test --workspace` 성공
