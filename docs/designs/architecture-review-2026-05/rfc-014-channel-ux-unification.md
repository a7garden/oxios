# RFC-014: 채널 UX 통일 (v2)

> **상태:** ✅ 구현 완료
> **날짜:** 2026-05-27
> **우선순위:** P0-P1
> **범위:** `crates/oxios-gateway/`, `channels/oxios-cli/`, `channels/oxios-telegram/`, `surface/oxios-web/`
> **선행:** RFC-013 (Gateway Event-Driven) — ✅ 완료
> **후행:** 없음
> **독립:** RFC-015~019와 겹침 없음

---

## 0. 개정 이유

v1 설계에서 다음 문제가 발견되었다:

| # | 문제 | 심각도 |
|---|------|--------|
| 1 | `ChannelResponse`가 기존 `OutgoingMessage`와 중복. 새 타입을 만들면 변환 보일러플레이트만 양산 | 🟡 |
| 2 | `ErrorKind` 분류가 `anyhow::Error` 체인에서 불가능. thiserror 전면 마이그레이션이 선행 조건인데 누락 | 🔴 |
| 3 | CLI reedline이 동기 블로킹. `send_and_wait`를 쓰면 readline 전체가 멈춤. 해법 불가 | 🔴 |
| 4 | Web의 `ChatResponse`가 `ChannelResponse`와 필드명 불일치. 프론트엔드 파서 깨짐 | 🟡 |
| 5 | Space 지원에 `handle_message()` 시그니처 변경이 필요한데 누락 | 🟡 |
| 6 | Gateway가 이미 `seed_id`, `agent_id`, `space_tag`, `duration_ms`를 버리고 있음 | 🟡 |
| 7 | Telegram UTF-8 청킹 버그, 타임아웃 미설정 등 기존 버그 미포함 | 🟡 |

v2는 이 문제들을 모두 해결한다.

---

## 1. 동기

### 1.1 현재 아키텍처 (RFC-013 완료 후)

RFC-013이 완료된 상태. Gateway는 event-driven 구조다:

```
채널 start() 태스크 ──mpsc──→ Gateway 이벤트 루프 ──tokio::spawn──→ dispatch()
                                      ↑                              │
                                      └─── 각 메시지 독립 처리        ↓
                                                              channel.send(OutgoingMessage)
```

**이미 갖춰진 인프라:**
- `OutgoingMessage.metadata`에 `session_id`, `space_id`, `phase`, `evaluation_passed` 포함
- `OutgoingMessage.id`가 `IncomingMessage.id`와 동일 → 상관관계 메커니즘 존재
- Web 채널은 oneshot `HashMap<Uuid, Sender>`로 요청-응답 상관관계 구현 완료
- Gateway dispatch가 Semaphore(32)로 동시 처리 제한

### 1.2 남은 문제: 메타데이터가 존재하지만 소비되지 않는다

| 메타데이터 | Web | CLI | Telegram |
|-----------|-----|-----|----------|
| `session_id` | ✅ 표시 | ❌ 버림 | ❌ 버림 |
| `space_id` | ✅ 표시 | ❌ 버림 | ❌ 버림 |
| `phase` | ✅ 표시 | ❌ 버림 | ❌ 버림 |
| `evaluation_passed` | ✅ 표시 | ❌ 버림 | ❌ 버림 |
| `space_tag` | ❌ 전달 안됨* | ❌ 전달 안됨 | ❌ 전달 안됨 |
| `seed_id` | ❌ 전달 안됨* | ❌ 전달 안됨 | ❌ 전달 안됨 |
| `duration_ms` | ❌ 측정 안됨 | ❌ 측정 안됨 | ❌ 측정 안됨 |
| 에러 구조화 | AppError (JSON) | `format!("An error occurred: {e}")` | 동일 (plain text) |

\* Gateway `dispatch()`가 `OrchestrationResult`에서 `seed_id`, `agent_id`, `space_tag`를 추출하지 않고 버림.

### 1.3 채널별 구체적 문제

**CLI (`channels/oxios-cli/`):**
- `CliChannel::send()`가 `println!("{}", msg.content)`만 실행 — 메타데이터 전부 무시
- Fire-and-forget: 응답 도착 전에 다음 프롬프트 표시, 사용자 혼란
- 응답이 stdout에 비동기 dump → 사용자 입력 중간에 끼어들 수 있음
- `.reset`이 커널 대화 상태를 초기화하지 않음 (cosmetic only)
- `.model`, `.persona`가 stub (TODO)

**Telegram (`channels/oxios-telegram/`):**
- `as_bytes().chunks(4000)`이 멀티바이트 UTF-8 분할 → 데이터 손상 가능
- `reqwest::Client::new()`에 타임아웃 미설정 → 무한 대기 가능
- 폴링 에러 시 5초 고정 대기 → 지수 백오프 없음
- 타이핑 인디케이터 없음 → 긴 작업 시 봇 고장으로 착각
- 오케스트레이션 메타데이터 전부 무시

