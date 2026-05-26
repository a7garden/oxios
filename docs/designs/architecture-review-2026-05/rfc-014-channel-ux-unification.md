# RFC-014: 채널 UX 통일

> **상태:** 📝 설계
> **날짜:** 2026-05-26
> **우선순위:** P0-P1
> **범위:** `channels/oxios-web/`, `channels/oxios-cli/`, `channels/oxios-telegram/`
> **선행:** RFC-013 (Gateway Event-Driven)
> **후행:** 없음

---

## 1. 동기

현재 Web-CLI-Telegram 간 사용자 경험 품질 격차가 크다:

| 측면 | Web | CLI | Telegram |
|------|-----|-----|----------|
| 응답 상관관계 | ✅ oneshot | ❌ fire-and-forget | ❌ |
| 에러 표시 | 정돈된 JSON | raw 내부 문자열 | fallback plain text |
| Phase/평가 노출 | ✅ | ❌ 버림 | ❌ 버림 |
| Space 컨텍스트 | ✅ | ❌ | ❌ |
| 세션 영속성 | ✅ StateStore | ❌ 메모리 | ❌ 메모리 |
| 스트리밍 | ✅ 토큰 단위 | ❌ | ❌ |

**원칙:** 사용자는 어떤 채널을 쓰든 동일한 품질의 응답과 에러 피드백을 받아야 한다.

---

## 2. 설계

### 2.1 통일 응답 포맷

모든 채널이 동일한 구조화된 응답을 소비:

```rust
/// 채널 독립적 통일 응답 (gateway → channel)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelResponse {
    /// 에이전트 응답 본문
    pub content: String,
    /// 실행 결과 메타데이터
    pub meta: ResponseMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMeta {
    pub session_id: Option<String>,
    pub space_id: Option<String>,
    pub space_tag: Option<String>,
    pub seed_id: Option<String>,
    pub phase_reached: String,       // "Interview" | "Seed" | "Execute" | "Evaluate"
    pub evaluation_passed: Option<bool>,
    pub duration_ms: Option<u64>,
    /// 에러인 경우 구조화된 에러 정보
    pub error: Option<ChannelError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelError {
    /// 사용자에게 보여줄 메시지 (채널 언어에 맞게)
    pub user_message: String,
    /// 에러 분류
    pub kind: ErrorKind,
    /// 복구 제안
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorKind {
    /// 에이전트 실행 실패
    ExecutionFailed,
    /// LLM 프로바이더 오류
    ProviderError,
    /// 시간 초과
    Timeout,
    /// 권한 부족
    PermissionDenied,
    /// 입력 검증 실패
    ValidationError,
    /// 시스템 내부 오류 (사용자에게 내역 노출 안함)
    InternalError,
}
```

### 2.2 채널 포매터 트레이트

각 채널이 자신의 출력 형식에 맞게 포맷:

```rust
/// 채널별 응답 포매팅 트레이트
pub trait ChannelFormatter: Send + Sync {
    /// 성공 응답 포맷
    fn format_success(&self, response: &ChannelResponse) -> String;
    /// 에러 응답 포맷
    fn format_error(&self, error: &ChannelError) -> String;
    /// 진행 상태 포맷 (스트리밍 불가 채널용)
    fn format_progress(&self, phase: &str) -> String;
}

// --- 구현체 ---

/// Web: JSON 그대로 전달 (프론트엔드가 렌더링)
pub struct WebFormatter;

/// CLI: 터미널 친화적 ANSI 포맷
pub struct CliFormatter;

/// Telegram: 마크다운 + emoji
pub struct TelegramFormatter;
```

**각 포매터 예시:**

