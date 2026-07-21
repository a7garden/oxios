# LobeHub Chat Port — Design Document

> **Date:** 2026-07-21
> **Supersedes / extends:** `docs/lobehub-analysis/05-implementation-design.md` (component-level porting), `00-master-synthesis.md` (high-level gap)
> **Scope:** Full chat UX transplant — streaming protocol, data model, rendering pipeline, composer. Backend adapter only where Oxios kernel semantics demand it.
> **Source:** LobeHub v2.2.9 (`/tmp/lobehub`, 13 137 files) analyzed 2026-07-21 via 5 parallel scouts: stream arch, message render, ChatInput, tool render, state model.

---

## 0. TL;DR

| Question | Answer |
|---|---|
| 채팅 입력 문제 해결됐나? | **Fix는 작업트리에 있고 논리적으로 정확하나, 커밋되지 않았고 브라우저 검증 안 됨.** Tiptap `editable`을 mount 시점 `connected=false`로 만들어 두고, 연결 후 동기화하지 않는 버그. `useEffect`로 `setEditable(!disabled && !!connected)` 호출하는 fix가 로컬에 있음 (`web/src/components/chat/chat-input.tsx:246-248`). 빌드는 통과. |
| LobeHub "그대로 이식"? | **불가.** LobeHub는 antd-style CSS-in-JS + Lexical + 72-provider runtime + PostgreSQL/Drizzle 기반이라 Oxios 스택(Tailwind + Tiptap + 단일 Rust daemon + 파일 시스템)과 근본 충돌. |
| 그러면 뭘 가져오나? | **LobeHub의 _구조와 로직_ 을 가져오고 스타일/의존성은 Tailwind로 치환.** 특히 (1) **streaming chunk taxonomy**, (2) **ChatMessage content-parts 모델**, (3) **4-tier tool render registry**, (4) **assistant message content pipeline**(Reasoning → Search → FileChunks → Display → Images), (5) **ChatItem wrapper + hover actions**. |
| 전략은? | 3경로 중 **"Reimplement UX patterns, port component logic verbatim"** 채택. 아래 §2 참조. |
| transport/백엔드? | **WebSocket 그대로, 백엔드 chunk type도 그대로.** 품질은 컴포넌트/데이터모델에 있고 transport가 아니다. 프론트엔드 어댑터(`lib/stream/adapter.ts`, 약 60줄)가 Oxios WS chunk → 컴포넌트가 기대하는 ChatEvent shape으로 변환. 백엔드는 최소 변경(messageId 발급 등, 옵션). SSE 전환 ❌.

---

## 1. Chat Input Fix — 검증 결과

### 1.1 버그 원인 (추적 완료)

`web/src/components/chat/chat-input.tsx:220-248`:

```tsx
const editor = useEditor({
  content: value,
  editable: !disabled && !!connected,    // ← (A) 생성 시점 값
  onUpdate: ({ editor }) => { onChange(editor.getText()) },
  // ...
})

// 로컬 fix (미커밋):
useEffect(() => {
  if (editor) editor.setEditable(!disabled && !!connected)   // ← (B) 동기화
}, [editor, connected, disabled])
```

Tiptap의 `useEditor({ editable })` 옵션은 **생성 시점에만** 적용된다. Oxios daemon 연결은 mount 이후에 완료되는 게 보통이므로 `connected`는 초기 `false` → 에디터가 `contenteditable=false`로 생성 → 이후 `connected=true`가 되어도 React state는 바뀌지만 Tiptap 내부 상태는 안 바뀐다. 결과: **입력 불가.**

Fix (B)는 `editor.setEditable()` imperative API로 props 변화를 Tiptap에 전달. 올바른 수정.

### 1.2 검증 상태

| 검증 | 결과 |
|---|---|
| `bun run build` | ✅ 통과 (`✓ built in 1.74s`, fix 포함) |
| TypeScript / lint | ✅ 빌드 통과로 간접 확인 |
| 브라우저 수동 테스트 | ❌ **미실행** |
| Git 커밋 | ❌ **미커밋** (작업트리에만 존재, `main` 브랜치) |

**권고:** fix를 별도 커밋으로 `fix(web): unlock ChatInput editor after daemon reconnect`로 분리해서 작성할 것. 설계 문서와 함께 커밋하면 리뷰가 어려우므로 쪼개는 게 낫다.

---

## 2. 세 가지 이식 경로 비교

> advisory가 제시한 기준. 각각의 비용/위험/이점을 정량화했다.

| 경로 | 코드 가치 | 의존성 비용 | 유지보수 | 충실도 | 채택? |
|---|---|---|---|---|---|
| **A. 전체 fork + API 어댑터** | LobeHub 그대로 | antd, antd-style, Lexical, Drizzle, Better-Auth, Casdoor, 76 packages 모두 도입 | LobeHub 업스트림 추적 비용 ↑↑↑, 버전 충돌 빈번 | 100% | ❌ |
| **B. 채팅 컴포넌트만 추출 (npm 패키지로)** | LobeHub `features/Conversation` 패키징 | `@lobehub/ui`, `@lobehub/editor`, antd-style 그대로 | Oxios 디자인 시스템(Tailwind)과 충돌, 두 디자인 언어 공존 | 70% | ❌ |
| **C. UX 패턴 재구현 + 로직은 그대로 포팅** | 구조/로직 80-95% 복사, 스타일만 Tailwind | 기존 의존성 유지 (`@tiptap`, shadcn/ui, zustand) | Oxios 코드베이스 안에 흡수, 단일 디자인 언어 | 85% | ✅ |

### 왜 C인가