**Web (`surface/oxios-web/`):**
- WebSocket이 `user_id`를 `"web-user"`로 하드코딩
- `AgentResponse.seed_id`가 항상 `None` (Gateway가 `seed_id`를 전달하지 않음)
- `oxios run --json`이 gateway를 우회 → 별도의 JSON 출력 형식 유지

---

## 2. 설계 원칙

1. **OutgoingMessage를 확장한다. 새 타입을 만들지 않는다.**
   - `ChannelResponse`를 새로 정의하지 않고, 기존 `OutgoingMessage`에 typed metadata를 추가
   - 변환 보일러플레이트 제거

2. **에러 분류는 점진적으로. 전면 thiserror 마이그레이션은 하지 않는다.**
   - `UserFacingError` 래퍼를 도입. kernel 에러는 점진적으로 opt-in
   - 분류 실패 시 `ErrorKind::Internal`로 폴백

3. **CLI는 send_and_wait 대신 "순차 입력" 모델을 쓴다.**
   - 처리 중인 요청이 있으면 새 입력을 reject
   - reedline 교체 없이 구현 가능
   - 미래: async 터미널(ratatui) 전환 시 true async UX 가능

4. **Web 응답 형식은 그대로. ChannelFormatter는 채널 내부 구현.**
   - `ChatResponse`는 Web route handler의 렌더링 책임
   - Gateway는 `OutgoingMessage`만 전달. 포매팅은 각 채널이 담당

5. **기존 버그를 Phase 0에서 먼저 고친다.**

---

## 3. 설계

### 3.1 OutgoingMessage 확장: typed metadata

```rust
// crates/oxios-gateway/src/message.rs

/// 오케스트레이션 결과 메타데이터.
///
/// Gateway dispatch()가 OrchestrationResult에서 추출하여 OutgoingMessage에 부착.
/// 기존 HashMap<String, String> metadata는 채널별 데이터(chat_id, message_id 등)용으로 유지.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMeta {
    pub session_id: Option<String>,
    pub space_id: Option<String>,
    pub space_tag: Option<String>,
    pub seed_id: Option<String>,
    pub phase: String,              // "Interview" | "Seed" | "Execute" | "Evaluate" | "Evolve"
    pub evaluation_passed: bool,
    pub duration_ms: Option<u64>,
    pub error: Option<UserFacingError>,
}

/// 사용자에게 표시되는 구조화된 에러.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFacingError {
    /// 사용자에게 보여줄 메시지 (한국어).
    pub message: String,
    /// 에러 분류.
    pub kind: ErrorKind,
    /// 복구 제안.
    pub suggestion: Option<String>,
}

/// 에러 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorKind {
    /// 에이전트 실행 실패
    ExecutionFailed,
    /// LLM 프로바이더 오류 (rate limit, API 오류 등)
    ProviderError,
    /// 시간 초과
    Timeout,
    /// 권한 부족
    PermissionDenied,
    /// 입력 검증 실패
    ValidationError,
    /// 시스템 내부 오류 (사용자에게 상세 노출 안함)
    Internal,
}

// ── OutgoingMessage 확장 ──

impl OutgoingMessage {
    /// 성공 응답 생성 (기존 with_id_and_metadata + ResponseMeta).
    pub fn success(
        correlation_id: Uuid,
        channel: &str,
        user_id: &str,
        content: &str,
        channel_meta: HashMap<String, String>,
        response_meta: ResponseMeta,
    ) -> Self {
        Self {
            id: correlation_id,
            channel: channel.to_string(),
            user_id: user_id.to_string(),
            content: content.to_string(),
            timestamp: Utc::now(),
            metadata: channel_meta,
            meta: Some(response_meta),
        }
    }

    /// 에러 응답 생성.
    pub fn error(
        correlation_id: Uuid,
        channel: &str,
        user_id: &str,
        err: UserFacingError,
    ) -> Self {
        Self {
            id: correlation_id,
            channel: channel.to_string(),
            user_id: user_id.to_string(),
            content: err.message.clone(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            meta: Some(ResponseMeta {
                session_id: None,
                space_id: None,
                space_tag: None,
                seed_id: None,
                phase: String::new(),
                evaluation_passed: false,
                duration_ms: None,
                error: Some(err),
            }),
        }
    }
}
```

기존 `OutgoingMessage`에 `#[serde(default)]` 필드 하나 추가:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingMessage {
    pub id: Uuid,
    pub channel: String,
    pub user_id: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    /// RFC-014: typed 오케스트레이션 메타데이터.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<ResponseMeta>,
}
```

`#[serde(default)]` + `skip_serializing_if`로 기존 JSON 소비자(Web 프론트엔드 등)에게 영향 없음.

### 3.2 에러 분류: 점진적 접근

