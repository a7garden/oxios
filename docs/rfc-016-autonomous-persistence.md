# RFC-016: Autonomous Persistence

> **Status**: Approved
> **Date**: 2026-06-13
> **Scope**: `oxios-kernel`, `surface/oxios-web`, binary crate (`src/kernel.rs`)
> **Depends on**: RFC-015 (tool description 정비, 완료)

## Problem

RFC-015는 memory와 knowledge의 tool 혼동을 해결했다. 하지만 근본 설계 문제는 남아 있다:

**저장은 에이전트가 스스로 판단해야 한다.**

- memory는 에이전트의 학습 결과다. "이 사용자는 Rust를 선호한다" — 에이전트가 스스로 기록하는 것이지, 사용자가 지시할 영역이 아니다.
- knowledge도 에이전트가 판단할 수 있어야 한다. 설계 문서, 보고서, 분석 결과 같은 마크다운 응답은 사용자가 "저장해줘"라고 말하지 않아도 보관하는 편이 낫다.
- 사용자가 명시적으로 "저장해줘"라고 하면, tool-calling 중에 즉시 실행되어야 한다.

## Design

### §1. 읽기는 tool, 쓰기는 이중 경로

```
┌─────────────────────────────────────────────────────────────┐
│                    Tool-calling 중 (실시간)                  │
│                                                             │
│  memory_read, memory_search        → tool (기억 recall)     │
│  knowledge (read/search/tree/...)  → tool (문서 검색)       │
│  knowledge (write)                 → tool (명시적 저장 요청) │
│  memory_write                      → ❌ 제거                 │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│                    실행 후 (비동기 hook)                     │
│                                                             │
│  PersistenceHook::evaluate()                                │
│    ├── 휴리스틱: 마크다운 문서 감지 → knowledge 자동 저장    │
│    ├── LLM reflection: 사실/선호 추출 → memory 저장          │
│    └── fire-and-forget (결과 반환을 막지 않음)               │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│                    Web UI (수동)                             │
│                                                             │
│  응답 버블 하단:                                             │
│    저장 안 됨 → [지식에 저장] 버튼                           │
│    저장 됨   → "저장됨" 인디케이터 + 노트 링크               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### §2. memory_write를 tool에서 제거

memory_write는 tool-calling 목록에서 제거한다. 이유:

- memory는 에이전트 내부 학습이다. 사용자가 "memory에 저장해줘"라고 지시할 일이 없다
- tool 목록에 있으면 LLM이 "저장" 요청에 memory_write를 선택지로 고려한다 (RFC-015 이전의 혼란 재발)
- memory 저장은 오직 hook의 LLM reflection만 수행

**제거 대상**:
- `builtin/mod.rs` — `register_all_kernel_tools()`에서 `MemoryWriteTool` 제거
- `kernel_bridge.rs` — `tool_names()`에서 `"memory_write"` 제거
- `memory_tools.rs` — `MemoryWriteTool` struct는 제거하지 않음 (hook에서 재사용 가능)

### §3. PersistenceHook — 휴리스틱 + Reflection 이층 구조

**원칙**: 비용이 싼 휴리스틱이 먼저 판단하고, 모호한 케이스만 LLM reflection에 넘긴다.

```
AgentLoop 완료
  │
  ├── 1차: 휴리스틱 (LLM 호출 없음, 비용 0)
  │     ├── 응답이 마크다운 문서인가? → knowledge 자동 저장
  │     └── 아니면 패스
  │
  ├── 2차: LLM Reflection (LLM 호출 1회)
  │     ├── trajectory에서 사용자 사실/선호 추출 → memory 저장
  │     ├── 휴리스틱이 놓친 knowledge 저장 기회 판단
  │     └── 아무것도 없으면 빈 계획 반환
  │
  └── 실행 (fire-and-forget)
        ├── MemoryManager::remember() × N
        └── KnowledgeBase::note_write() × N