- **의존성 폭발 방지**: LobeHub는 antd 생태계 기반. `@lobehub/ui`는 antd-style(createStaticStyles)을 전제. Tailwind와 양립 불가.
- **디자인 일관성**: Oxios는 이미 Tailwind + shadcn/ui로 통일. 두 디자인 언어가 공존하면 유지보수 비용이 지배적.
- **로직은 거의 그대로**: LobeHub의 conversation 로직은 React + zustand + rehype/remark 플러그인으로, **외부 UI 의존성은 Flexbox/Accordion/ScrollArea/Markdown 4개뿐** (`05-implementation-design.md` §0). 이는 모두 shadcn/ui 또는 `<div className="flex">`로 1:1 치환된다.
- **백엔드는 그대로**: LobeHub는 Next.js Route Handler + Drizzle이지만, Oxios는 Rust daemon. 어차피 _어댑터_ 가 필요하므로 어댑터 비용은 세 경로 모두 동일.

---

## 3. LobeHub 아키텍처 요약 (Scout 결과 종합)

### 3.1 스트리밍 파이프라인 (LobeHubStreamArch)

```
Browser
  │ fetchEventSource (POST webapi/chat/[provider])
  ▼
Next.js route.ts (thin passthrough)
  │ modelRuntime.chat(data, { signal })
  ▼
packages/model-runtime
  │ OpenAI SDK → raw stream
  ▼
OpenAIStream transformer
  │ maps → StreamProtocolChunk (13 types)
  ▼
createSSEProtocolTransformer
  │ serializes → "event: text\ndata: {...}\n\n"
  ▼
Response (ReadableStream of SSE)
  │
Browser fetchSSE
  │ onmessage switch(ev.event) dispatches typed chunks
  ▼
StreamingHandler (client state machine)
  │ accumulates text/reasoning/tools/images
  ▼
StreamingCallbacks → zustand dispatchMessage
```

**13 StreamProtocolChunk 타입**: `text`, `reasoning`, `reasoning_part`, `content_part`, `tool_calls`, `tool_call_delta`, `grounding`, `usage`, `stop`, `file`, `image`, `video`, `audio` (멀티모달).

**클라이언트 state machine**: `StreamingHandler.handleChunk(chunk)` → accumulated state → `handleFinish()` → final `StreamingResult`. Abort는 `AbortController` (per-operation).

**Reasoning은 first-class**: `text`와 별개 `reasoning` 이벤트로 흐른다. Oxios는 현재 reasoning을 `activities[].type === 'reasoning'` 사이드 채널로 처리.

### 3.2 메시지 렌더링 파이프라인 (LobeHubChatRender)

```
Messages/index.tsx
  switch(message.role):
    user → UserMessage
    assistant → AssistantMessage
    assistantGroup → AssistantGroupMessage   ← (tool calls이 여기 그룹핑됨)
    tool, task, tasks, groupTasks, agentCouncil, verify, taskCallback
  → 모두 ChatItem wrapper 사용

ChatItem (universal wrapper)
  avatar │ title │ error │ MessageContent(body) │ messageExtra │ actions │ FollowUpChips

AssistantMessage content pipeline (fixed order):
  SearchGrounding → FileChunks → Reasoning → DisplayContent → ImageFileListViewer → ReactionDisplay

DisplayContent:
  RichContentRenderer (multimodal parts) OR MarkdownMessage
  content === LOADING_FLAT → ContentLoading

MarkdownMessage:
  @lobehub/ui Markdown + 13 rehype/remark plugins
  (Thinking, LobeArtifact, LobeThinking, LocalFile, Mention, Skill, Tool,
   Task, UserFeedback, ImageSearchRef, LobeAgents, LocalFileLink, Link)
```

**Streaming UX**: `isLoading`, `editing` props로 제어. `ContentLoading`이 elapsed-time 표시 (operation label 자동 추론).

**Actions**: copy, retry, branch(=branching), edit, share, translate, tts, del, delAndRegenerate, regenerate, restoreToInput, select, collapse, continueGeneration.

### 3.3 ChatInput (LobeHubChatInput)

- **Editor**: Lexical (`@lobehub/editor/react`) + 11+ plugins
- **ActionBar**: 16 actions (config map at `ActionBar/config.ts`)
  - agentMode, clear, contextWindow, fileUpload, history, memory, mention, model, modelLabel, params, plus, promptTransform, search, temperature, tools, typo
- **ControlBar**: ModeSelector + WorkspaceControls + ApprovalMode + ContextWindow
- **@-mention**: 6 categories (private/workspace agents, members, topics, skills, tools). Fuse.js 퍼지 검색.
- **ActionTags**: Lexical decorator nodes. 인라인 칩으로 `<skill/>`, `<tool/>`, `<projectSkill/>`, `<action/>` 삽입. `/` 메뉴로 호출.
- **Store**: per-instance zustand (글로벌 아님). `onSend`/`leftActions`/`rightActions` props로 주입.
- **IME**: `useIMECompositionEvent` + `useEnterToSend` 로 안전.

### 3.4 Tool Rendering (LobeHubToolRender)

**Data model** (`packages/types/src/message/common/tools.ts`):
```ts
ChatToolPayload {
  id: string
  identifier: string          // tool package name (e.g. "builtin-tool-local-system")
  apiName: string             // specific tool (e.g. "readFile")
  arguments: string           // JSON
  result?: ChatToolResult
  intervention?: ToolIntervention
}
```

**4-tier registry** (`packages/builtin-tools/src/register.ts`):
1. **renders** — 완료 후 결과 표시 컴포넌트
2. **inspectors** — 헤더 인라인 컴포넌트 (상태/이름/시간 대신 custom)
3. **streamings** — 실행 중 실시간 progress
4. **interventions** — human approval UI