전면 thiserror 마이그레이션 대신, Gateway dispatch에서 anyhow 에러를 분류:

```rust
// crates/oxios-gateway/src/error_classify.rs (신규)

/// anyhow::Error를 사용자 친화적 에러로 분류.
///
/// kernel이 thiserror를 도입하면 downcast로 정확한 분류 가능.
/// 현재는 휴리스틱 + 타입 체크의 하이브리드.
pub fn classify_error(e: &anyhow::Error) -> UserFacingError {
    let kind = infer_kind(e);
    let message = user_message(&kind);
    let suggestion = suggest(&kind);

    UserFacingError { message, kind, suggestion }
}

fn infer_kind(e: &anyhow::Error) -> ErrorKind {
    // 1. 타입 기반 분류 (정확)
    if e.is::<tokio::time::error::Elapsed>() {
        return ErrorKind::Timeout;
    }

    // 2. cause chain 순회
    let mut source = e.source();
    while let Some(err) = source {
        if err.is::<tokio::time::error::Elapsed>() {
            return ErrorKind::Timeout;
        }
        source = err.source();
    }

    // 3. 메시지 패턴 매칭 (휴리스틱)
    let msg = e.to_string().to_lowercase();
    if msg.contains("rate limit") || msg.contains("api key") || msg.contains("provider") {
        return ErrorKind::ProviderError;
    }
    if msg.contains("permission") || msg.contains("unauthorized") || msg.contains("access denied") {
        return ErrorKind::PermissionDenied;
    }
    if msg.contains("timeout") || msg.contains("deadline exceeded") {
        return ErrorKind::Timeout;
    }
    if msg.contains("validation") || msg.contains("invalid") || msg.contains("empty") {
        return ErrorKind::ValidationError;
    }

    ErrorKind::Internal
}

fn user_message(kind: &ErrorKind) -> String {
    match kind {
        ErrorKind::ExecutionFailed => "요청을 처리하는 중 오류가 발생했습니다.".into(),
        ErrorKind::ProviderError => "AI 서비스에 일시적인 문제가 있습니다. 잠시 후 다시 시도해 주세요.".into(),
        ErrorKind::Timeout => "요청 처리 시간이 초과되었습니다.".into(),
        ErrorKind::PermissionDenied => "이 작업을 수행할 권한이 없습니다.".into(),
        ErrorKind::ValidationError => "입력이 올바르지 않습니다.".into(),
        ErrorKind::Internal => "내부 오류가 발생했습니다.".into(),
    }
}

fn suggest(kind: &ErrorKind) -> Option<String> {
    match kind {
        ErrorKind::ProviderError => Some("1-2분 후 다시 시도하거나 다른 모델을 선택하세요.".into()),
        ErrorKind::Timeout => Some("더 간단한 요청으로 시도하거나 타임아웃을 늘리세요.".into()),
        ErrorKind::PermissionDenied => Some("관리자에게 권한을 요청하세요.".into()),
        _ => None,
    }
}
```

### 3.3 ChannelFormatter trait

```rust
// crates/oxios-gateway/src/format.rs (신규)

/// 채널별 응답 포매팅 트레이트.
///
/// 각 채널이 자신의 출력 매체에 맞게 OutgoingMessage를 포맷.
/// trait은 gateway에 정의하고, 구현체는 각 채널 crate에 둔다.
/// 의존성 방향: channel → gateway (기존과 동일, 순환 없음).
pub trait ChannelFormatter: Send + Sync {
    /// 성공 응답 포맷.
    fn format_success(&self, msg: &OutgoingMessage) -> String;

    /// 에러 응답 포맷.
    fn format_error(&self, msg: &OutgoingMessage) -> String;

    /// 처리 중 상태 표시 (스티리밍 불가 채널용).
    fn format_progress(&self, phase: &str) -> String;
}
```

**포매터는 gateway가 호출하지 않는다.** 각 채널의 `send()` 내부에서 사용:

```rust
// channels/oxios-cli/src/channel.rs
struct CliChannel {
    formatter: CliFormatter,
    // ...
}

impl Channel for CliChannel {
    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        let output = match &msg.meta {
            Some(meta) if meta.error.is_some() => self.formatter.format_error(&msg),
            Some(meta) => self.formatter.format_success(&msg),
            None => msg.content.clone(), // fallback
        };
        println!("{}", output);
        Ok(())
    }
}
```

#### CliFormatter