```

#### 3-1. 휴리스틱: 마크다운 문서 감지

에이전트의 최종 응답이 마크다운 문서인지 판별한다. LLM 호출 없이 문자열 분석만으로 판단.

```rust
/// 마크다운 문서로 판단할 조건:
/// 1. 길이 > 300자
/// 2. 최소 1개 이상의 ## 헤더 포함
/// 3. 구조적 요소가 있음 (리스트, 코드블록, 테이블 중 하나)
fn looks_like_document(content: &str) -> bool {
    if content.len() < 300 {
        return false;
    }
    let has_headers = HEADER_REGEX.is_match(content);
    let has_structure = content.contains("- ") || content.contains("* ")
        || content.contains("```") || content.contains("| ");
    has_headers && has_structure
}
```

조건을 만족하면 knowledge에 자동 저장:

```rust
fn auto_save_path(seed: &Seed, content: &str) -> String {
    // seed.goal이나 original_request에서 카테고리 추론
    // 예: "Rust 설계 문서 작성" → "notes/rust-design-2026-06-13.md"
    // 예: "해커뉴스 분석" → "research/hackernews-2026-06-13.md"
    // 추론 실패 시 "notes/{slugified-first-heading}.md"
}
```

**휴리스틱만으로 충분한 케이스**:
- 설계 문서, 보고서, 분석 결과 → 길고 구조화된 마크다운
- 이 경우 LLM reflection은 knowledge 판단을 건너뛴다 (중복 방지)

#### 3-2. LLM Reflection: memory + knowledge 판단

휴리스틱이 처리하지 못한 케이스와 memory 추출을 담당.

**입력** (토큰 절약을 위해 컴팩트):
- `seed.goal` — 한 줄
- `seed.original_request` — 한 줄
- trajectory 요약 — 각 step을 1줄로 압축 (최대 20줄)
- 최종 응답의 앞부분 500자

**프롬프트**:

```
Review this agent execution. Decide what to persist.

Goal: {goal}
Request: {original_request}
Steps: {trajectory_summary}
Result: {result_snippet}

Two stores:
- Memory: facts about the user, preference corrections, project context.
  Not visible to the user. Agent's own learning.
- Knowledge: documents, research, reference material the user would want later.
  Visible via Web UI.

JSON only:
{"memory":[{"content":"...","type":"fact|episode","importance":0.0-1.0}],"knowledge":[{"path":"cat/file.md","content":"..."}]}
```

**비용 관리**:
- trajectory 요약은 각 step을 1줄로 압축하여 토큰 절약
- 응답은 500자로 잘라서 입력
- 출력은 JSON만 (설명 없음) — 출력 토큰 최소화
- `max_tokens: 512`로 제한
- 휴리스틱이 이미 knowledge를 저장한 경우, 프롬프트에서 knowledge 필드를 제외하고 memory만 판단 → 프롬프트가 짧아짐

**실패 처리**:
- LLM 호출 실패 → 로그만 남기고 패스
- JSON 파싱 실패 → 로그만 남기고 패스
- hook 전체가 실패해도 ExecutionResult 반환에 영향 없음
- **UI의 "지식에 저장" 버튼이 최종 안전망** — hook이 놓치면 사용자가 수동으로 저장

### §4. Hook 실행 조건과 중복 방지

hook은 **trajectory가 비어 있어도** 실행된다. 이유:

- 사용자가 "Rust 소유권 정리해줘" → 에이전트가 tool 없이 마크다운 문서를 직접 작성 → trajectory 비어있음 → 휴리스틱이 감지해야 할 가장 전형적인 케이스
- 사용자가 대화 중 "나는 Rust 좋아해" → 에이전트가 "네" → trajectory 비어있음 → 가장 중요한 memory(사용자 선호)가 날아감

hook이 실행되지 않는 유일한 조건은 **실행이 실패했을 때**뿐이다.

```rust
impl AgentRuntime {
    pub async fn execute(&self, agent_id: AgentId, seed: Seed) -> Result<ExecutionResult> {
        let result = run_agent(...).await?;

        // 자동 저장 hook — 성공한 실행에 대해 항상 실행
        if result.success {
            if let Some(hook) = &self.persistence_hook {
                let already_saved_knowledge = trajectory_has_knowledge_write(&trajectory);
                let hook = hook.clone();
                tokio::spawn(async move {
                    match hook.evaluate(&seed, &trajectory, &result.output, already_saved_knowledge).await {
                        Ok(plan) => hook.execute(plan).await,
                        Err(e) => tracing::warn!(error = %e, "PersistenceHook failed"),
                    }
                });
            }
        }

        Ok(result)
    }
}
```

중복 방지 (3레이어):
- **tool → hook**: tool-calling에서 이미 knowledge write했다면 hook은 knowledge를 건너뛴다
- **hook → UI**: hook이 저장을 완료하면 SSE로 UI에 알리고, StateStore에 기록. POST /save-to-knowledge는 저장 전에 StateStore를 확인하여 중복 차단
- **UI → hook**: 사용자가 버튼을 누른 직후 낙관적 업데이트로 "저장됨"으로 전환. 이후 hook의 SSE가 와도 이미 저장됨 상태이므로 무시

### §5. 메시지 저장 상태 추적

hook이 저장하면, 어느 메시지가 어디에 저장되었는지 추적해야 UI가 "저장됨"을 표시할 수 있다.

**저장소**: StateStore에 세션별 매핑 파일

```
~/.oxios/workspace/knowledge-saves/{session_id}.json
```

```json
[
  {
    "message_index": 5,
    "knowledge_path": "research/hackernews-2026-06-13.md",
    "saved_at": "2026-06-13T10:30:00Z",
    "source": "hook"
  },
  {
    "message_index": 8,
    "knowledge_path": "notes/rust-design.md",
    "saved_at": "2026-06-13T11:00:00Z",
    "source": "user"
  }
]
```

**source 필드**:
- `"hook"` — 에이전트가 자율 판단으로 저장
- `"user"` — 사용자가 UI 버튼으로 저장
- `"tool"` — tool-calling 중에 knowledge write 수행

**hook이 저장할 때**:

```rust
// 1. KnowledgeBase에 노트 작성
self.knowledge_base.note_write(&path, &content)?;

