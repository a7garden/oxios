# 멀티턴 채팅 시스템 수정 설계

> **상태:** Draft
> **날짜:** 2026-06-09
> **영역:** oxios-web (backend chat.rs, frontend chat.ts), oxios-gateway (bridge.rs)
> **의존:** RFC-015 (chat transparency), Chat UI Redesign (2026-06-07)

---

## 0. 배경

Web UI 채팅에서 멀티턴 대화 점검 결과 4가지 이슈 발견.
본 설계는 4가지 모두에 대한 수정안을 다룸.

| ID | 심각도 | 요약 |
|----|--------|------|
| P0 | Critical | WS 핸들러 메타데이터 키 불일치 (`project_id` vs `project_ids`) |
| P1 | High | 브로드캐스트 채널 멀티탭/멀티세션 충돌 |
| P2 | Medium | Trajectory(실행 추적)가 마지막 응답에만 부착됨 |
| P3 | Low | WS 재연결 시 전역 변수 race condition |

---

## 1. P0: 메타데이터 키 불일치

### 1.1 문제

프론트엔드 → 백엔드 WS 메시지 흐름:

```
chat.ts sendMessage()
  → WS JSON: { project_ids: activeProjectId }
    → chat.rs WS send_task: incoming.metadata.insert("project_id", ...)  ← BUG
      → gateway.rs dispatch(): msg.metadata.get("project_ids")  ← meta.rs 상수
        → orchestrator: project_ids = None  ← 프로젝트 컨텍스트 유실!
```

POST `/api/chat` 핸들러는 올바른 키 `"project_ids"`를 사용하지만,
WS 핸들러는 `"project_id"`를 사용하여 Gateway가 읽지 못함.

### 1.2 수정

`surface/oxios-web/src/routes/chat.rs` WS send_task의 2곳에서 `"project_id"` → `"project_ids"`.

```rust
// Before (BUG)
incoming.metadata.insert("project_id".into(), vid.clone());

// After (FIX)
incoming.metadata.insert("project_ids".into(), vid.clone());
```

영향 파일: `surface/oxios-web/src/routes/chat.rs` (2줄)

---

## 2. P1: 브로드캐스트 채널 멀티탭 충돌

### 2.1 문제

```
WebBridge.outgoing_tx (broadcast::Sender)
  → 탭 A WS (subscribe) → 모든 OutgoingMessage 수신
  → 탭 B WS (subscribe) → 모든 OutgoingMessage 수신
```

모든 WS 연결이 같은 브로드캐스트 채널을 구독.
`active_session_id` 필터링이 있지만:
- 초기 연결 시 `active_session_id = None` → 모든 메시지 통과
- 동일 세션을 여러 탭에서 보면 중복 수신
- `pending_user_msg`가 전역 상태 → 다른 탭의 응답과 매칭될 수 있음

### 2.2 설계: conn_id 기반 필터링

각 WS 연결에 고유 `conn_id`를 부여하고, `OutgoingMessage`에 `target_conn_id` 필드를 추가.
`chat.rs`의 `recv_task`가 자신의 `conn_id`와 일치하는 메시지만 처리.

#### 2.2.1 OutgoingMessage 확장

```rust
// oxios-gateway/src/message.rs

pub struct OutgoingMessage {
    pub id: uuid::Uuid,
    // ... 기존 필드 ...
    /// Target connection ID. When set, only the WS connection with
    /// matching conn_id should process this message. None = broadcast to all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_conn_id: Option<String>,
}
```

#### 2.2.2 WS 연결에 conn_id 할당

```rust
// surface/oxios-web/src/routes/chat.rs: handle_chat_websocket()

pub(crate) async fn handle_chat_websocket(socket: WebSocket, state: Arc<AppState>) {
    let conn_id = uuid::Uuid::new_v4().to_string();
    let (mut ws_tx, mut ws_rx) = socket.split();
    let mut outgoing_rx = state.bridge.subscribe();
    let mut kernel_event_rx = state.kernel.infra.subscribe();
    // ...
}
```

