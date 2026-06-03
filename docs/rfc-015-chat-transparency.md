# RFC-015: Chat Transparency — 실시간 에이전트 추론 과정 시각화

> **상태:** Draft  
> **날짜:** 2026-06-03  
> **영역:** oxios-kernel (protocol), oxios-gateway (routing), oxios-web (frontend)  
> **의존:** RFC-014 (typed response meta), oxi-sdk AgentEvent streaming

---

## 1. 동기

현재 Web UI 채팅은:
- `token` → 텍스트 누적, `done` → 최종 메타 한방
- 사이에 아무것도 안 보임 → "로딩 중..." 스피너만
- 에이전트가 무엇을 하고 있는지 (도구 호출, 메모리 회상, 페이즈 전환) 불투명

**목표:** Gemini / Claude Code / ChatGPT Canvas 수준의 **실시간 투명한 실행 추적**을 Web UI에 제공.

**비목표:**
- Telegram/CLI 채널 변경 없음 (Web 전용)
- agent tool 호출 패턴 자체 변경 없음 (UI 레이어만)

---

## 2. 설계 개요

### 2.1 아키텍처

```
AgentRuntime (oxi-sdk AgentEvent)
  │  ToolExecutionStart/End, Usage, Compaction, ...
  ▼
Orchestrator (phase: Interview → Seed → Execute → Evaluate)
  │  PhaseStarted/Completed, MemoryRecalled, SeedCreated
  ▼
WebChannel → WS chunks (NEW types: phase, tool_start, tool_end, memory, reasoning)
  │
  ▼
Frontend chat store → ActivityTimeline 컴포넌트
   └─ 접힌 카드들 (클릭하면 펼침)
```

### 2.2 핵심 원칙

1. **WS 프로토콜 확장** — 새 chunk type 6종 추가
2. **세션 영구 저장** — trajectory_steps를 session에 JSON으로 저장
3. **Frontend 접힘 카드** — 기본 헤더만, 클릭으로 펼침
4. **순서 보장** — chunk 순서 = 화면 순서 (WS는 TCP 기반)
5. **마크다운 렌더링** — 모든 텍스트에 react-markdown 적용

---

## 3. Backend 변경

### 3.1 WS Chunk Protocol 확장

기존: `{ type: "token" }` | `{ type: "done" }` | `{ type: "error" }`  
추가: `{ type: "phase" }` | `{ type: "tool_start" }` | `{ type: "tool_end" }` | `{ type: "memory" }` | `{ type: "reasoning" }` | `{ type: "usage" }`

#### 3.1.1 `phase` — Ouroboros 페이즈 전환

```json
{
  "type": "phase",
  "phase": "execute",
  "session_id": "abc...",
  "status": "started",       // "started" | "completed"
  "summary": "Agent 실행 중" // 선택
}
```

발행 시점:
- `Orchestrator::publish_phase_started()` / `publish_phase_completed()`
- Phase: Interview, Seed, Execute, Evaluate, Evolve

#### 3.1.2 `tool_start` — 도구 호출 시작

```json
{
  "type": "tool_start",
  "tool_name": "read_file",
  "tool_call_id": "call_abc123",
  "tool_args": { "path": "/src/main.rs" }
}
```

발행 시점: `AgentEvent::ToolExecutionStart` 콜백

#### 3.1.3 `tool_end` — 도구 호출 완료

```json
{
  "type": "tool_end",
  "tool_call_id": "call_abc123",
  "tool_name": "read_file",
  "duration_ms": 234,
  "is_error": false,
  "output_summary": "fn main() { ... }"   // 최대 500자
}
```

발행 시점: `AgentEvent::ToolExecutionEnd` 콜백

#### 3.1.4 `memory` — 메모리 회상/저장

```json
{
  "type": "memory",
  "action": "recall",    // "recall" | "store"
  "query": "Rust 에러 핸들링 패턴",
  "count": 3,
  "source": "warm"       // "hot" | "warm" | "cold"
}
```

발행 시점: `MemoryManager::recall_with_proactive()` 응답 후

#### 3.1.5 `reasoning` — 추론/생각 과정

