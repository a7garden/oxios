# RFC-016: Questionnaire Tool

> **Status**: Draft
> **Date**: 2025-06-07
> **Replaces**: Ouroboros `interview_structured()` (partial)

## Problem

The current interview system asks the Orchestrator to call the LLM **twice** per
clarification round (`interview()` + `interview_structured()`). When the second
call fails (JSON parse error, model refuses structured output), the Web UI falls
back to plain text — the interactive widgets disappear.

Worse, the Orchestrator *guesses* what questions to ask. The agent (which
understands the full context) has no say in what it needs clarified.

## Proposal

Make **questionnaire** a first-class **kernel tool** — just like `exec` or
`memory_read`. The agent calls it when it decides it needs clarification. The
kernel handles channel-appropriate rendering and collects the response.

```
Agent decides "I need clarification"
  → tool_call: questionnaire({ questions: [...] })
  → kernel emits KernelEvent::QuestionnaireRequested
  → channel renders natively:
      Web      → interactive widgets (chips, buttons, free text)
      Telegram → inline keyboard
      CLI      → numbered list + stdin
  → user answers
  → tool returns structured result to agent
  → agent continues
```

## Channel Rendering

| Channel | single_choice | multi_choice | yes_no | free_text |
|---------|:---:|:---:|:---:|:---:|
| **Web** | chip buttons | toggleable chips | Yes/No buttons | textarea |
| **Telegram** | InlineKeyboard (one per row) | Sequential messages | InlineKeyboard ✓/✗ | Reply text |
| **CLI** | Numbered list → stdin number | Numbered list → comma-separated | y/n prompt | stdin text |

## Data Model

### Question (tool input)

```rust
struct Question {
    id: String,          // e.g. "q1"
    prompt: String,      // "Which framework?"
    kind: QuestionKind,  // single_choice | multi_choice | yes_no | free_text
    options: Vec<Option>,
    allow_other: bool,   // default true
}

struct Option {
    value: String,
    label: String,
    description: Option<String>,
}

enum QuestionKind {
    SingleChoice,
    MultiChoice,
    YesNo,
    FreeText,
}
```

### Answer (tool output)

```rust
struct Answer {
    question_id: String,
    values: Vec<String>,   // selected values (single for single_choice)
    was_custom: bool,      // true when user typed a custom answer
}

struct QuestionnaireResult {
    answers: Vec<Answer>,
    cancelled: bool,
}
```

## Kernel Architecture

```
questionnaire_tool.rs
  ┌──────────────────────┐
  │  AgentTool impl      │
  │  execute(args)       │
  │    1. Parse questions│
  │    2. Create pending │
  │    3. Emit event     │
  │    4. Await response │
  │    5. Return result  │
  └──────────────────────┘
         │                    KernelEvent::QuestionnaireRequested
         ▼
  ┌──────────────────────┐
  │  event_bus           │
  │  broadcast to all    │
  │  subscribers         │
  └──────────────────────┘
         │
         ▼
  ┌──────────────────────┐
  │  channel (web/tg/cli)│
  │  renders UI          │
  │  collects answer     │
  │  resolves pending    │
  └──────────────────────┘
```

The pending questionnaire uses `tokio::sync::oneshot` — identical pattern to
the existing approval system.

## Web UI Design (Claude-like)

The questionnaire appears inline in the chat flow, replacing the old
`InterviewResponse` component. Key differences from the current UI:

1. **Part of the tool call timeline** — shows as a tool_call card with embedded
   interactive widgets, not a separate overlay.
2. **Chat input remains visible** — the user can still type (for free_text
   questions or general comments alongside the structured answers).
3. **Claude-style** — clean card with question text, option chips, submit button.

```
┌─────────────────────────────────────────────────┐
│ 🔧 questionnaire                    3 questions │
├─────────────────────────────────────────────────┤
│                                                 │
│ 1. 상위 스토리를 원하시나요?                     │
│   [Top stories]  [Newest]  [Best]               │
│                                                 │
│ 2. 어떤 정보가 필요하신가요?                     │
│   ☐ 제목  ☐ 링크  ☐ 점수  ☐ 댓글 수             │
│                                                 │
│ 3. 결과 형식은?                                  │
│   [한국어 요약]  [원문]  [JSON]                   │
│                                                 │
│ ────────────────────────────────────            │
│ 추가 의견 (선택):                                │
│ ┌───────────────────────────────────┐           │
│ └───────────────────────────────────┘           │
│                                    [제출 →]     │
└─────────────────────────────────────────────────┘
```

## Implementation Plan

### Phase 1: Kernel tool + Web UI

1. **`questionnaire_tool.rs`** — new kernel tool with oneshot await
2. **`KernelEvent::QuestionnaireRequested`** — new event variant
3. **`KernelEvent::QuestionnaireResolved`** — resolved event
4. **`QuestionnaireManager`** — stores pending questionnaires, resolves them
5. **Register in `kernel_bridge.rs`** — as always-on tool
6. **Web route `chat.rs`** — catch the event, send WS chunk, handle response
7. **Frontend `QuestionnaireCard`** — new component replacing `InterviewResponse`
8. **Frontend chat store** — handle `questionnaire` / `questionnaire_response` chunks

### Phase 2: Ouroboros integration

- Remove `interview_structured()` — agent calls `questionnaire` tool instead
- Simplify `interview()` to just score ambiguity, no question generation
- Orchestrator still runs interview → seed → execute pipeline, but questions
  come from the agent's tool calls, not from a separate LLM invocation

### Phase 3: Telegram + CLI

- Telegram: `InlineKeyboardMarkup` for single_choice / yes_no
- CLI: numbered text + stdin read

## Migration

The existing `interview` WS chunk and `InterviewResponse` component remain
functional until Phase 2 is complete. The new `questionnaire` tool runs in
parallel — agents can use either path. Once Phase 2 lands, the old path is
removed.

## Telegram Inline Keyboard

Telegram Bot API supports `InlineKeyboardMarkup` natively:

```json
{
  "reply_markup": {
    "inline_keyboard": [
      [{"text": "Top stories", "callback_data": "q1:top"}],
      [{"text": "Newest", "callback_data": "q1:newest"}],
      [{"text": "Best", "callback_data": "q1:best"}]
    ]
  }
}
```

For multi_choice, send multiple key=value pairs via callback.
For free_text, fall back to regular text reply.

## CLI Fallback

```
1. 상위 스토리를 원하시나요?
   1) Top stories  2) Newest  3) Best
   > 1

2. 어떤 정보가 필요하신가요? (comma-separated)
   1) 제목  2) 링크  3) 점수  4) 댓글 수
   > 1,3

3. 결과 형식은?
   1) 한국어 요약  2) 원문  3) JSON
   > 1
```