```rust
impl ChannelFormatter for CliFormatter {
    fn format_success(&self, resp: &ChannelResponse) -> String {
        let mut out = resp.content.clone();
        if let Some(meta) = resp.meta.evaluation_passed {
            let icon = if meta { "✅" } else { "⚠️" };
            out.push_str(&format!("\n\n{} 평가: {}", icon, if meta { "통과" } else { "미통과" }));
        }
        if let Some(dur) = resp.meta.duration_ms {
            out.push_str(&format!(" ({}ms)", dur));
        }
        out
    }

    fn format_error(&self, err: &ChannelError) -> String {
        let kind_icon = match err.kind {
            ErrorKind::ExecutionFailed => "❌",
            ErrorKind::ProviderError => "🔌",
            ErrorKind::Timeout => "⏱️",
            ErrorKind::PermissionDenied => "🔒",
            ErrorKind::ValidationError => "⚠️",
            ErrorKind::InternalError => "💥",
        };
        let mut out = format!("{} {}", kind_icon, err.user_message);
        if let Some(s) = &err.suggestion {
            out.push_str(&format!("\n💡 {}", s));
        }
        out
    }

    fn format_progress(&self, phase: &str) -> String {
        match phase {
            "Interview" => "🔍 분석 중...".to_string(),
            "Seed" => "📋 계획 수립 중...".to_string(),
            "Execute" => "⚡ 실행 중...".to_string(),
            "Evaluate" => "📊 평가 중...".to_string(),
            _ => "⏳ 처리 중...".to_string(),
        }
    }
}
```

### 2.3 CLI 응답 상관관계

현재 CLI는 fire-and-forget이다. oneshot 채널로 요청-응답 상관관계 추가:

```rust
/// CLIChannel 개선안
pub struct CliChannel {
    formatter: CliFormatter,
    /// 요청별 oneshot 채널 보관
    pending: Mutex<HashMap<String, oneshot::Sender<ChannelResponse>>>,
}

impl Channel for CliChannel {
    async fn start(&self, tx: Sender, mut shutdown: watch::Receiver<bool>) -> Result<()> {
        let tx = tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    line = read_line_async() => {
                        if let Some(input) = line {
                            let id = Uuid::new_v4().to_string();
                            let msg = IncomingMessage::with_id(&id, "cli", &input);
                            let _ = tx.send(("cli".into(), msg)).await;
                            // 진행 상태 표시
                            print!("{}", self.formatter.format_progress("Interview"));
                        }
                    }
                    _ = shutdown.changed() => break,
                }
            }
        });
        Ok(())
    }

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        // 상관관계 ID로 pending에서 oneshot 찾기
        if let Some(tx) = self.pending.lock().unwrap().remove(&msg.correlation_id) {
            let response: ChannelResponse = serde_json::from_str(&msg.content)
                .unwrap_or_else(|_| ChannelResponse::raw(&msg.content));
            let _ = tx.send(response);
        }
        Ok(())
    }
}
```

**대안 (더 간단):** Gateway가 `send_and_wait` 패턴을 CLI에도 적용:

```rust
// cli interactive loop
async fn handle_input(&self, input: String) {
    let response = self.channel.send_and_wait(input, Duration::from_secs(300)).await;
    match response {
        Ok(resp) => {
            // 진행 표시 지우고
            clear_progress();
            // 결과 출력
            print!("{}", self.formatter.format_success(&resp));
        }
        Err(e) => {
            clear_progress();
            print!("{}", self.formatter.format_error(&e));
        }
    }
    self.rl.readline("oxios> "); // 응답 후에만 새 프롬프트
}
```

### 2.4 메타데이터 상수화

문자열 키의 오타를 방지:

```rust
/// 메타데이터 키 상수
pub mod meta_keys {
    pub const SESSION_ID: &str = "session_id";
    pub const SPACE_ID: &str = "space_id";
    pub const CHAT_ID: &str = "chat_id";
    pub const MESSAGE_ID: &str = "message_id";
    pub const PHASE: &str = "phase";
    pub const EVALUATION_PASSED: &str = "evaluation_passed";
    pub const USER_ID: &str = "user_id";
    pub const CORRELATION_ID: &str = "correlation_id";
}

/// 타입-안전 메타데이터 접근
impl IncomingMessage {
    pub fn session_id(&self) -> Option<&str> { self.meta.get(meta_keys::SESSION_ID).map(|s| s.as_str()) }
    pub fn space_id(&self) -> Option<&str> { self.meta.get(meta_keys::SPACE_ID).map(|s| s.as_str()) }
}
```