```rust
// channels/oxios-cli/src/format.rs (신규)

pub struct CliFormatter;

impl ChannelFormatter for CliFormatter {
    fn format_success(&self, msg: &OutgoingMessage) -> String {
        let mut out = msg.content.clone();

        if let Some(meta) = &msg.meta {
            // Phase + 평가
            let eval_icon = if meta.evaluation_passed { "✅" } else { "⚠️" };
            if !meta.phase.is_empty() {
                out.push_str(&format!("\n{} {} | {}", eval_icon, meta.phase,
                    if meta.evaluation_passed { "통과" } else { "미통과" }));
            }

            // Space
            if let Some(tag) = &meta.space_tag {
                out.push_str(&format!(" | {}", tag));
            }

            // 소요 시간
            if let Some(dur) = meta.duration_ms {
                if dur >= 1000 {
                    out.push_str(&format!(" | {:.1}s", dur as f64 / 1000.0));
                } else {
                    out.push_str(&format!(" | {}ms", dur));
                }
            }
        }

        out
    }

    fn format_error(&self, msg: &OutgoingMessage) -> String {
        let meta = msg.meta.as_ref();
        let kind = meta.and_then(|m| m.error.as_ref()).map(|e| e.kind);

        let icon = match kind {
            Some(ErrorKind::ExecutionFailed) => "❌",
            Some(ErrorKind::ProviderError) => "🔌",
            Some(ErrorKind::Timeout) => "⏱️",
            Some(ErrorKind::PermissionDenied) => "🔒",
            Some(ErrorKind::ValidationError) => "⚠️",
            _ => "💥",
        };

        let mut out = format!("{} {}", icon, msg.content);

        if let Some(err) = meta.and_then(|m| m.error.as_ref()) {
            if let Some(s) = &err.suggestion {
                out.push_str(&format!("\n💡 {}", s));
            }
        }

        out
    }

    fn format_progress(&self, phase: &str) -> String {
        match phase {
            "Interview" => "🔍 분석 중...".into(),
            "Seed" => "📋 계획 수립 중...".into(),
            "Execute" => "⚡ 실행 중...".into(),
            "Evaluate" => "📊 평가 중...".into(),
            "Evolve" => "🔄 개선 중...".into(),
            _ => "⏳ 처리 중...".into(),
        }
    }
}
```

#### TelegramFormatter

```rust
// channels/oxios-telegram/src/format.rs (신규)

pub struct TelegramFormatter;

impl ChannelFormatter for TelegramFormatter {
    fn format_success(&self, msg: &OutgoingMessage) -> String {
        let mut out = msg.content.clone();

        if let Some(meta) = &msg.meta {
            let mut footer_parts = Vec::new();
            if !meta.phase.is_empty() {
                let eval = if meta.evaluation_passed { "✅" } else { "⚠️" };
                footer_parts.push(format!("{} {}", eval, meta.phase));
            }
            if let Some(tag) = &meta.space_tag {
                footer_parts.push(tag.clone());
            }
            if let Some(dur) = meta.duration_ms {
                footer_parts.push(format!("{:.1}s", dur as f64 / 1000.0));
            }
            if !footer_parts.is_empty() {
                out.push_str(&format!("\n\n_{}_", footer_parts.join(" · ")));
            }
        }

        out
    }

    fn format_error(&self, msg: &OutgoingMessage) -> String {
        let meta = msg.meta.as_ref();
        let kind = meta.and_then(|m| m.error.as_ref()).map(|e| e.kind);

        let icon = match kind {
            Some(ErrorKind::ProviderError) => "🔌",
            Some(ErrorKind::Timeout) => "⏱️",
            _ => "❌",
        };

        let mut out = format!("{} {}", icon, msg.content);

        if let Some(err) = meta.and_then(|m| m.error.as_ref()) {
            if let Some(s) = &err.suggestion {
                out.push_str(&format!("\n\n💡 _{}_", s));
            }
        }

        out
    }

    fn format_progress(&self, phase: &str) -> String {
        match phase {
            "Interview" => "🔍 분석 중...",
            "Seed" => "📋 계획 수립 중...",
            "Execute" => "⚡ 실행 중...",
            "Evaluate" => "📊 평가 중...",
            "Evolve" => "🔄 개선 중...",
            _ => "⏳ 처리 중...",
        }.into()
    }
}
```

#### WebFormatter

```rust
// surface/oxios-web/src/format.rs (신규)

/// Web은 OutgoingMessage를 그대로 JSON 직렬화하여 route handler에 전달.
/// 포매팅은 route handler의 책임 (ChatResponse, WS JSON 등).
/// 이 포매터는 identity 역할만 수행.
pub struct WebFormatter;

impl ChannelFormatter for WebFormatter {
    fn format_success(&self, msg: &OutgoingMessage) -> String {
        msg.content.clone() // 그대로. route handler가 ChatResponse 구성
    }

    fn format_error(&self, msg: &OutgoingMessage) -> String {
        msg.content.clone() // 그대로. AppError가 HTTP 상태 코드 매핑
    }

    fn format_progress(&self, _phase: &str) -> String {
        String::new() // Web은 스트리밍으로 실시간 업데이트. progress 불필요
    }
}
```

### 3.4 Gateway dispatch 개선

현재 dispatch에서 누락된 필드를 추가하고, `ResponseMeta`를 생성:

```rust
// crates/oxios-gateway/src/gateway.rs — dispatch() 내부

// ── 시간 측정 (세마포어 대기 포함 — 사용자 체감 대기 시간) ──
let start = std::time::Instant::now();

// ── 오케스트레이션 ──
let result = orchestrator
    .handle_message(&msg.user_id, &msg.content, session_id.as_deref())
    .await;

let duration_ms = start.elapsed().as_millis() as u64;

match (result, entry) {
    (Ok(orch), Some(entry)) => {
        // 기존 metadata (채널별 데이터 — chat_id, message_id 등)
        let mut channel_meta = HashMap::new();
        if let Some(ref sid) = orch.session_id {
            channel_meta.insert("session_id".to_owned(), sid.clone());
        }
        if let Some(ref vid) = orch.space_id {
            channel_meta.insert("space_id".to_owned(), vid.to_string());
        }

        // Typed metadata (NEW)
        let meta = ResponseMeta {
            session_id: orch.session_id,
            space_id: orch.space_id.map(|u| u.to_string()),
            space_tag: orch.space_tag,
            seed_id: orch.seed_id.map(|u| u.to_string()),       // NEW: 이전에 버려짐
            phase: orch.phase_reached.to_string(),
            evaluation_passed: orch.evaluation_passed,
            duration_ms: Some(duration_ms),                      // NEW: 이전에 측정 안됨
            error: None,
        };

        let outgoing = OutgoingMessage::success(
            msg.id, &msg.channel, &msg.user_id,
            &orch.response, channel_meta, meta,
        );

        if let Err(e) = entry.channel.send(outgoing).await {
            tracing::error!(error = %e, "Failed to send response");
        }
    }

    (Err(e), Some(entry)) => {
        tracing::error!(error = %e, "Orchestration failed");
        let user_err = classify_error(&e);
        // 에러 응답에도 session_id 보존 (대화 연속성 유지)
        let session_id = msg.metadata.get(meta::SESSION_ID).cloned();
        let mut outgoing = OutgoingMessage::error(msg.id, &msg.channel, &msg.user_id, user_err);
        if let Some(sid) = session_id {
            outgoing.metadata.insert(meta::SESSION_ID.to_string(), sid);
        }
        if let Err(e) = entry.channel.send(outgoing).await {
            tracing::error!(error = %e, "Failed to send error response");
        }
    }

    (_, None) => {
        tracing::warn!(channel = %channel_name, "Channel no longer registered");
    }
}
```

### 3.5 CLI: 순차 입력 모델

reedline은 동기 블로킹이므로 send_and_wait를 쓸 수 없다. 대신 **처리 중 입력 reject** 모델:

```
사용자 입력 → is_processing?
  ├─ No  → 전송 → processing = true → 프롬프트 변경 "⏳ oxios>"
  └─ Yes → "⏳ 이전 요청 처리 중..." 출력 → read_line 재진입

CliChannel::send() → 응답 출력 → processing = false → 프롬프트 복원 "oxios>"
```

```rust
// channels/oxios-cli/src/interactive.rs — 변경

struct InteractiveLoop {
    handle: CliChannelHandle,
    session: Arc<std::sync::Mutex<Session>>,
    processing: Arc<AtomicBool>,     // NEW
    prompt: Prompt,                   // NEW: 동적 프롬프트
}

impl InteractiveLoop {
    async fn run(&self) -> Result<()> {
        loop {
            let status = if self.processing.load(Ordering::Relaxed) {
                "⏳ oxios> "
            } else {
                "oxios> "
            };
            self.prompt.update(status);

            let line = self.editor.read_line(&self.prompt)?;
            // ...

            let input = line.trim().to_string();

            if self.processing.load(Ordering::Relaxed) {
                println!("⏳ 이전 요청을 처리 중입니다. 잠시만 기다려주세요.");
                continue;
            }

            // 메타 명령 처리 (기존) ...

            self.processing.store(true, Ordering::Relaxed);
            self.handle.send_user_message(input)?;
            // 응답은 CliChannel::send()에서 processing = false로 설정
        }
    }
}
```

```rust
// channels/oxios-cli/src/channel.rs — 변경

struct CliChannel {
    formatter: CliFormatter,
    incoming_rx: Mutex<mpsc::Receiver<IncomingMessage>>,
    incoming_tx: mpsc::Sender<IncomingMessage>,
    session: Arc<std::sync::Mutex<Session>>,
    processing: Arc<AtomicBool>,     // NEW: interactive loop과 공유
}

impl Channel for CliChannel {
    // ... start() 동일 ...

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        let output = match &msg.meta {
            Some(meta) if meta.error.is_some() => self.formatter.format_error(&msg),
            Some(_) => self.formatter.format_success(&msg),
            None => msg.content.clone(),
        };
        println!("{}", output);
        self.processing.store(false, Ordering::Relaxed);  // 입력 잠금 해제
        Ok(())
    }
```

#### TelegramChannel::send() — 포매터 통합