#### 2.2.3 send_task에서 conn_id를 IncomingMessage에 첨부

```rust
// send_task에서 메시지 전송 시
let mut incoming = IncomingMessage::new("web", "default", content.clone());
// ... session_id, project_ids 설정 ...
incoming.metadata.insert("conn_id".into(), conn_id.clone());
```

#### 2.2.4 Gateway에서 응답에 conn_id 복사

```rust
// oxios-gateway/src/gateway.rs: dispatch()

let conn_id = msg.metadata.get("conn_id").cloned();
// ... orchestration 완료 후 ...
let mut outgoing = OutgoingMessage::success(...);
outgoing.target_conn_id = conn_id;  // ← NEW
```

#### 2.2.5 recv_task에서 conn_id 필터링

```rust
// recv_task: OutgoingMessage 수신 시
msg_result = outgoing_rx.recv() => {
    let Ok(msg) = msg_result else { break };
    // 자신의 conn_id와 일치하거나 broadcast(None)인 메시지만 처리
    if msg.target_conn_id.as_ref().is_some_and(|id| id != &conn_id) {
        continue;  // 다른 연결의 메시지 — 무시
    }
    // ... 기존 처리 로직 ...
}
```

#### 2.2.6 pending_user_msg를 conn_id 범위로 제한

기존 `pending_user_msg`는 `Arc<Mutex<Option<...>>>`로 WS 연결당 하나씩 생성됨.
conn_id 도입으로 각 연결이 독립적인 pending 상태를 가지므로,
기존 구조 그대로 사용 가능. 변경 불필요.

### 2.3 변경 파일

| 파일 | 변경 |
|------|------|
| `oxios-gateway/src/message.rs` | `OutgoingMessage`에 `target_conn_id: Option<String>` 추가 |
| `oxios-gateway/src/gateway.rs` | `dispatch()`에서 `conn_id` 복사 |
| `surface/oxios-web/src/routes/chat.rs` | conn_id 생성, 필터링 로직 추가 |

---

## 3. P2: Trajectory per-turn 매핑

### 3.1 문제

현재 `loadSession`에서 전체 `trajectory_steps`를 마지막 assistant 메시지에만 부착.
5턴 대화에서 2번째 턴의 tool call이 마지막 턴에 표시됨.

### 3.2 설계: AgentResponse에 trajectory 인덱스 범위 저장

세션 저장 시 각 `AgentResponse`가 자신에게 해당하는 trajectory_steps의
시작/끝 인덱스를 저장. 복원 시 정확히 해당 범위만 부착.

#### 3.2.1 AgentResponse 확장

```rust
// oxios-kernel/src/state_store.rs

pub struct AgentResponse {
    pub content: String,
    pub session_id: Option<String>,
    pub seed_id: Option<String>,
    pub phase_reached: Option<String>,
    pub evaluation_passed: Option<bool>,
    pub timestamp: DateTime<Utc>,
    /// Index range into Session.trajectory_steps for this response.
    /// None when no tool calls were made during this response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trajectory_range: Option<TrajectoryRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryRange {
    /// Start index (inclusive) into Session.trajectory_steps.
    pub start: usize,
    /// End index (exclusive) into Session.trajectory_steps.
    pub end: usize,
}
```

#### 3.2.2 세션 저장 시 trajectory_range 설정

`chat.rs`의 `persist_session()`에서 trajectory_steps를 추가하기 전에
기존 길이를 기준으로 범위를 계산:

```rust
// persist_session() 내

let existing_len = session.trajectory_steps.len();
session.extend_trajectory(trajectory_steps);
let new_len = session.trajectory_steps.len();

let traj_range = if new_len > existing_len {
    Some(TrajectoryRange { start: existing_len, end: new_len })
} else {
    None
};

session.add_agent_response(AgentResponse {
    content: agent_content.to_string(),
    // ... 기존 필드 ...
    trajectory_range: traj_range,
});
```