```json
{
  "type": "reasoning",
  "content": "이 요청은 파일 읽기와 코드 수정이 필요하다...",
  "source": "chain_of_thought"  // "chain_of_thought" | "compaction"
}
```

발행 시점:
- `AgentEvent::Compaction` (컨텍스트 압축)
- 모델의 extended thinking output (provider 지원 시)

#### 3.1.6 `usage` — 토큰 사용량

```json
{
  "type": "usage",
  "input_tokens": 1234,
  "output_tokens": 567
}
```

발행 시점: `AgentEvent::Usage` 콜백

### 3.2 Kernel-side 변경: AgentEvent → WS Chunk 브릿지

핵심: `agent_runtime.rs`의 `run_streaming` 콜백에서 WS chunk를 직접 보내는 게 아니라, **EventBus에 새 이벤트를 publish**하고 WebChannel이 이를 WS chunk로 변환.

#### 3.2.1 새 KernelEvent 변종

```rust
// event_bus.rs 에 추가
pub enum KernelEvent {
    // ... 기존 ...
    
    /// Tool 실행 시작 (실시간)
    ToolExecutionStarted {
        session_id: String,
        tool_name: String,
        tool_call_id: String,
        tool_args: serde_json::Value,
    },
    /// Tool 실행 완료 (실시간)
    ToolExecutionFinished {
        session_id: String,
        tool_call_id: String,
        tool_name: String,
        duration_ms: u64,
        is_error: bool,
        output_summary: String,
    },
    /// 메모리 회상 발생
    MemoryRecallUsed {
        session_id: String,
        query: String,
        count: usize,
        source: String,
    },
    /// 토큰 사용량 업데이트
    TokenUsageUpdate {
        session_id: String,
        input_tokens: u64,
        output_tokens: u64,
    },
    /// 추론 과정 조각
    ReasoningFragment {
        session_id: String,
        content: String,
        source: String,
    },
}
```

#### 3.2.2 Orchestrator에서 session_id 전달

현재 `agent_runtime::execute()`는 `SessionContext`를 받지만 event bus publish에 session_id를 직접 사용하지 않음.

해결: `AgentRuntimeConfig`에 `session_id: Option<String>` 필드 추가. Orchestrator가 spawn_and_run 시 전달.

```rust
// orchestrator.rs handle_message()에서
let session_id_clone = session_id.clone();
self.lifecycle.spawn_and_run_with_session(&seed, Priority::Normal, &session_id_clone).await?;
```

### 3.3 Gateway/WebChannel: KernelEvent → WS Chunk

`handle_chat_websocket()`의 `recv_task`가 `state.channel.subscribe()` 대신 **kernel event bus도 구독**.

```rust
// chat.rs: handle_chat_websocket()
let mut event_rx = state.kernel.infra.subscribe();
let mut outgoing_rx = state.channel.subscribe();

tokio::select! {
    // 기존: gateway 응답 (token + done)
    msg = outgoing_rx.recv() => { /* 기존 로직 */ }
    // 새로: kernel events → WS chunks
    event = event_rx.recv() => {
        if let Ok(kernel_event) = event {
            if matches_session(&kernel_event, &active_session_id) {
                if let Some(chunk) = kernel_event_to_ws_chunk(&kernel_event) {
                    let json = serde_json::to_string(&chunk).unwrap();
                    ws_tx.send(Message::Text(json.into())).await;
                }
            }
        }
    }
}
```

`kernel_event_to_ws_chunk()` 맵핑:

| KernelEvent | WS chunk type |
|---|---|
| `PhaseStarted { .. }` | `{ type: "phase", status: "started" }` |
| `PhaseCompleted { .. }` | `{ type: "phase", status: "completed" }` |
| `ToolExecutionStarted { .. }` | `{ type: "tool_start" }` |
| `ToolExecutionFinished { .. }` | `{ type: "tool_end" }` |
| `MemoryRecallUsed { .. }` | `{ type: "memory" }` |
| `MemoryStored { .. }` | `{ type: "memory", action: "store" }` |
| `TokenUsageUpdate { .. }` | `{ type: "usage" }` |
| `ReasoningFragment { .. }` | `{ type: "reasoning" }` |
| `SeedCreated { .. }` | `{ type: "phase", phase: "seed_created" }` |
| `EvaluationComplete { .. }` | `{ type: "phase", phase: "evaluation" }` |
| `EvolutionStarted { .. }` | `{ type: "phase", phase: "evolution" }` |
| `Compaction` (from runtime) | `{ type: "reasoning", source: "compaction" }` |