```rust
// channels/oxios-telegram/src/lib.rs — send() 수정

async fn send(&self, msg: OutgoingMessage) -> Result<()> {
    let chat_id: i64 = msg.metadata.get(meta::CHAT_ID)
        .and_then(|id| id.parse().ok())
        .or_else(|| msg.user_id.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("No chat_id for Telegram message"))?;

    let reply_to = msg.metadata.get(meta::MESSAGE_ID)
        .and_then(|id| id.parse().ok());

    // 포매터로 출력 포맷 결정
    let raw = match &msg.meta {
        Some(meta) if meta.error.is_some() => self.formatter.format_error(&msg),
        Some(_) => self.formatter.format_success(&msg),
        None => msg.content.clone(),
    };

    // Markdown-safe 분할 전송
    for chunk in split_message(&raw, 4000) {
        self.send_text(chat_id, &chunk, reply_to).await?;
    }

    Ok(())
}
}
```

**미래 개선 (별도 RFC):** reedline → ratatui + crossterm 교체로 진짜 async UX.

### 3.6 Telegram 개선

#### UTF-8 안전 청킹

```rust
// channels/oxios-telegram/src/lib.rs — send_text() 수정

/// 메시지를 4000자(바이트 아님) 단위로 분할.
fn split_message(text: &str, max_chars: usize) -> Vec<String> {
    if text.chars().count() <= max_chars {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut chars = text.chars().peekable();
    let mut current = String::new();

    while let Some(ch) = chars.next() {
        current.push(ch);
        if current.len() > max_chars {
            // 마지막 글자 제거 → 다음 청크로
            current.pop();
            chunks.push(std::mem::take(&mut current));
            current.push(ch);
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}
```

#### 타이핑 인디케이터

```rust
// start() 내부, 메시지 수신 시:
if tx.send((channel_name.clone(), incoming)).await.is_err() {
    break;
}
// 타이핑 인디케이터 전송
let _ = this.send_chat_action(cid, "typing").await;
```

```rust
impl TelegramChannel {
    async fn send_chat_action(&self, chat_id: i64, action: &str) -> Result<()> {
        self.client
            .post(format!("https://api.telegram.org/bot{}/sendChatAction", self.token))
            .json(&serde_json::json!({ "chat_id": chat_id, "action": action }))
            .send().await?;
        Ok(())
    }
}
```

#### HTTP 타임아웃 + 지수 백오프

```rust
// plugin.rs 또는 new()
let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(60))
    .build()?;

// poll error 시 지수 백오프
Err(e) => {
    tracing::warn!(error = %e, "Telegram poll error");
    let delay = Duration::from_secs(5 * 2u64.pow(retry_count.min(4))); // 5s, 10s, 20s, 40s, 80s
    tokio::time::sleep(delay).await;
    retry_count += 1;
}
Ok(_) => { retry_count = 0; /* ... */ }
```

### 3.7 Space 지원 (kernel API 전제 조건)

**현재 한계:** `orchestrator.handle_message(user_id, content, session_id)`에 `space_id` 파라미터가 없다. Space는 orchestrator 내부에서 자동 감지된다.

**변경 필요 (kernel 쪽):**

```rust
// crates/oxios-kernel/src/orchestrator.rs

/// handle_message에 space_id 파라미터 추가.
pub async fn handle_message(
    &self,
    user_id: &str,
    content: &str,
    session_id: Option<&str>,
    space_id: Option<&str>,      // NEW
) -> Result<OrchestrationResult> {
    // space_id가 제공되면 해당 Space를 직접 사용
    // None이면 기존 자동 감지 로직
}
```

**CLI 메타 명령:**

```
.space <tag>     — 현재 space 전환 (IncomingMessage.metadata에 space_id 추가)
.spaces          — space 목록 표시
.space           — 현재 space 확인
```

**Telegram 명령:**

```
/space <tag>     — space 전환
/spaces          — space 목록
```

**이 기능은 kernel API 변경이 선행되어야 하므로 Phase 3으로 분리.**

### 3.8 Web `user_id` 통일

```rust
// surface/oxios-web/src/routes/chat.rs — WebSocket 핸들러

// 변경 전:
let mut incoming = IncomingMessage::new("web", "web-user", content.clone());

// 변경 후: 인증 컨텍스트에서 user_id 추출, 없으면 "default"
let user_id = auth_context.user_id().unwrap_or("default");
let mut incoming = IncomingMessage::new("web", user_id, content.clone());
```

### 3.9 CLI/Telegram 메타데이터 상수화

```rust
// crates/oxios-gateway/src/meta.rs (신규)

/// 메타데이터 키 상수 — 모든 채널이 공유.
///
/// 장기적으로는 HashMap<String, String>을 typed struct로 교체해야 하지만,
/// 당장은 키 상수화만으로 오타 방지 가능.
pub mod meta {
    pub const SESSION_ID: &str = "session_id";
    pub const SPACE_ID: &str = "space_id";
    pub const CHAT_ID: &str = "chat_id";
    pub const MESSAGE_ID: &str = "message_id";
    pub const USER_ID: &str = "user_id";
}
```