각 툴 호출은 `AccordionItem`:
- Title bar = **Inspector** (StatusIndicator + ToolTitle + ExecutionTime)
- Body = **Detail** (streaming | rejected | aborted | custom render | FallbackArgumentRender)

**Catalog**: 30+ builtin-tool 패키지 중 Oxios kernel 툴에 직접 매핑 가능한 것은 ~10개:
`readFile, writeFile, editFile, runCommand, glob, grep, listFiles, webSearch, webFetch, agentDispatch`.

LobeHub 고유(GitHub/Linear/Plans/Todos/AgentBuilder/SkillStore/KnowledgeBase 등 20+)는 **skip**.

### 3.5 State Model (LobeHubStateModel)

- **12 slices** zustand store (message, topic, thread, agentRun, plugin, portal, operations, tts, translation, forwarding, builtinTool, aiAgent)
- **Dual layer**: flat DB rows (Drizzle/PGlite) → `@lobechat/conversation-flow` parser → display messages (grouped children + tool blocks)
- **UIChatMessage**: 40+ fields
- **Topic**: status lifecycle (active→running→completed/archived), parentId 체인으로 branching
- **Persistence**: Postgres (Drizzle) 서버 / PGlite 클라이언트 / Dexie/IndexedDB SWR 캐시

---

## 4. Oxios 현재 상태 (Baseline)

### 4.1 백엔드 스트리밍

**Transport**: WebSocket at `GET /api/chat/stream` (`src/api/routes/chat.rs:466-1237`). SSE는 `/api/events` (KernelEvent 브로드캐스트) 용도로만 사용.

**WS protocol** (RFC-015 기준):
```
Incoming (FE → BE):
  { type: "message", content, session_id?, project_id? }

Outgoing (BE → FE) — chunk types:
  { type: "model",      model: "..." }                          // 모델 발표
  { type: "token",      content: "..." }                        // 텍스트 delta
  { type: "tool_start", tool_name, tool_call_id, ... }
  { type: "tool_progress", tool_name, tool_call_id, progress? }
  { type: "tool_end",   tool_name, tool_call_id, result?, duration_ms? }
  { type: "reasoning",  ... }                                    // (existing, RFC-015)
  { type: "phase",      phase: "Execute" }
  { type: "memory",     ... }
  { type: "usage",      ... }
  { type: "error",      error: {...} }
  { type: "done",       session_id?, phase?, evaluation_passed?, duration_ms? }
  { type: "interview_question", ... }
  { type: "tool_approval", ... }
  { type: "mount_detected", ... }
```

**관찰**: 이미 LobeHub의 13-event taxonomy와 거의 대응. 다음이 빈약하거나 없음:
- `reasoning_part` / `content_part` (멀티모달 파트) — 현재 단일 `reasoning.content` string
- `grounding` (search citations) — tool_end에 payload로 끼어있음 (별도 이벤트 아님)
- `usage` — assistant message footer에만 표시, 별도 chunk 없음
- `file` / `image` / `video` / `audio` — 생성형 멀티모달 미지원

### 4.2 프론트엔드 데이터 모델 (`web/src/types/index.ts:180-231`)

```ts
ChatMessage {
  id, role: 'user'|'assistant'|'system'|'tool', content, model, timestamp,
  toolName?, toolArgs?, toolResult?, toolDurationMs?,
  metadata?: { phase, evaluation_passed, duration_ms, tool_calls?, isError?, errorKind? },
  activities?: ChatActivity[],         // RFC-015 transparency timeline
  totalInputTokens?, totalOutputTokens?,
  _interviewQuestions?, _interviewRound?,
}
```

**ChatActivity**: `{ type: 'phase'|'tool_call'|'memory'|'reasoning'|'usage', ... }` — 사이드 채널.

**ChatMessageExtensions** (`web/src/types/chat.ts:96-118`): 이미 LobeHub 필드 일부 선언만 되어 있음 (`reasoning`, `search`, `chunksList`, `imageList`, `tools`, `error`). 미사용.

### 4.3 프론트엔드 스토어 (`web/src/stores/chat.ts`)

- **Single zustand store** `useChatStore` (`persist` wrapping)
- `handleChunk(chunk: StreamChunk)` — inline reducer. token은 RAF batching (F9).
- `_pendingQueue` — 메시지 큐잉 (사용자가 입력을 잠그지 않고 연속 전송)
- `_pendingTokens` — token flush 버퍼
- WS dedup: `seen_ids` ring (256) + `last_seq` cursor (RFC-024 SP2)
- `useChatStore` ↔ `useQuickAskStore`는 shared helpers(`appendTokenToMessages`, `appendActivityToMessages`, `patchAssistantModel`)로 동작

### 4.4 이미 포팅된 컴포넌트 (Main 브랜치 기준)

- `web/src/components/chat/chat-item/` — LobeHub ChatItem Tailwind 포팅 ✅
- `web/src/components/chat/thinking/` — Thinking accordion ✅
- `web/src/components/chat/content-loading.tsx` — elapsed timer ✅
- `web/src/components/chat/search-grounding.tsx` — citations + image grid ✅
- `web/src/components/chat/tool-renders/registry.tsx` — tool render registry (1-tier) ✅
- `web/src/components/chat/tool-renders/WebSearch.tsx` — ✅

**상태**: 컴포넌트는 부분 포팅됐으나 **스트리밍 파이프라인/데이터 모델은 LobeHub 수준에 도달하지 못함**. 이것이 이번 설계의 핵심 갭.

---

## 5. 전송 계층: 변경 최소, 프론트엔드 어댑터만 (쉬운 부분)