### 2.5 CLI/Telegram에 Space 지원

```rust
// CLI 메타 명령 확장
// .space <tag>     — 현재 space 전환
// .spaces          — space 목록
// .space           — 현재 space 확인

// Telegram 명령 확장
// /space <tag>     — space 전환
// /spaces          — space 목록
```

CLI `IncomingMessage`에 현재 space_id 포함:

```rust
// cli channel start()
let msg = IncomingMessage::new("cli", &input)
    .with_meta(meta_keys::SESSION_ID, &self.session.id)
    .with_meta(meta_keys::SPACE_ID, &self.current_space_id()); // 추가
```

---

## 3. 마이그레이션 계획

### Phase 1: 통일 포맷 + 포매터 (1-2일)

| 작업 | 파일 | 설명 |
|------|------|------|
| `ChannelResponse`/`ChannelError` 정의 | `gateway/src/message.rs` | 통일 응답 구조체 |
| `ChannelFormatter` trait + 3개 구현 | `gateway/src/format.rs` (신규) | Web/Cli/Telegram 포매터 |
| Orchestrator가 `ChannelResponse` 생성 | `kernel/src/orchestrator.rs` | 응답 래핑 |
| 메타데이터 상수 | `gateway/src/meta.rs` (신규) | `meta_keys` 모듈 |

### Phase 2: CLI 개선 (1-2일)

| 작업 | 설명 |
|------|------|
| 응답 상관관계 추가 | oneshot channel or send_and_wait |
| 포매터 적용 | 에러/성공 통일 포맷 |
| 진행 상태 표시 | phase별 spinner |
| Space 지원 | `.space` 명령 + 메타데이터 |

### Phase 3: Telegram 개선 (1일)

| 작업 | 설명 |
|------|------|
| 포매터 적용 | 에러 emoji + 마크다운 |
| Phase 노출 | 실행 단계 메시지 |
| Space 지원 | `/space` 명령 |

### Phase 4: Web 정리 (0.5일)

| 작업 | 설명 |
|------|------|
| 중복 세션 영속화 코드 제거 | HTTP/WS 공통 함수 추출 |
| user_id 통일 | WS에서 `"web-user"` 대신 실제 user_id |

---

## 4. 영향 범위

| 컴포넌트 | 변경 |
|----------|------|
| `oxios-gateway` | `message.rs` 확장, `format.rs` 신규 |
| `oxios-cli` | `channel.rs`, `interactive.rs` 대폭 수정 |
| `oxios-telegram` | `lib.rs` 소폭 수정 (포매터 적용) |
| `oxios-web` (Rust) | 중복 코드 제거, user_id 통일 |
| `oxios-web` (Frontend) | 변경 없음 (이미 JSON 응답 소비) |
| `oxios-kernel` | `orchestrator.rs` 응답 래핑만 |

---

## 5. 위험 및 완화

| 위험 | 완화 |
|------|------|
| CLI 응답 대기 중 블로킹 | 5분 타임아웃 + ctrl+c로 취소 |
| Telegram 마크다운 파싱 실패 | fallback plain text 유지 |
| 기존 Web API 응답 형식 변경 | `ChannelResponse`를 OutgoingMessage.content에 JSON으로 직렬화 — 기존 Web 파서 호환 |

---

## 6. 성공 기준

- [ ] CLI에서 에러 발생 시 구조화된 메시지 + 복구 제안 표시
- [ ] CLI에서 프롬프트가 응답 도착 후에만 재표시
- [ ] 모든 채널에서 `phase_reached` + `evaluation_passed` 사용자 노출
- [ ] CLI/Telegram에서 Space 전환 가능
- [ ] 메타데이터 키에 상수 사용 (문자열 리터럴 제거)
- [ ] Web 중복 세션 영속화 코드 제거