POST `/api/chat` 핸들러의 `handle_chat()`에도 동일하게 적용.

#### 3.2.3 프론트엔드 loadSession에서 per-turn 매핑

```typescript
// stores/chat.ts: loadSession()

// 각 agent_response에 해당하는 trajectory만 추출
if (agentMsg) {
    const range = agentResponse.trajectory_range;
    let activities: ChatActivity[] | undefined;
    if (range && trajectoryActivities.length > 0) {
        activities = trajectoryActivities.slice(range.start, range.end);
    }
    messages.push({
        id: crypto.randomUUID(),
        role: 'assistant',
        content: agentMsg.content ?? '',
        timestamp: agentMsg.timestamp ?? data.updated_at,
        ...(activities && activities.length > 0 ? { activities } : null),
    });
}
```

#### 3.2.4 API 응답에 trajectory_range 포함

`GET /api/sessions/:id` 응답의 `agent_responses` 배열에 각 `trajectory_range`가
포함되도록 `Session`의 `Serialize` 구현이 이미 필드를 포함.
백엔드 변경 없이 자동 직렬화됨.

#### 3.2.5 기존 세션 마이그레이션

기존 세션은 `trajectory_range: None` → 프론트엔드에서 fallback:
- `trajectory_range`가 없으면 기존 동작 유지 (전체를 마지막에 부착)
- 또는 아예 trajectory를 표시하지 않음 (새 세션만 정확 표시)

### 3.3 변경 파일

| 파일 | 변경 |
|------|------|
| `oxios-kernel/src/state_store.rs` | `AgentResponse`에 `trajectory_range` 필드 추가 |
| `surface/oxios-web/src/routes/chat.rs` | `persist_session()` + `handle_chat()`에서 range 계산 |
| `surface/oxios-web/web/src/stores/chat.ts` | `loadSession()`에서 per-turn 매핑 |

---

## 4. P3: WS 재연결 상태 관리

### 4.1 문제

```typescript
// 전역 변수 — store 외부에서 관리
let wsInstance: WebSocket | null = null
let chunkHandler: ((chunk: StreamChunk) => void) | null = null
```

- `wsInstance`와 `chunkHandler`가 store 외부 전역 변수
- `connect()`가 여러 번 호출되면 이전 WS 연결이 정리되지 않을 수 있음
- `chunkHandler`가 마지막 `connect()` 호출에서만 설정됨 → 이전 연결의 onmessage가 stale handler를 참조

### 4.2 설계: WS 수명을 store 내부로 캡슐화

전역 변수를 제거하고, 모든 WS 상태를 store 내부 클로저에서 관리.
`useRef` 패턴 대신 store 함수 자체에서 수명을 관리.

#### 4.2.1 전역 변수 제거

```typescript
// 제거
- let wsInstance: WebSocket | null = null
- let chunkHandler: ((chunk: StreamChunk) => void) | null = null

// store 인터페이스에 추가
interface ChatRuntimeState {
    // ... 기존 ...
    _ws: WebSocket | null  // store 내부에서 관리
    _reconnectTimer: ReturnType<typeof setTimeout> | null
}
```

#### 4.2.2 connect() 재설계

