# RFC-013: Gateway Event-Driven 마이그레이션

> **상태:** 📝 설계
> **날짜:** 2026-05-26
> **우선순위:** P0
> **범위:** `crates/oxios-gateway/`, `channels/`
> **선행:** 없음
> **후행:** RFC-014 (채널 UX 통일)

---

## 1. 동기

현재 Gateway는 폴링 기반 이벤트 루프다:

```rust
// 현재 gateway.rs run()
loop {
    for name in &channel_names {
        let mut channels = self.channels.write().await;  // ← WRITE LOCK
        if let Some(ch) = channels.get_mut(name) {
            let msg = ch.receive().await.ok().flatten(); // ← 최대 30초 블로킹 (Telegram)
        }
    }
    // adaptive sleep: yield_now() or 50ms
}
```

**문제:**
1. **전체 채널 블로킹**: Telegram 30초 long poll이 Web/CLI 수신을 모두 차단
2. **불필요한 write lock**: `receive()`는 읽기만 하는데 전체 맵에 write lock
3. **순차 처리**: 한 채널의 버스트가 다른 채널 지연
4. **불필요한 CPU 폴링**: idle 시 50ms 간격 무의미한 루프

---

## 2. 설계

### 2.1 핵심 변경: Push 모델

각 채널이 **공유 mpsc 채널로 메시지를 push**하고, Gateway는 `tokio::select!`로 수신:

```rust
// 새 gateway.rs
pub struct Gateway {
    // 채널별 sender 보관 (등록/해제용)
    channels: RwLock<HashMap<String, ChannelEntry>>,
    // 모든 채널의 메시지가 모이는 통합 수신구
    rx: mpsc::Receiver<(String, IncomingMessage)>,
    tx: mpsc::Sender<(String, IncomingMessage)>,
}

struct ChannelEntry {
    send_tx: mpsc::Sender<OutgoingMessage>,
    shutdown: watch::Sender<bool>,
}
```

### 2.2 Channel Trait 변경

```rust
// 변경 전
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn receive(&mut self) -> Result<Option<IncomingMessage>>;
    async fn send(&mut self, msg: OutgoingMessage) -> Result<()>;
}

// 변경 후
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;

    /// 채널을 백그라운드에서 시작. 메시지가 들어오면 tx로 push.
    /// 종료 시 shutdown이 발동.
    async fn start(
        &self,
        tx: mpsc::Sender<(String, IncomingMessage)>,
        shutdown: watch::Receiver<bool>,
    ) -> Result<()>;

    /// 응답 전송. Gateway가 호출.
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;
}
```

### 2.3 Gateway 이벤트 루프

```rust
impl Gateway {
    pub async fn run(&self, orchestrator: Arc<dyn OrchestratorApi>) -> Result<()> {
        loop {
            tokio::select! {
                // 어떤 채널에서든 메시지 도착
                Some((channel_name, msg)) = self.rx.recv() => {
                    let response = orchestrator.handle_message(
                        &channel_name, &msg.content, msg.metadata.clone()
                    ).await;

                    if let Ok(result) = response {
                        let outgoing = OutgoingMessage::from_result(&msg, &result);
                        if let Some(entry) = self.channels.read().await.get(&channel_name) {
                            let _ = entry.send_tx.send(outgoing).await;
                        }
                    }
                }

                // 종료 신호
                _ = self.shutdown.changed() => {
                    info!("Gateway shutting down");
                    break;
                }
            }
        }
        Ok(())
    }
}
```

### 2.4 채널별 마이그레이션

#### WebChannel (기존 구조와 유사)

```rust
impl Channel for WebChannel {
    async fn start(&self, tx: Sender, mut shutdown: watch::Receiver<bool>) -> Result<()> {
        // 기존 mpsc 수신 루프를 그대로 활용
        let internal_rx = self.internal_rx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = internal_rx.recv() => {
                        if let Some(msg) = msg {
                            let _ = tx.send(("web".into(), msg)).await;
                        }
                    }
                    _ = shutdown.changed() => break,
                }
            }
        });
        Ok(())
    }

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        // 기존 oneshot + broadcast 로직 그대로
    }
}
```

#### CliChannel (새로운 구조)