### 3.4 Session 영구 저장에 trajectory 추가

```rust
// state_store.rs: Session 구조체에 추가
pub struct Session {
    // ... 기존 필드 ...
    /// Trajectory steps recorded during agent execution (RFC-015).
    pub trajectory_steps: Vec<TrajectoryStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryStep {
    pub tool_name: String,
    pub tool_args: serde_json::Value,
    pub output_summary: String,
    pub duration_ms: u64,
    pub is_error: bool,
    pub timestamp: DateTime<Utc>,
}
```

저장 시점: `OrchestrationResult` 생성 후, `OutgoingMessage::success()` 호출 전.

복원 시점: `loadSession()` API에서 `trajectory_steps`를 포함하여 반환.

---

## 4. Frontend 변경

### 4.1 타입 확장

```typescript
// types/index.ts
export interface StreamChunk {
  type: 'token' | 'tool_call' | 'tool_result' | 'done' | 'error'
    | 'phase' | 'tool_start' | 'tool_end' | 'memory' | 'reasoning' | 'usage'
  // 기존 필드 ...
  
  // phase
  phase?: string
  status?: 'started' | 'completed'
  summary?: string
  
  // tool_start / tool_end
  tool_call_id?: string
  tool_name?: string
  tool_args?: Record<string, unknown>
  duration_ms?: number
  is_error?: boolean
  output_summary?: string
  
  // memory
  action?: 'recall' | 'store'
  query?: string
  count?: number
  source?: string
  
  // reasoning
  content?: string  // 기존 token과 공유
  
  // usage
  input_tokens?: number
  output_tokens?: number
}

// 새로운 Activity 타입
export type ActivityType = 'phase' | 'tool_call' | 'memory' | 'reasoning' | 'usage'

export interface ChatActivity {
  id: string
  type: ActivityType
  timestamp: string
  // phase
  phase?: string
  status?: 'started' | 'completed'
  summary?: string
  // tool
  toolCallId?: string
  toolName?: string
  toolArgs?: Record<string, unknown>
  outputSummary?: string
  durationMs?: number
  isError?: boolean
  // memory
  action?: 'recall' | 'store'
  query?: string
  count?: number
  memorySource?: string
  // reasoning
  content?: string
  reasoningSource?: string
  // usage
  inputTokens?: number
  outputTokens?: number
}

export interface ChatMessage {
  // ... 기존 ...
  activities?: ChatActivity[]       // NEW: 이 메시지에 대한 실행 추적
  totalInputTokens?: number         // NEW: 누적
  totalOutputTokens?: number        // NEW: 누적
}
```

### 4.2 Chat Store `handleChunk` 확장

```typescript
handleChunk(chunk: StreamChunk) {
  switch (chunk.type) {
    case 'token': { /* 기존: 텍스트 누적 */ }
    
    case 'phase':
    case 'tool_start':
    case 'tool_end':
    case 'memory':
    case 'reasoning':
    case 'usage': {
      set((s) => {
        const updated = [...s.messages]
        const last = updated[updated.length - 1]
        if (last?.role === 'assistant') {
          const activity: ChatActivity = chunkToActivity(chunk)
          return {
            messages: [...updated.slice(0, -1), {
              ...last,
              activities: [...(last.activities ?? []), activity],
              // usage 누적
              totalInputTokens: (last.totalInputTokens ?? 0) + (chunk.input_tokens ?? 0),
              totalOutputTokens: (last.totalOutputTokens ?? 0) + (chunk.output_tokens ?? 0),
            }],
          }
        }
        return s // tool_end 등이 assistant msg 전에 오면 무시
      })
      break
    }
    
    case 'done': { /* 기존 로직 + activities 유지 */ }
    case 'error': { /* 기존 */ }
  }
}
```