```typescript
async connect() {
    const { _ws } = get()

    // 이미 연결되어 있으면 무시
    if (_ws && _ws.readyState === WebSocket.OPEN) return
    if (typeof window === 'undefined') return

    // 이전 연결 정리
    if (_ws) {
        _ws.onopen = null
        _ws.onmessage = null
        _ws.onclose = null
        _ws.onerror = null
        if (_ws.readyState === WebSocket.OPEN || _ws.readyState === WebSocket.CONNECTING) {
            _ws.close()
        }
    }

    // 이전 재연결 타이머 정리
    const prevTimer = get()._reconnectTimer
    if (prevTimer) clearTimeout(prevTimer)

    const url = await buildWsUrl()
    const ws = new WebSocket(url)

    // store에 WS 참조 저장
    set({ _ws: ws, connected: false, isStreaming: false })

    ws.onopen = () => {
        // connect()가 다시 호출되어 ws가 교체되었는지 확인
        if (get()._ws !== ws) return  // stale connection — 무시
        set({ connected: true })
        // Flush queue
        const queue = get()._sendQueue
        if (queue.length > 0) {
            set({ _sendQueue: [] })
            for (const msg of queue) {
                get().sendMessage(msg)
            }
        }
    }

    ws.onmessage = (event) => {
        // stale connection 체크
        if (get()._ws !== ws) return
        try {
            const raw = JSON.parse(event.data as string)
            const chunk = parseChunk(raw)
            get().handleChunk(chunk)
        } catch { /* ignore */ }
    }

    ws.onclose = () => {
        if (get()._ws !== ws) return  // 이미 새 연결로 교체됨
        set({ connected: false, isStreaming: false, _ws: null })
        // 자동 재연결 (exponential backoff)
        // ...
    }

    ws.onerror = () => {
        if (get()._ws !== ws) return
        ws.close()
    }
}
```

#### 4.2.3 disconnect() 재설계

```typescript
disconnect() {
    const { _ws, _reconnectTimer } = get()
    if (_reconnectTimer) clearTimeout(_reconnectTimer)
    if (_ws) {
        // 핸들러 해제 후 close
        _ws.onopen = null
        _ws.onmessage = null
        _ws.onclose = null
        _ws.onerror = null
        _ws.close()
    }
    set({ connected: false, isStreaming: false, _ws: null, _reconnectTimer: null })
}
```

#### 4.2.4 sendMessage() 재설계

```typescript
sendMessage(content: string) {
    const { activeSessionId, activeProjectId, connected, connect, _ws } = get()

    if (!connected || !_ws || _ws.readyState !== WebSocket.OPEN) {
        connect()
        const q = get()._sendQueue
        if (!q.includes(content)) {
            set({ _sendQueue: [...q, content] })
        }
        return
    }

    // Optimistic user message
    const userMsg: ChatMessage = { ... }
    set((s) => ({ messages: [...s.messages, userMsg], isStreaming: true }))

    _ws.send(JSON.stringify({
        type: 'message',
        content,
        session_id: activeSessionId ?? '',
        project_ids: activeProjectId ?? '',
    }))
}
```

#### 4.2.5 submitInterviewResponse() 재설계

```typescript
submitInterviewResponse(answers: InterviewAnswer[]) {
    const { _ws, activeInterview, activeSessionId, activeProjectId, interviewRound } = get()
    if (!activeInterview) return
    // ...
    if (_ws && _ws.readyState === WebSocket.OPEN) {
        _ws.send(JSON.stringify({ ... }))
    }
    // ...
}
```

#### 4.2.6 자동 재연결 (exponential backoff)

```typescript
// connect()의 ws.onclose 핸들러 내
ws.onclose = () => {
    if (get()._ws !== ws) return
    set({ connected: false, isStreaming: false, _ws: null })

    // Exponential backoff 재연결 (최대 5회)
    const attempt = get()._reconnectAttempts ?? 0
    if (attempt >= 5) return
    const delay = 1000 * Math.pow(2, attempt)
    const timer = setTimeout(() => {
        if (get()._ws === null) {  // 아직 새 연결이 없으면
            set({ _reconnectAttempts: attempt + 1 })
            get().connect()
        }
    }, delay)
    set({ _reconnectTimer: timer })
}
```

### 4.3 store 인터페이스에 추가할 내부 상태

```typescript
interface ChatRuntimeState {
    // ... 기존 ...
    /** WebSocket instance (managed internally). */
    _ws: WebSocket | null
    /** Reconnect timer. */
    _reconnectTimer: ReturnType<typeof setTimeout> | null
    /** Reconnect attempt counter. */
    _reconnectAttempts: number
}
```

`_ws`는 `partialize`에서 제외 (런타임 상태이므로 persist 불필요).

### 4.4 변경 파일