```rust
impl Channel for CliChannel {
    async fn start(&self, tx: Sender, mut shutdown: watch::Receiver<bool>) -> Result<()> {
        let tx = tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // readline은 블로킹이므로 spawn_blocking
                    line = tokio::task::spawn_blocking(|| {
                        reedline::read_line() // pseudocode
                    }) => {
                        if let Ok(Some(input)) = line {
                            let msg = IncomingMessage::new("cli", &input);
                            let _ = tx.send(("cli".into(), msg)).await;
                        }
                    }
                    _ = shutdown.changed() => break,
                }
            }
        });
        Ok(())
    }

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        // 응답을 stdout에 출력 (기존과 동일)
        println!("{}", msg.content);
        Ok(())
    }
}
```

#### TelegramChannel (핵심 개선)

```rust
impl Channel for TelegramChannel {
    async fn start(&self, tx: Sender, mut shutdown: watch::Receiver<bool>) -> Result<()> {
        let bot = self.bot.clone();
        let tx = tx.clone();

        tokio::spawn(async move {
            let mut offset: i64 = 0;
            loop {
                tokio::select! {
                    // 30초 long poll — 하지만 이제 다른 채널을 블로킹하지 않음
                    updates = bot.get_updates().offset(offset).timeout(30).await => {
                        match updates {
                            Ok(updates) => {
                                for update in updates {
                                    offset = update.update_id + 1;
                                    if let Some(msg) = update.message {
                                        let incoming = IncomingMessage::from_telegram(&msg);
                                        let _ = tx.send(("telegram".into(), incoming)).await;
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Telegram poll error: {}", e);
                                tokio::time::sleep(Duration::from_secs(5)).await;
                            }
                        }
                    }
                    _ = shutdown.changed() => break,
                }
            }
        });
        Ok(())
    }
}
```

---

## 3. 마이그레이션 계획

### Phase 1: Gateway 코어 변경 (1-2일)

| 작업 | 파일 | 설명 |
|------|------|------|
| `Gateway` 구조체 변경 | `gateway.rs` | `mpsc` 채널 추가, polling 루프 → `tokio::select!` |
| `Channel` trait 변경 | `channel.rs` | `receive()` 제거, `start()` 추가 |
| `ChannelPlugin` 조정 | `plugin.rs` | `ChannelBundle`에서 sender/shutdown 전달 |
| 하위 호환 adapter | `channel.rs` | 기존 `receive()` 기반 채널을 위한 `LegacyChannelAdapter` |

### Phase 2: 채널 마이그레이션 (2-3일)

| 채널 | 난이도 | 비고 |
|------|--------|------|
| WebChannel | 낮음 | 이미 mpsc 기반, start() 래핑만 필요 |
| TelegramChannel | 낮음 | long poll 루프를 start() 안으로 이동 |
| CliChannel | 중간 | `spawn_blocking`으로 readline 래핑 필요 |

### Phase 3: 정리 (1일)

- `LegacyChannelAdapter` 제거
- 기존 `receive()` trait 메서드 제거
- 통합 테스트 업데이트

---

## 4. 영향 범위

| 컴포넌트 | 변경 필요? | 비고 |
|----------|-----------|------|
| `oxios-gateway` | ✅ 대폭 변경 | 핵심 모듈 |
| `channels/oxios-web` | ⚠️ 소폭 | `start()` 구현 |
| `channels/oxios-cli` | ⚠️ 소폭 | `start()` 구현 |
| `channels/oxios-telegram` | ⚠️ 소폭 | `start()` 구현 |
| `oxios-kernel` | ❌ 없음 | Gateway는 kernel 독립 |
| `oxios-web/web` | ❌ 없음 | 프론트엔드 변경 없음 |

---

## 5. 위험 및 완화

| 위험 | 확률 | 영향 | 완화 |
|------|------|------|------|
| Telegram long poll 백그라운드 태스크 충돌 | 낮음 | 중간 | 재시도 + exponential backoff |
| mpsc 채널 버퍼 오버플로우 | 낮음 | 낮음 | bounded 채널 (버퍼 1024) + backpressure |
| 마이그레이션 중 기존 채널 동시 지원 | 중간 | 낮음 | `LegacyChannelAdapter`로 점진적 전환 |
| 메시지 순서 보장 | 낮음 | 중간 | 채널 내 순서는 보장 (채널 간는 보장 안함 — 기존과 동일) |

---

## 6. 성공 기준

- [ ] Telegram long poll이 Web/CLI 수신을 블로킹하지 않음
- [ ] Gateway 이벤트 루프에 write lock 없음
- [ ] idle 시 CPU 사용률 0% (50ms 폴링 제거)
- [ ] 기존 기능 회귀 없음 (통합 테스트 통과)
- [ ] 각 채널 독립 start/stop 가능