### 4.3 새 컴포넌트: `ActivityTimeline`

```
components/chat/
├── activity-timeline.tsx       // 활동 카드 리스트
├── activity-card.tsx           // 개별 카드 (접힘/펼침)
├── phase-indicator.tsx         // 페이즈 진행 표시
├── tool-card.tsx               // 도구 호출 카드
├── memory-card.tsx             // 메모리 회상/저장 카드
├── reasoning-card.tsx          // 추론 과정 카드
└── usage-bar.tsx               // 토큰 사용량 표시
```

#### ActivityTimeline

```tsx
function ActivityTimeline({ activities, streaming }: Props) {
  if (!activities?.length) return null
  
  // phase 카드를 그룹화: started + completed 쌍
  const grouped = groupPhasePairs(activities)
  
  return (
    <div className="space-y-1 my-2 ml-11">
      {grouped.map((activity) => (
        <ActivityCard key={activity.id} activity={activity} />
      ))}
    </div>
  )
}
```

#### ActivityCard (통일된 접힘 카드)

```tsx
function ActivityCard({ activity }: { activity: ChatActivity }) {
  const [expanded, setExpanded] = useState(false)
  
  const { icon, label, badge } = getActivityMeta(activity)
  
  return (
    <div className="rounded-lg border bg-muted/30 text-xs">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 px-3 py-1.5"
      >
        {icon}
        <span className="font-medium truncate">{label}</span>
        {badge}
        {activity.durationMs && (
          <span className="ml-auto text-muted-foreground">
            {formatDuration(activity.durationMs)}
          </span>
        )}
        <ChevronRight className={cn("h-3 w-3 transition-transform", expanded && "rotate-90")} />
      </button>
      {expanded && (
        <div className="border-t px-3 py-2">
          <ActivityDetail activity={activity} />
        </div>
      )}
    </div>
  )
}
```

아이콘 매핑:
| type | icon | label 예시 |
|---|---|---|
| phase | `🔄`/`✅` (status에 따라) | "Interview" / "Execute" |
| tool_call | `🔧` | "read_file" / "bash" |
| memory | `🧠` | "Recalled 3 memories" |
| reasoning | `💭` | "Thinking..." / "Context compressed" |
| usage | `📊` | "1,234 → 567 tokens" |

#### ToolCard (펼침 상세)

```
펼침 시:
┌─────────────────────────────────────┐
│ 🔧 read_file                  234ms │
├─────────────────────────────────────┤
│ Input                               │
│ { "path": "/src/main.rs" }          │
│                                     │
│ Output                              │
│ fn main() {                         │
│     println!("hello");              │
│ }                                   │
└─────────────────────────────────────┘
```

#### PhaseIndicator (진행 트래커)

```
Interview ─── Seed ─── Execute ─── Evaluate
   ●           ●         ◐            ○
 completed  completed  active       pending
```

### 4.4 MessageBubble 수정

기존 MessageBubble에 ActivityTimeline 삽입:

```tsx
function MessageBubble({ message }: Props) {
  // ... 기존 ...
  
  return (
    <div className={...}>
      {/* 아바타 */}
      {/* 메시지 내용 (마크다운 렌더) */}
      
      {/* NEW: 실행 추적 타임라인 */}
      {message.activities && message.activities.length > 0 && (
        <ActivityTimeline activities={message.activities} />
      )}
      
      {/* 기존: ChatMetadata (phase, eval, duration) */}
      <ChatMetadata message={message} />
    </div>
  )
}
```

### 4.5 스트리밍 중 페이즈 표시 (헤더 영역)

스트리밍 중일 때 헤더 영역에 현재 페이즈를 표시:

```tsx
{isStreaming && (
  <div className="flex items-center gap-2 px-4 py-2 bg-muted/50 text-xs text-muted-foreground border-b">
    <Loader2 className="h-3 w-3 animate-spin" />
    <PhaseIndicator activities={currentActivities} />
  </div>
)}
```

### 4.6 Session 복원 시 trajectory 표시