---

## 4. 선행 조건

| 조건 | 이유 | 상태 |
|------|------|------|
| RFC-013 완료 | Event-driven gateway | ✅ 완료 |
| `OutgoingMessage.meta` 필드 추가 | serde 기본값으로 기존 호환 | Phase 1 |
| `handle_message()`에 `space_id` 추가 | Space 명령 지원 | Phase 3 |

---

## 5. 마이그레이션 계획

### Phase 0: 기존 버그 수정 (0.5일)

| # | 작업 | 파일 | 비고 |
|---|------|------|------|
| 1 | UTF-8 안전 청킹 | `telegram/lib.rs` | `chars()` 기반으로 교체 |
| 2 | reqwest 타임아웃 설정 | `telegram/lib.rs` | `Client::builder().timeout(60s)` |
| 3 | 폴링 지수 백오프 | `telegram/lib.rs` | `2^retry_count` 지연 |
| 4 | 에러 응답에 session_id 보존 | `gateway.rs` | `IncomingMessage.metadata`에서 추출 → 에러 응답에도 포함 |

### Phase 1: 통일 메타데이터 + 포매터 (1-2일)

| # | 작업 | 파일 | 비고 |
|---|------|------|------|
| 1 | `ResponseMeta` + `UserFacingError` + `ErrorKind` 정의 | `gateway/src/message.rs` | 타입 정의 |
| 2 | `OutgoingMessage.meta` 필드 추가 | `gateway/src/message.rs` | `#[serde(default)]` |
| 3 | `error_classify.rs` | `gateway/src/error_classify.rs` (신규) | anyhow → UserFacingError |
| 4 | `ChannelFormatter` trait | `gateway/src/format.rs` (신규) | trait 정의 |
| 5 | `meta.rs` 상수 | `gateway/src/meta.rs` (신규) | 메타데이터 키 상수 |
| 6 | Gateway dispatch에 `ResponseMeta` 생성 | `gateway/src/gateway.rs` | duration 측정 + meta 생성 |
| 7 | `CliFormatter` 구현 | `cli/src/format.rs` (신규) | ANSI 포맷 |
| 8 | `TelegramFormatter` 구현 | `telegram/src/format.rs` (신규) | 마크다운 + emoji |
| 9 | `WebFormatter` 구현 | `web/src/format.rs` (신규) | identity (그대로) |
| 10 | 각 채널 `send()`에서 포매터 사용 | 각 채널 `channel.rs` | `match meta.error` 분기 |

### Phase 2: CLI 순차 입력 + Web user_id (1-2일)

| # | 작업 | 파일 | 비고 |
|---|------|------|------|
| 1 | `processing: AtomicBool` 공유 | `cli/channel.rs`, `interactive.rs` | 입력 잠금 |
| 2 | 동적 프롬프트 (⏳ 상태) | `cli/interactive.rs` | reedline prompt 업데이트 |
| 3 | 응답 시 `processing = false` | `cli/channel.rs` | send() 내 |
| 4 | Web WS `user_id` 통일 | `web/routes/chat.rs` | `"web-user"` → 실제 ID |
| 5 | Web `ChatResponse`에 `meta` 필드 반영 | `web/routes/chat.rs` | seed_id, duration_ms 추가 |
| 6 | 기존 테스트 업데이트 | `cli/tests/`, `web/tests/` | |

### Phase 3: Space 지원 (1-2일)

| # | 작업 | 파일 | 비고 |
|---|------|------|------|
| 1 | `handle_message()`에 `space_id` 파라미터 추가 | `kernel/orchestrator.rs` | Optional, None이면 자동 |
| 2 | Gateway dispatch에서 `space_id` 메타데이터 전달 | `gateway.rs` | IncomingMessage.metadata → handle_message |
| 3 | CLI `.space` / `.spaces` 메타 명령 | `cli/commands.rs`, `interactive.rs` | |
| 4 | Telegram `/space` / `/spaces` 명령 | `telegram/lib.rs` | |
| 5 | Space 목록 API (CLI/Telegram용) | `kernel/kernel_handle/space_api.rs` | 기존에 있을 수 있음 |

### Phase 4: Telegram UX 개선 (0.5일)