> **핵심 통찰:** LobeHub의 응답 품질은 transport에 있지 않다. LobeHub가 SSE를 쓰고 Oxios가 WebSocket을 쓰는 건 품질과 무관한 구현 디테일이다. 진짜 가치는 (a) **ChatMessage 데이터 모델**(아래 §6.1)과 (b) **StreamingHandler 클라이언트 상태머신**(§6.2)에 있다. 이 절에서는 백엔드 변경을 최소화하는 전략만 정의한다.

### 5.1 결정: 백엔드 WS chunk 그대로, 프론트엔드에서 어댑트

**백엔드 (`src/api/routes/chat.rs`)는 거의 손대지 않는다.** 기존 chunk type이 이미 LobeHub의 `StreamProtocolChunk`와 의미상 1:1 대응한다:

| Oxios WS chunk (current) | LobeHub StreamProtocolChunk | 비고 |
|---|---|---|
| `token` | `text` | 이름만 다름. 어댑터에서 rename |
| `reasoning` (RFC-015 activity) | `reasoning` (+ `reasoning_part`) | Oxios는 activity 사이드채널 → first-class `message.reasoning` 필드로 승격 (어댑터에서) |
| `tool_start` / `tool_progress` / `tool_end` | `tool_calls` + `tool_call_delta` | Oxios가 더 세분화돼 있음 — 그대로 두고 어댑터에서 조합해 `ChatToolPayload`로 |
| `done` | `stop` (+ `usage`) | 동일 |
| `error` | (스트림 종료로 취급) | 동일 |
| `model` announcement | (LobeHub는 별도 chunk 없음) | Oxios 고유 — 유지, 첫 chunk로 사용 |

**프론트엔드 어댑터** (`web/src/lib/stream/adapter.ts`, 신규 약 60줄):

```ts
// Oxios WS chunk → unified ChatEvent (StreamProcessor와 컴포넌트가 소비)
export type ChatEvent =
  | { kind: 'text.delta';       messageId: string; text: string }
  | { kind: 'reasoning.delta';  messageId: string; text: string }
  | { kind: 'reasoning.start';  messageId: string }
  | { kind: 'reasoning.end';    messageId: string; durationMs?: number }
  | { kind: 'tool.start';       messageId: string; toolCallId: string; toolName: string; args?: unknown }
  | { kind: 'tool.progress';    messageId: string; toolCallId: string; progress?: string }
  | { kind: 'tool.end';         messageId: string; toolCallId: string; result?: unknown; durationMs?: number; error?: ChatError }
  | { kind: 'grounding';        messageId: string; search: GroundingSearch }
  | { kind: 'file_chunks';      messageId: string; chunks: ChatFileChunk[] }
  | { kind: 'usage';            messageId: string; usage: TokenUsage }
  | { kind: 'phase';            phase: string; evaluationPassed?: boolean }
  | { kind: 'stream.stop';      messageId?: string; reason: 'done'|'aborted'|'error'; error?: ChatError }

export function adaptChunk(raw: StreamChunk, ctx: { currentAssistantId: () => string }): ChatEvent[] {
  const mid = ctx.currentAssistantId()
  switch (raw.type) {
    case 'token':         return [{ kind: 'text.delta', messageId: mid, text: raw.content }]
    case 'tool_start':    return [{ kind: 'tool.start', messageId: mid, toolCallId: raw.tool_call_id, toolName: raw.tool_name, args: raw.tool_args }]
    case 'tool_progress': return [{ kind: 'tool.progress', messageId: mid, toolCallId: raw.tool_call_id, progress: raw.progress }]
    case 'tool_end':      return [{ kind: 'tool.end', messageId: mid, toolCallId: raw.tool_call_id, result: raw.tool_result, durationMs: raw.duration_ms }]
    case 'reasoning':     return [{ kind: 'reasoning.delta', messageId: mid, text: raw.content ?? '' }]
    case 'done':          return [{ kind: 'stream.stop', messageId: mid, reason: 'done' }]
    case 'error':         return [{ kind: 'stream.stop', messageId: mid, reason: 'error', error: raw.error }]
    // phase, memory, usage, interview_question, tool_approval, mount_detected — 동일 패턴
    default:              return []
  }
}
``+
이 어댑터는 thin layer. `StreamProcessor` (§6.2)가 소비한다.

### 5.2 백엔드 최소 변경 (선택, 블로킹 아님)

다음 두 가지는 백엔드에서 _추가_ 할 수 있지만, 프론트엔드에서도 우회 가능하므로 **블로킹이 아니다**:

1. **MessageId 발급** — 현재 Oxios는 "마지막 assistant 메시지"를 implicit target으로 삼는다 (`ensureLastAssistant`). 백엔드가 첫 chunk에 messageId를 붙여주면 동시 스트리밍(멀티 agent, 백그라운드 작업)이 깔끔해진다. **프론트엔드 우회**: 어댑터가 `currentAssistantId()`로 가장 최근 assistant id를 사용.
2. **`reasoning.start` / `reasoning.end` 분할** — 현재 Oxios는 reasoning을 단일 청크로 보낸다. start/end 구분이 있으면 Thinking 블록의 자동 펼침/접힘 애니메이션이 자연스럽다. **프론트엔드 우회**: 첫 reasoning delta를 start로, `done`을 end로 해석.

→ **Phase 1에서는 프론트엔드 우회로 진행**. 백엔드 변경은 별도 PR로 분리 (후보: RFC-045 "스트리밍 메시지 ID 명시화").

### 5.3 하지 않을 것

- **SSE로 전환** ❌ — Oxios WS의 양방향(send + receive + tool approval + interview)이 단일 소켓으로 잘 동작. SSE 전환은 send 채널을 POST로 분리해야 하는 등 부수 비용만 증가.
- **WS chunk type 이름 변경 (v2 taxonomy)** ❌ — 백엔드-프론트엔드 양쪽을 동시에 바꿔야 하는 롤아웃 리스크. 어댑터로 충분.
- **deprecation 주기 도입** ❌ — 오버엔지니어링. 단순 rename이 필요하면 한 사이클에 끝낼 것.
## 6. 데이터 모델 설계

### 6.1 ChatMessage v2 (LobeHub 정렬)

`ChatMessageExtensions`를 **실제 ChatMessage에 통합**:

```ts
// web/src/types/index.ts (proposed)
export interface ChatMessage {
  // ── Identity ──
  id: string
  role: 'user' | 'assistant' | 'system' | 'tool'
  parentId?: string                  // branching (LobeHub-style)
  threadId?: string                  // thread support
  