// 2. StateStore에 매핑 기록
self.state_store.save_json(
    "knowledge-saves",
    &session_id,
    &updated_saves,
)?;

// 3. EventBus에 이벤트 발행
self.event_bus.publish(KernelEvent::KnowledgePersisted {
    session_id,
    message_index,
    path,
    source,
})?;
```

### §6. Backend API

#### GET `/api/chat/{session_id}/knowledge-saves`

세션의 메시지 저장 상태를 반환.

```json
{
  "saves": [
    {"message_index": 5, "path": "research/hackernews-2026-06-13.md"},
    {"message_index": 8, "path": "notes/rust-design.md"}
  ]
}
```

세션 로딩 시 호출하여 각 메시지 버블의 초기 상태를 결정.

#### POST `/api/chat/{session_id}/messages/{message_index}/save-to-knowledge`

사용자가 "지식에 저장" 버튼을 눌렀을 때 호출.

```json
// Request
{
  "path": "notes/optional-path-hint.md"  // 선택사항
}

// Response — 저장 성공
{ "path": "notes/rust-design-2026-06-13.md" }

// Response — 이미 저장됨 (중복 요청)
{ "error": "already_saved", "path": "notes/rust-design-2026-06-13.md" }
```

서버에서:
1. StateStore에서 해당 message_index의 저장 여부 확인 → 이미 저장되었으면 기존 경로 반환
2. 해당 메시지 내용을 조회
3. KnowledgeBase에 저장 (path가 없으면 자동 생성)
4. StateStore에 매핑 기록
5. EventBus에 이벤트 발행 → 다른 클라이언트에도 실시간 반영

#### DELETE `/api/chat/{session_id}/messages/{message_index}/knowledge-save`

사용자가 이미 저장된 버블의 인디케이터를 클릭했을 때 호출.

```json
// Response
{ "deleted_path": "notes/rust-design-2026-06-13.md" }
```

서버에서:
1. StateStore에서 해당 매핑 조회
2. KnowledgeBase에서 노트 삭제
3. StateStore에서 매핑 제거
4. EventBus에 이벤트 발행 → UI가 "지식에 저장" 버튼으로 복원

#### SSE 이벤트

```
event: knowledge_persisted
data: {"session_id":"...","message_index":5,"path":"research/hackernews.md","source":"hook"}
```

hook이 비동기로 저장을 완료하면 이 이벤트가 날아가고, 열려 있는 Web UI의 해당 메시지 버블이 "저장됨"으로 전환.

```
event: knowledge_removed
data: {"session_id":"...","message_index":5}
```

사용자가 삭제하면 이 이벤트가 날아가고, 다른 클라이언트의 해당 메시지 버블이 "지식에 저장" 버튼으로 복원.

### §7. UI: 메시지 버블 토글 인디케이터

**위치**: `surface/oxios-web/web/src/components/chat/message-bubble.tsx` 하단

인디케이터는 **토글**이다. 저장 ↔ 삭제가 하나의 버튼으로 동작한다.

```
┌──────────────────────────────────────────────────┐
│  🤖 에이전트 응답                                 │
│                                                  │
│  ## 해커뉴스 베스트 3건                           │
│  1. Show HN: Rust 런타임 ...                     │
│  2. Why SQLite is enough ...                     │
│  3. The Art of System Design ...                 │
│                                                  │
│  ─────────────────────────────────────────────── │
│  📄 저장됨 · research/hackernews-2026-06-13.md   │  ← 클릭 → "삭제하시겠습니까?"
└──────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────┐
│  🤖 에이전트 응답                                 │
│  Rust의 소유권 시스템은...                         │
│  ─────────────────────────────────────────────── │
│  📝 [지식에 저장]                                 │  ← 클릭 → 즉시 저장
└──────────────────────────────────────────────────┘
```

**상태 머신**:

```
                ┌──────────┐
     초기:      │ 미저장    │
                │ [저장]    │
                └────┬─────┘
                     │
         ┌───────────┼───────────┐
         │           │           │
    사용자 클릭  hook SSE    tool-calling
    POST 저장     수신        중 저장
         │           │           │
         └───────────┼───────────┘
                     │
                     ▼
                ┌──────────┐
                │ 저장됨    │
                │ 📄 path   │
                └────┬─────┘
                     │
              사용자 클릭
                     │
                     ▼
              ┌─────────────┐
              │ "이 노트를   │
              │  삭제하시겠  │
              │  습니까?"   │
              │ [취소][삭제] │
              └──┬──────┬───┘
                 │      │
              취소    삭제 확인
                 │      │
                 │      ▼
                 │  ┌──────────┐
                 │  │ 미저장    │
                 └─▶│ [저장]    │
                    └──────────┘