| # | 작업 | 파일 | 비고 |
|---|------|------|------|
| 1 | 타이핑 인디케이터 | `telegram/lib.rs` | `sendChatAction` |
| 2 | 마크다운 새니타이제이션 | `telegram/format.rs` | `_`, `*`, `` ` `` escape |

---

## 6. 영향 범위

| 컴포넌트 | 변경 규모 | 상세 |
|----------|-----------|------|
| `oxios-gateway` | **중** | `message.rs` 확장, `error_classify.rs` 신규, `format.rs` 신규, `meta.rs` 신규, `gateway.rs` dispatch 개선 |
| `channels/oxios-cli` | **중** | `format.rs` 신규, `channel.rs` send() 개선, `interactive.rs` 순차 입력 |
| `channels/oxios-telegram` | **중** | `format.rs` 신규, `lib.rs` UTF-8+타임아웃+백오프+인디케이터 |
| `surface/oxios-web` (Rust) | **소** | `format.rs` 신규, `chat.rs` user_id 통일 + ChatResponse 확장 |
| `surface/oxios-web` (Frontend) | **없음** | 기존 JSON 소비 방식 유지. meta 필드는 무시(serde default) |
| `oxios-kernel/orchestrator.rs` | **소** | Phase 3에서 `handle_message` 시그니처 변경 |
| `src/cmd_run.rs` | **없음** | `oxios run --json`은 gateway 우회 유지. 별도 JSON 출력 형식 |

**변경 없는 것:**
- `Channel` trait — 변경 없음 (RFC-013에서 이미 개정 완료)
- `ChannelPlugin` / `ChannelBundle` — 변경 없음
- `IncomingMessage` — 변경 없음 (metadata HashMap 유지)
- Frontend — 영향 없음

---

## 7. 위험 및 완화

| 위험 | 확률 | 영향 | 완화 |
|------|------|------|------|
| `processing` AtomicBool race condition | 낮음 | 중 | single-threaded reedline + `Relaxed` ordering. CLI 단일 사용자 |
| 에러 분류 휴리스틱 부정확 | 중간 | 낮음 | `Internal` 폴백. kernel thiserror 점진 도입으로 정확도 향상 |
| Web 프론트엔드가 새 `meta` 필드 무시 | 의도 | 없음 | `#[serde(default, skip_serializing_if)]` 숨김 |
| `handle_message` 시그니처 변경 | 낮음 | 중 | Phase 3. `space_id: Option`, 기존 호출처에 `None` 추가 |
| Telegram 마크다운 새니타이제이션 불완전 | 중간 | 낮음 | 기존 fallback(plain text 재전송) 유지 |
| CLI 응답이 타이핑 도중 도착 | 높음 | 중 | reedline → ratatuiAsync 전환 시 해결. Phase 2에서는 `processing` 플래그로 부분 완화 |
| 마이그레이션 중 회귀 | 중간 | 높음 | Phase별 점진 적용. 각 Phase 끝에 `cargo test --workspace` |

---

## 8. 성공 기준

### Phase 0 (버그 수정)
- [ ] Telegram 메시지 분할 시 UTF-8 문자가 손상되지 않음
- [ ] Telegram HTTP 요청에 60초 타임아웃 적용
- [ ] Gateway가 `seed_id`, `space_tag`를 OutgoingMessage에 포함

### Phase 1 (통일 포맷)
- [ ] 모든 채널이 `ResponseMeta`를 수신 (phase, evaluation, duration, error)
- [ ] CLI에서 에러 발생 시 아이콘 + 메시지 + 복구 제안 표시
- [ ] Telegram에서 phase + evaluation이 메시지 하단에 표시
- [ ] 에러 분류가 동작 (Timeout, ProviderError 등 구분)

### Phase 2 (CLI + Web)
- [ ] CLI에서 처리 중일 때 새 입력이 reject됨 ("⏳ 이전 요청 처리 중...")
- [ ] CLI 프롬프트가 처리 상태를 표시 ("⏳ oxios>")
- [ ] CLI에서 응답에 phase, 평가 결과, 소요 시간이 포함됨
- [ ] Web WS에서 `"web-user"` 대신 실제 user_id 사용
- [ ] `cargo test --workspace` 성공

### Phase 3 (Space)
- [ ] CLI `.space <tag>` 명령으로 Space 전환
- [ ] Telegram `/space <tag>` 명령으로 Space 전환
- [ ] 기존 `oxios run --json`에 영향 없음

### Phase 4 (Telegram UX)
- [ ] Telegram에서 메시지 수신 시 타이핑 인디케이터 표시
- [ ] Telegram 마크다운 파싱 실패 시 안전한 fallback

---

## 9. 총 추정 일정

| Phase | 일수 | 누적 |
|-------|------|------|
| Phase 0: 버그 수정 | 0.5일 | 0.5일 |
| Phase 1: 통일 포맷 + 포매터 | 1-2일 | 1.5-2.5일 |
| Phase 2: CLI 순차 입력 + Web | 1-2일 | 2.5-4.5일 |
| Phase 3: Space 지원 | 1-2일 | 3.5-6.5일 |
| Phase 4: Telegram UX | 0.5일 | 4-7일 |

**v1 추정 3.5-5.5일 → v2 추정 4-7일** (버그 수정 + CLI 아키텍처 재설계 + Space API 반영)