`loadSession()` 시 백엔드에서 `trajectory_steps`를 받아 `ChatActivity[]`로 변환:

```typescript
// loadSession() 내에서
const trajectoryActivities: ChatActivity[] = (data.trajectory_steps ?? []).map(step => ({
  id: crypto.randomUUID(),
  type: 'tool_call',
  timestamp: step.timestamp,
  toolName: step.tool_name,
  toolArgs: step.tool_args,
  outputSummary: step.output_summary,
  durationMs: step.duration_ms,
  isError: step.is_error,
}))
```

---

## 5. 마크다운 렌더링 강화

현재 `react-markdown` + `remark-gfm` 사용 중. 추가:

### 5.1 Syntax Highlighting

```bash
bun add react-syntax-highlighter @types/react-syntax-highlighter
```

```tsx
// message-bubble.tsx
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter'
import { oneDark } from 'react-syntax-highlighter/dist/esm/styles/prism'

<ReactMarkdown
  remarkPlugins={[remarkGfm]}
  components={{
    code({ className, children }) {
      const match = /language-(\w+)/.exec(className ?? '')
      return match
        ? <SyntaxHighlighter style={oneDark} language={match[1]} PreTag="div">
            {String(children).replace(/\n$/, '')}
          </SyntaxHighlighter>
        : <code className={className}>{children}</code>
    }
  }}
>
```

### 5.2 LaTeX (이미 katex 의존성 있음)

`remark-math` + `rehype-katex` 추가:

```bash
bun add remark-math rehype-katex
```

---

## 6. 구현 순서

### Phase 1: Backend Wire Format (kernel + gateway)
1. `KernelEvent`에 새 변종 5개 추가 (`event_bus.rs`)
2. `agent_runtime.rs` 콜백에서 event bus publish
3. `chat.rs` WS handler에서 event bus 구독 + chunk 변환
4. `orchestrator.rs`에서 session_id를 agent runtime에 전달
5. 단위 테스트: chunk 직렬화

### Phase 2: Session Persistence
6. `Session`에 `trajectory_steps` 필드 추가 (`state_store.rs`)
7. `OrchestrationResult` → trajectory 저장 로직
8. `loadSession` API에 trajectory 포함
9. 통합 테스트

### Phase 3: Frontend Core
10. `StreamChunk` 타입 확장
11. `ChatActivity` 타입 정의
12. `handleChunk` 확장
13. `ActivityTimeline` + `ActivityCard` 컴포넌트
14. `MessageBubble`에 삽입
15. `loadSession` → activity 복원

### Phase 4: Polish
16. `PhaseIndicator` 진행바
17. Syntax highlighting (react-syntax-highlighter)
18. `remark-math` + `rehype-katex`
19. i18n 키 추가 (en.json, ko.json)
20. 로딩 중 현재 페이즈 헤더 표시

---

## 7. 성능 고려사항

- **Chunk 빈도:** tool 호출은 초당 수 회 → 문제 없음. `reasoning`은 모델이 stream할 때마다 → throttle (500ms debounce)
- **메모리:** `ChatActivity[]`는 메시지당 평균 10-30개 → 1KB 이하. 100개 메시지 세션도 100KB 미만
- **WS 대역폭:** tool output은 `output_summary` (500자)만 전송. 전체 output은 done의 `tool_calls`에
- **Event bus fan-out:** 현재 모든 subscriber가 모든 event를 받음. session_id filtering으로 자신의 것만 처리

## 8. 보안

- `tool_args` / `output_summary`에 민감 정보(API key, password)가 포함될 수 있음
- WebChannel의 `sanitize_event()`와 동일한 수준으로 필터링
- `reasoning` chunk에 사용자 PII가 포함될 수 있으므로 WS 연결 시에만 전송 (SSE 미전송)

## 9. 하위 호환성

- 새 chunk type을 모르는 기존 client는 `type` field로 무시 가능
- `StreamChunk` 유니온에 새 type 추가 → TypeScript가 자동으로 narrow
- 기존 `done` chunk의 `tool_calls` 필드 유지 → 구버전 client도 동작