  // ── Content (multi-part, LobeHub-aligned) ──
  content: string                    // primary text (markdown)
  contentParts?: ContentPart[]       // multimodal / structured
  
  // ── Reasoning (first-class) ──
  reasoning?: ModelReasoning | null  // { content, durationMs?, modelId? }
  
  // ── Tool calls (structured, replaces toolName/toolArgs/toolResult) ──
  toolCalls?: ChatToolPayload[]      // 0..N per message
  
  // ── Search/RAG ──
  search?: GroundingSearch           // web search citations
  chunksList?: ChatFileChunk[]       // knowledge RAG refs
  
  // ── Media ──
  imageList?: ChatImageItem[]
  fileList?: ChatFileItem[]
  
  // ── Meta ──
  model?: string
  timestamp: string
  metadata?: ChatMessageMetadata     // phase, evaluation, tokens, cost
  activities?: ChatActivity[]        // KEEP: timeline (phase/memory/usage), 낮은 우선순위로 전환
  error?: ChatError
  
  // ── Oxios-specific ──
  _interviewQuestions?: InterviewQuestion[]
  _interviewRound?: number
  
  // ── Lifecycle ──
  generating?: boolean               // stream in progress
}

export interface ChatToolPayload {
  id: string                          // tool_call_id
  identifier: string                  // tool package name (e.g. 'kernel')
  apiName: string                     // specific tool (e.g. 'read_file')
  arguments: unknown                  // parsed args
  result?: unknown
  error?: ChatError
  status: 'loading' | 'success' | 'error' | 'aborted'
  startedAt?: number
  endedAt?: number
  durationMs?: number
  intervention?: ToolIntervention     // approval state
  progress?: string                   // live progress text
}

export type ContentPart =
  | { type: 'text'; text: string }
  | { type: 'image'; image: ChatImageItem }
  | { type: 'file'; file: ChatFileItem }
  | { type: 'tool_call'; toolCallId: string }
  | { type: 'reasoning'; text: string }
```

### 6.2 StreamingHandler — 클라이언트 상태머신 (이 설계의 심장)

> LobeHub의 `StreamingHandler` (`src/store/chat/agents/StreamingHandler.ts`)를 그대로 포팅. Oxios 현재는 `useChatStore.handleChunk` 안에 인라인 reducer로 구현돼 있는데, 이게 reasoning merge 버그(`chat.ts:312-316` 주석에 언급)나 tool call 동시성 문제의 근원이다. 별도 클래스로 추출한다.

**역할**: 하나의 assistant 메시지에 대한 스트림 상태를 축적하고, 완료 시 `ChatMessage`로 materialize. Zustand store는 이 결과를 받아 `messages[]`에 반영할 뿐이다.

```ts
// web/src/lib/stream/StreamProcessor.ts (신규)
export class StreamProcessor {
  private messageId: string
  private text = ''
  private reasoning = ''
  private reasoningStartTs: number | null = null
  private tools = new Map<string, ChatToolPayload>()
  private search?: GroundingSearch
  private chunks: ChatFileChunk[] = []
  private usage?: TokenUsage
  private error?: ChatError
  private aborted = false

  constructor(messageId: string) { this.messageId = messageId }

  /** 어댑터(§5.1)가 변환한 ChatEvent를 소비. RAF batching은 호출자 책임. */
  handleEvent(ev: ChatEvent): { patch: Partial<ChatMessage>; finished?: boolean } {
    switch (ev.kind) {
      case 'text.delta':
        this.text += ev.text
        return { patch: { content: this.text, generating: true } }
      case 'reasoning.start':
        this.reasoningStartTs = Date.now()
        return { patch: { reasoning: { content: '', durationMs: 0 } } }
      case 'reasoning.delta':
        this.reasoning += ev.text
        return { patch: { reasoning: { content: this.reasoning, durationMs: this.reasoningDuration() } } }
      case 'reasoning.end':
        return { patch: { reasoning: { content: this.reasoning, durationMs: ev.durationMs ?? this.reasoningDuration() } } }
      case 'tool.start':
        this.tools.set(ev.toolCallId, { id: ev.toolCallId, identifier: 'kernel', apiName: ev.toolName, arguments: ev.args, status: 'loading', startedAt: Date.now() })
        return { patch: { toolCalls: Array.from(this.tools.values()) } }
      case 'tool.progress':
        this.mergeTool(ev.toolCallId, { progress: ev.progress })
        return { patch: { toolCalls: Array.from(this.tools.values()) } }
      case 'tool.end':
        this.mergeTool(ev.toolCallId, { result: ev.result, status: ev.error ? 'error' : 'success', endedAt: Date.now(), durationMs: ev.durationMs, error: ev.error })
        return { patch: { toolCalls: Array.from(this.tools.values()) } }
      case 'grounding':    this.search = ev.search;     return { patch: { search: this.search } }
      case 'file_chunks':  this.chunks = ev.chunks;     return { patch: { chunksList: this.chunks } }
      case 'usage':        this.usage = ev.usage;       return { patch: { metadata: { ...placeholderMeta, usage: this.usage } } }
      case 'stream.stop':  this.error = ev.error; this.aborted = ev.reason === 'aborted'
        return { patch: { generating: false, error: ev.error }, finished: true }
      default: return { patch: {} }
    }
  }