| 파일 | 변경 |
|------|------|
| `surface/oxios-web/web/src/stores/chat.ts` | 전역 변수 제거, store 내부 `_ws` 관리, stale connection 체크, 자동 재연결 |

---

## 5. 변경 파일 전체 목록

| 파일 | P0 | P1 | P2 | P3 | 변경 내용 |
|------|----|----|----|----|----|
| `surface/oxios-web/src/routes/chat.rs` | ✅ | ✅ | ✅ | | `"project_ids"` 수정, conn_id 생성/필터링, trajectory_range 계산 |
| `oxios-gateway/src/message.rs` | | ✅ | | | `OutgoingMessage.target_conn_id` 추가 |
| `oxios-gateway/src/gateway.rs` | | ✅ | | | `dispatch()`에서 conn_id 복사 |
| `oxios-kernel/src/state_store.rs` | | | ✅ | | `AgentResponse.trajectory_range` 추가 |
| `surface/oxios-web/web/src/stores/chat.ts` | | | ✅ | ✅ | 전역 변수 제거, per-turn trajectory, `_ws` 캡슐화 |

---

## 6. 구현 순서

의존성 그래프에 따라 순서대로 진행:

```
P0 (독립) ────────────────────────────────→ 바로 수정
P1 (gateway message.rs → gateway.rs → chat.rs) → 위에서부터 아래로
P2 (state_store.rs → chat.rs → chat.ts)       → 백엔드 먼저
P3 (chat.ts만)                                  → 마지막
```

1. **P0**: chat.rs 메타데이터 키 수정 (2줄, 즉시)
2. **P2 백엔드**: state_store.rs에 `TrajectoryRange` 추가
3. **P2 백엔드**: chat.rs에 trajectory_range 계산 로직 추가
4. **P1 백엔드**: message.rs에 `target_conn_id` 추가
5. **P1 백엔드**: gateway.rs에 conn_id 복사 로직
6. **P1 백엔드**: chat.rs에 conn_id 생성 + 필터링
7. **P2 프론트엔드**: chat.ts loadSession per-turn 매핑
8. **P3 프론트엔드**: chat.ts 전역 변수 제거, `_ws` 캡슐화

---

## 7. 테스트 계획

| 테스트 | 대상 | 방법 |
|--------|------|------|
| P0: project_ids 전달 | chat.rs WS 경로 | WS로 메시지 전송 후 orchestrator가 받은 project_ids 확인 |
| P1: conn_id 필터링 | chat.rs + gateway | 2개 WS 연결에서 각각 다른 메시지 전송, 응답 분리 확인 |
| P2: trajectory per-turn | state_store + chat.ts | 3턴 대화 후 loadSession, 각 턴에 올바른 tool calls 부착 확인 |
| P3: 재연결 | chat.ts | connect() 중복 호출, disconnect + 재연결, stale onclose 무시 |

---

## 8. 하위 호환성

- **`target_conn_id: None`** → 기존 동작 유지 (broadcast). 기존 클라이언트도 동작.
- **`trajectory_range: None`** → 기존 세션은 range 없음. 프론트엔드 fallback 유지.
- **`_ws` 캡슐화** → 프론트엔드 전용 변경. 백엔드 영향 없음.
- **`project_ids` 키 수정** → 기존에 동작하지 않던 기능이 동작하게 됨. breaking change 아님.

---

## 9. 리스크

| 리스크 | 완화 |
|--------|------|
| broadcast 채널에 conn_id가 없는 메시지 (다른 채널에서 온 응답) | `target_conn_id: None`은 통과시킴 (broadcast semantics 유지) |
| trajectory_range 계산 시 기존 trajectory_steps 길이가 정확하지 않음 | `extend_trajectory` 호출 직전에 len() 캡처 |
| `_ws`를 persist하려는 시도 | `partialize`에서 `_ws`, `_reconnectTimer`, `_reconnectAttempts` 제외 |
| WS onclose 핸들러가 reconnect 중 connect()를 재귀 호출 | max 5회 제한 + exponential backoff |