```

**동작**:

| 현재 상태 | 사용자 행동 | 결과 |
|-----------|------------|------|
| 미저장 | [지식에 저장] 클릭 | POST 저장 → "저장됨" 전환 |
| 저장됨 | 📄 인디케이터 클릭 | 확인 다이얼로그 표시 |
| 저장됨 → 다이얼로그 | 삭제 클릭 | DELETE 요청 → "미저장" 복원 |
| 저장됨 → 다이얼로그 | 취소 클릭 | 그대로 유지 |

**표시 조건**:
- 에이전트 응답(message role = assistant)에만 표시
- 사용자 메시지, 시스템 메시지에는 표시하지 않음
- 짧은 응답에도 인디케이터는 항상 표시 — 사용자가 판단

### §8. 모듈 구조

```
crates/oxios-kernel/src/
├── persistence_hook.rs          ← 신규: PersistenceHook, PersistencePlan
│                                 휴리스틱 + LLM reflection
├── agent_runtime.rs             ← 수정: hook 필드, execute()에 hook 호출
│                                 memory_write tool 제거
├── tools/
│   ├── builtin/mod.rs           ← 수정: MemoryWriteTool 등록 제거
│   ├── kernel_bridge.rs         ← 수정: tool_names에서 memory_write 제거
│   └── memory_tools.rs          ← MemoryWriteTool struct 유지 (hook에서 사용)
└── ...

surface/oxios-web/
├── src/routes/chat.tsx                  ← 수정: knowledge-saves 상태 관리
├── src/components/chat/
│   ├── message-bubble.tsx               ← 수정: 저장 인디케이터 추가
│   └── knowledge-save-indicator.tsx     ← 신규: 저장 상태 컴포넌트
├── src/hooks/
│   └── use-knowledge-saves.ts           ← 신규: 세션별 저장 상태 훅
└── src/stores/
    └── chat.ts                          ← 수정: SSE 이벤트 처리

src/kernel.rs                   ← 수정: KernelBuilder에 PersistenceHook 조립
```

## Implementation Order

```
① persistence_hook.rs          → 신규 모듈
                                  휴리스틱 (looks_like_document)
                                  LLM reflection 프롬프트 + JSON 파싱
                                  PersistencePlan, execute()

② agent_runtime.rs             → PersistenceHook 필드 추가
                                  execute()에 hook 호출
                                  memory_write tool 등록 제거

③ builtin/mod.rs, kernel_bridge.rs
                                → MemoryWriteTool 등록 제거

④ kernel.rs (binary crate)     → KernelBuilder에 PersistenceHook 조립
                                  (MemoryManager + KnowledgeBase + EngineHandle)

⑤ StateStore                   → knowledge-saves 카테고리 저장/조회

⑥ Backend API                  → GET /knowledge-saves
                                  POST /save-to-knowledge
                                  SSE 이벤트

⑦ Web UI                       → knowledge-save-indicator 컴포넌트
                                  message-bubble에 통합
                                  use-knowledge-saves 훅
                                  SSE 이벤트 핸들링

⑧ 테스트                        → 휴리스틱 단위 테스트
                                  reflection 파싱 테스트
                                  중복 방지 테스트
                                  API 엔드포인트 테스트
```

## What This Does NOT Do

- **memory_read, memory_search를 변경하지 않음** — 읽기 도구는 그대로. 에이전트가 실행 중에 기억을 recall하는 데 필요.
- **knowledge tool을 제거하지 않음** — 사용자가 명시적으로 "저장해줘"라고 하면 tool-calling 중에 즉시 실행.
- **Dream process를 대체하지 않음** — RFC-008의 Dream은 장기 기억 통합. hook은 즉각적인 판단. 다른 레이어.
- **모든 응답을 저장하지 않음** — 휴리스틱 + reflection이 가치 있다고 판단한 것만.
- **저장됨 인디케이터 클릭으로 에디터를 열지 않음** — 이후 작업. 현재는 삭제 토글만.