  /** 스트림 종료 후 최종 ChatMessage. handleEvent에서 축적한 모든 상태 반영. */
  materialize(base: ChatMessage): ChatMessage {
    return { ...base, id: this.messageId, content: this.text, reasoning: this.reasoning ? { content: this.reasoning, durationMs: this.reasoningDuration() } : null, toolCalls: Array.from(this.tools.values()), search: this.search, chunksList: this.chunks, error: this.error, generating: false }
  }

  private reasoningDuration(): number | undefined {
    return this.reasoningStartTs ? Date.now() - this.reasoningStartTs : undefined
  }
  private mergeTool(id: string, patch: Partial<ChatToolPayload>) {
    const cur = this.tools.get(id); if (!cur) return
    this.tools.set(id, { ...cur, ...patch })
  }
}
```

**Zustand store 통합** (`web/src/stores/chat.ts`):
- `handleChunk`는 `adaptChunk(raw)` → `processor.handleEvent(ev)` → store patch 적용으로 단순화
- 메시지별 `StreamProcessor` 인스턴스 관리: `Map<messageId, StreamProcessor>` (동시 스트림 대비)
- 토큰 batching (F9 RAF)은 유지 — `text.delta` 이벤트를 버퍼링
- `_pendingTokens` / `flushPendingTokens` / `scheduleTokenFlush` 로직은 그대로, 다만 `_pendingTokens += ev.text` 형태로 변경

**useQuickAskStore와의 공유**: 현재 `appendTokenToMessages` / `appendActivityToMessages` / `patchAssistantModel` 헬퍼를 quick-ask와 chat가 공유. StreamProcessor 도입 후에는 두 스토어 모두 StreamProcessor를 사용하므로 헬퍼는 deprecated → 제거. 중복 코드 사라짐.

**이게 왜 핵심인가**: LobeHub 응답 품질의 80%는 "실시간으로 축적되는 정확한 상태"에서 나온다. reasoning과 text가 동시에 스트리밍될 때 두 영역이 깜빡이지 않게, tool call의 진행 상태가 매끄럽게 전환되게, 에러 발생 시 스트림이 깨끗이 종료되게 — 이게 다 StreamingHandler의 책임이다. 어댑터(§5)는 변환만, 컴포넌트(§7)는 렌더링만. **상태 축적 로직이 단일 클래스에 모여 있어야 reasoning merge 버그 같은 drift가 재발하지 않는다.**

### 6.3 마이그레이션 전략

**기존 필드 보존 + 점진적 이전**:
- `toolName`/`toolArgs`/`toolResult`/`toolDurationMs` → `toolCalls: [ChatToolPayload]`로 통합
  - 1사이클 동안 양쪽 유지 (loader에서 변환)
  - 2사이클 후 구 필드 제거
- `metadata.tool_calls` → `toolCalls`로 이관
- `activities[type='reasoning']` → `reasoning` 필드로 이관
- `activities[type='tool_call']` → `toolCalls`로 이관
- `activities[type='phase'|'memory'|'usage']` → 유지 (타임라인 표시용)

**Session history 로딩** (`stores/chat.ts:983` 영역): 백엔드 `/api/sessions/:id` 응답을 v2 형식으로 정규화하는 `normalizeLegacyMessage()` 헬퍼 도입.

### 6.4 Branching — **본 설계 범위 외 (사용자 결정 2026-07-21)**

LobeHub는 `parentId` 체인으로 message branching을 지원. Oxios는 linear 모델 유지. Branching 데이터 모델(`parentId`, `threadId`)과 UI 모두 **이번 설계에서 제외**. content pipeline 포팅과 무관하게 동작하므로 나중에 별도 설계에서 다룬다.

---

## 7. 컴포넌트 마이그레이션 계획

> `docs/lobehub-analysis/05-implementation-design.md`의 파일 맵은 그대로 유효. 여기서는 **순서와 의존성**을 정의.

### Phase 1 — Streaming Foundation (1-2일)

> 가장 큰 가시적 효과. 백엔드 변경 없이 프론트엔드만으로 LobeHub 수준의 스트리밍 확보.

**백엔드**: 변경 없음 (§5.1 원칙). chunk taxonomy와 WS 프로토콜 그대로.

**프론트엔드**:
- **신규 파일** `web/src/lib/stream/adapter.ts` (§5.1) — Oxios WS chunk → ChatEvent 변환 (약 60줄)
- **신규 파일** `web/src/lib/stream/StreamProcessor.ts` (§6.2) — 클라이언트 상태머신 클래스 (약 120줄)
- `stores/chat.ts`:
  - `handleChunk`를 `adaptChunk → processor.handleEvent → store patch`로 단순화 (기존 inline reducer는 제거)
  - 메시지별 `StreamProcessor` 인스턴스 관리: `Map<messageId, StreamProcessor>` (현재는 단일 스트림이지만 향후 동시 스트림 대비)
  - 토큰 batching (F9 RAF) 유지 — `text.delta` 이벤트를 `_pendingTokens += ev.text` 형태로 버퍼링
  - `appendTokenToMessages` / `appendActivityToMessages` / `patchAssistantModel` 헬퍼는 StreamProcessor 내부로 흡수 → 제거
- `types/chat.ts`의 `ChatMessageExtensions`를 `types/index.ts`의 `ChatMessage`로 통합 (§6.1)
- `useQuickAskStore`도 동일한 StreamProcessor 사용 → quick-ask와 chat의 스트리밍 코드 중복 제거

**수용 기준**:
1. 동일한 메시지에서 reasoning과 text가 동시에 스트리밍될 때 Thinking block + 본문이 분리 렌더링되는지 (reasoning merge 버그 재발 없이)
2. tool 호출이 여러 번 일어날 때 각 tool의 progress/end가 같은 activity slot에 merge되는지
3. 스트림 중 에러 발생 시 `generating=false`로 깨끗이 종료되는지

### Phase 2 — AssistantMessage Pipeline (2-3일)

> LobeHub `MessageContent.tsx`의 6-stage pipeline을 Oxios `AssistantMessage`로 이식.

- `web/src/components/chat/messages/` 디렉토리 신설
  - `index.tsx` — role dispatcher (현재 `message-bubble.tsx`를 thin dispatcher로)
  - `Assistant.tsx` — pipeline:
    ```
    Reasoning → SearchGrounding → FileChunks → DisplayContent → Images → Files
    ```
  - `User.tsx` — placement='right', 첨부 파일
  - `Tool.tsx` — standalone tool message (role='tool')
- 기존 포팅 컴포넌트(thinking, search-grounding, content-loading)를 pipeline에 wire
- `DisplayContent.tsx`:
  - `generating=true` + `content=''` → ContentLoading (operation label 추론)
  - multimodal `contentParts` → RichContentRenderer
  - 단순 텍스트 → MarkdownMessage

**수용 기준**: reasoning 모델(gpt-5, claude-4-thinking) 응답에서 "Thinking… Ns" 블록이 실시간 갱신되고, 완료 후 자동 접힘. 그 아래 본문이 자연스럽게 이어짐.

### Phase 3 — Tool Render 4-Tier Registry (2-3일)

> LobeHub의 renders/inspectors/streamings/interventions 패턴 도입.

- `web/src/components/chat/tool-renders/` 확장:
  - `registry.tsx` — 4개 맵 (`renders`, `inspectors`, `streamings`, `interventions`)
  - `Inspector.tsx` — universal header (status + name + duration)
  - `Detail.tsx` — body dispatcher (streaming | render | fallback)
  - `Intervention.tsx` — approval card
- Oxios kernel 툴 매핑 (Phase 1 대상 10개):
  | Oxios tool | LobeHub에서 차용할 패턴 | Source |
  |---|---|---|
  | `read_file` | LocalFile preview + syntax highlight | `builtin-tool-local-system/Render` |
  | `write_file` | diff view | LobeHub `writeFile` |
  | `edit_file` | diff view | LobeHub `editFile` |
  | `bash` / `run_command` | terminal output card | LobeHub `runCommand` |
  | `web_search` | citations grid (이미 포팅됨) | `builtin-tool-web-browsing` |
  | `web_fetch` | page preview | LobeHub `crawlSinglePage` |
  | `glob`, `grep`, `list_files` | file list cards | LobeHub `listFiles` |
  | `agent_dispatch` (A2A) | sub-agent status | LobeHub `agent_dispatch` |
- 백엔드: `tool_call.end` chunk에 `toolName`을 LobeHub identifier(apiName) 구조로 정규화

**수용 기준**: `bash` 툰 호출 시 터미널 출력이 인라인으로 스트리밍되고 syntax highlight 적용. `read_file`은 파일 경로 + 라인 수 + 미리보기.

### Phase 4 — ChatItem Wrapper & Actions (2일)

> hover actions, branching 기초, message-level 메타데이터 표시.

- ChatItem 강화 (`chat-item/index.tsx`):
  - hover 시 actions 노출 (copy, retry, edit, delete)
  - title row: agent/model 이름 + 타임스탬프
  - error display slot
- `MessageActionBar` 신설:
  - assistant: copy | retry | regenerate
  - user: edit | delete
- ~~Branching~~: **본 설계 범위 외** (§6.4 참조).

**수용 기준**: assistant 메시지 hover 시 copy/retry 버튼이 부드럽게 노출. 에러 메시지는 인라인 카드 + 재시도 버튼.

### Phase 5 — Composer (ChatInput) 고도화 (3-4일)

> LobeHub의 ActionBar + ControlBar + @mention + ActionTags 중 Oxios에 맞는 것만.

**Oxios 현재 ChatInput은 이미 Tiptap 기반** — Lexical로 전환하지 않는다 (의존성 비용 ↑, Tiptap으로 충분).

**포팅 대상**:
- **ActionBar 구조화**: `chat-input-action-bar.tsx`를 LobeHub의 `config.ts` + `actionMap` 패턴으로 재구성. 현재는 하드코딩 버튼들.
  - 도입: model picker (이미 있음), search toggle (이미 있음), tools picker (**신규**, Oxios는 현재 mount picker만 있음), knowledge picker (**신규**, multi-select)
  - 스킵: typo, promptTransform, params(temperature), memory toggle (Oxios은 자동), history 화살표
- **Slash commands 고도화**: 현재 SLASH_COMMANDS (10개)를 LobeHub의 `useSlashActionItems` 패턴으로 — installed skills + builtin commands 결합. Tiptap의 `Suggestion` 플러그인 활용.
- **@-mention 카테고리화**: 현재는 knowledge/memory/mount만. LobeHub의 6-category 모델에서 **topics, skills, tools** 추가.
- **IME 안전**: 현재 Tiptap으로 IME 처리 가능. 추가 작업最小.
- **ControlBar**: workspace/mount picker + approval mode 표시 + token counter (신규)

**수용 기준**: `/` 입력 시 스킬/명령 메뉴가 LobeHub처럼 풍부하게 노출. `@` 입력 시 6-category 메뉴.

### Phase 6 — Polish & What Not to Port (1일)

- ~~FollowUpChips~~: **본 설계 범위 외** (사용자 결정). 백엔드 제안 생성이 필요하므로 별도 RFC에서 다룸.
- ~~Message branching navigation UI~~: **본 설계 범위 외** (§6.4).
- Markdown plugins 고도화: Thinking, Artifact, Mention 3개만 추가 포팅.
- Mobile 반응형 점검.

---

## 8. 도입하지 않을 것 (What NOT to Port)

> LobeHub 기능 중 Oxios 컨텍스트에서 **불필요/과잉**인 것들.

| LobeHub 기능 | 도입 여부 | 이유 |
|---|---|---|
| antd / antd-style (createStaticStyles) | ❌ | Tailwind와 충돌. 모든 스타일은 Tailwind로 치환 |
| Lexical (`@lobehub/editor`) | ❌ | Oxios는 이미 Tiptap. Lexical 이식 비용 ↑↑ |
| PostgreSQL/Drizzle dual-layer | ❌ | Oxios는 filesystem + in-memory. 단일 진실 원천 유지 |
| 12+ message role types (task/tasks/groupTasks/agentCouncil/verify/...) | ❌ | Oxios는 user/assistant/tool/system 4개로 충분. 다중 agent 표현은 AssistantGroup 1개로 |
| Better-Auth + Casdoor SSO | ❌ | Single-user daemon. 이미 auth.rs에 토컨 기반 인증 있음 |
| 76-package monorepo 구조 | ❌ | Oxios는 7-crate workspace가 적절 |
| OTEL observability 스택 | ❌ | Phase 외. Prometheus 익스포터는 이미 있음 |
| 30+ builtin-tool 패키지 중 LobeHub 고유 (GitHub, Linear, Plans, Todos, AgentBuilder, SkillStore, KnowledgeBase 등) | ❌ | Oxios는 SKILL.md + kernel tools로 커버 |
| `fetchEventSource` / SSE 전환 | ❌ | WebSocket 유지 (양방향 + 이미 검증된 dedup) |
| `@lobehub/ui` Markdown 컴포넌트 | ❌ | Oxios ReactMarkdown + 기존 remark/rehype 플러그인 유지. LobeHub 플러그인(Thinking/Artifact)만 추가 포팅 |
| Follow-up chips 자동 제안 | ❌ (본 설계 외) | 별도 RFC에서 다룸 |
| Branch navigation UI | ❌ (본 설계 외) | 별도 설계에서 다룸 |
| Electron desktop | ❌ | Web UI + PWA로 충분 (이미 결정됨) |
| Marketplace (Skill Store) | ❌ | ClawHub가 이미 있음 (RFC-010) |
| Chat adapters (WeChat/QQ/Feishu) | ❌ | CLI/Telegram 채널로 충분 |

---

## 9. 의존성 변경

**추가**:
- `katex` (이미 번들에 있음, rehype-katex 활성화만)
- `react-diff-viewer` (또는 자체 구현) — `edit_file` diff 표시용
- `fzf-style` fuzzy matcher (이미 Fuse.js 있음 — 그대로 사용)

**제거**: 없음.

**번들 크기 우려**:
- 현재 index.js 590KB (gzip 182KB). LobeHub 포팅 후 예상 +50-80KB (registry + tool renders). code splitting으로 tool renders는 lazy load (`tool-renders/<name>.tsx`를 `React.lazy`).

---

## 10. 구현 순서 (요약)

```
[Phase 0] 별도 커밋: chat-input fix (이미 로컬에 있음)
    ↓
[Phase 1] Streaming Foundation
    - 백엔드: chunk taxonomy v2 + messageId
    - 프론트엔드: StreamProcessor + messagesById
    ↓
[Phase 2] AssistantMessage content pipeline
    - messages/{Assistant,User,Tool,Dispatcher}
    - DisplayContent + ContentLoading wiring
    ↓
[Phase 3] Tool render 4-tier registry
    - registry + Inspector + Detail + Intervention
    - 10 Oxios kernel tools에 대한 render 구현
    ↓
[Phase 4] ChatItem actions + 에러 UX
    - hover actions, 에러 카드 강화
    - parentId 데이터 준비 (UI는 추후)
    ↓
[Phase 5] Composer (ChatInput) 고도화
    - ActionBar config map 구조화
    - Slash command + @mention 카테고리 확장
    ↓
[Phase 6] Polish + markdown plugins
    - Thinking/Artifact/Mention rehype 플러그인
    - mobile 점검
```

각 Phase는 별도 PR로 분리. Phase 1-2가 가장 큰 임팩트. Phase 1까지만 해도 "LobeHub 수준의 스트리밍"이라고 사용자가 체감할 수 있음.

---

## 11. 다음 액션 (사용자 결정 필요)

1. **이 설계 방향 승인?** — 경로 C(로직 포팅 + Tailwind 재구현), 백엔드 WS 그대로 + 프론트엔드 어댑터, 6-phase 일정. 핵심 투자는 ChatMessage 데이터 모델(§6.1)과 StreamingHandler(§6.2).
2. **Phase 1부터 바로 착수?** — 아니면 특정 Phase를 우선?
3. ~~**branching / follow-up chips**~~ → **✅ 제외 확정** (사용자 결정 2026-07-21).
4. ~~**채팅 입력 fix**~~ → **✅ 별도 커밋 확정** (commit `ff91e3161`, 2026-07-21).

이 4개에 답을 주시면 Phase 1부터 바로 구현 들어갑니다.
