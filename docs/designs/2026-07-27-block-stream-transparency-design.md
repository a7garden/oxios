# Block-Stream Transparency — Design

> Status: Design (advisor-reviewed, revised)
> Date: 2026-07-27
> Scope: Web UI chat transparency — render the agent's flow of thought
> (reason → tool → reason → tool → answer) as an interleaved timeline,
> replacing the current categorized LobeHub-chat layout.
> Related: `docs/designs/2026-07-21-lobehub-chat-port-design.md`, RFC-015.

## 1. Problem

The agent streams reasoning and tool calls **interleaved in time**, but the UI
renders them **categorized**, not temporally:

```
Current render (AssistantMessage.tsx:92-117):
  Thinking (ALL reasoning, one concatenated string)   ← top
  Search / Chunks
  MarkdownMessage (the answer)
  ToolCallList (ALL tools, as a list)                  ← below the answer
```

Four parallel representations exist on `ChatMessage`, and the renderer picks
the categorized ones and discards the ordered one:

| Field | Meaning | Used in render? |
|---|---|---|
| `content` | answer text | yes |
| `reasoning.content` | ALL reasoning concatenated into one string (position lost) | yes (`Thinking`) |
| `toolCalls[]` | structured tool list | yes (`ToolCallList`) |
| `activities[]` | tool + reasoning + usage in **arrival order** | **no** (only `LiveActivityBar` reads it; `ActivityTimeline` is orphaned — 0 usages) |

Symptoms:
- Reasoning is one block at the top regardless of when each span occurred.
- Tools render as a list **below** the answer — execution order is inverted.
- `activities[]` (the timeline) is built then thrown away in the message body.

This fits a chat assistant (ChatGPT/Claude.ai) but not an **Agent OS** where
the execution trace is primary content.

## 2. Goals / Non-Goals

Goals:
- Render the turn as an ordered block stream: reasoning segments and tool
  cards interleaved in execution order, answer as the terminal text block(s).
- Single source of truth for a turn's content (no dual representation).
- Reasoning position survives session reopen (not only live).
- Reuse existing rich tool renderers (`tool-renders/registry.tsx`).

Non-goals:
- Changing the streaming transport (WS/SSE) or the backend event vocabulary.
- Redesigning input, sidebar, or non-chat surfaces.
- Multi-agent / group chat rendering.

## 3. Data Model

### 3.1 Frontend: `ChatBlock`

A message is an ordered array of blocks. New type in `types/chat.ts`:

```ts
export type ChatBlock = ReasoningBlock | ToolBlock | TextBlock

interface ReasoningBlock {
  type: 'reasoning'
  id: string                 // stable: `r-${msgId}-${reasoningSeq}` (see §3.3)
  text: string
  status: 'streaming' | 'done'
  source?: 'thinking' | 'compaction'   // compaction reasoning is tagged (agent_runtime ~1345)
  startedAt: number
  durationMs?: number        // consumed by <Thinking>'s "· Xs" timer
}

interface ToolBlock {
  type: 'tool'
  id: string                 // = tool_call_id (server-stable merge key)
  name: string
  args?: unknown
  status: ChatToolStatus     // 'loading' | 'success' | 'error' | 'aborted'
  result?: unknown
  outputSummary?: string
  progress?: string
  tabId?: string
  context?: BrowseContext
  durationMs?: number
}

interface TextBlock {
  type: 'text'
  id: string                 // stable: `t-${msgId}-${textSeq}`
  text: string
  streaming?: boolean
}
```

`ChatMessage` gains `blocks?: ChatBlock[]` as the single source of truth.
Legacy `content` / `reasoning` / `toolCalls` / `activities` are **derived from
`blocks`** during transition (so `LiveActivityBar`, quick-ask, and any other
`activities[]` consumer keep working), then removed once no consumer reads them.

Invariants:
- `blocks` is append-mostly, ordered by arrival.
- A turn may contain ≥0 reasoning blocks, ≥0 tool blocks, ≥1 text block. Simple
  Q&A ⇒ `blocks = [text]`. Tool-using turn ⇒ `[reasoning?, tools…, reasoning?,
  text]`. Mid-turn preamble text (`text → tool → text`) yields multiple text
  blocks — that is correct and preserved.
- The **terminal answer** = the last text block. `content` (persisted answer,
  for search/summary) = concatenation of all text blocks in order.
- Tool blocks keyed by `tool_call_id` ⇒ `progress` / `end` merge in place
  (same rule as today's `mergeOrAppendActivity`).

### 3.2 Backend: unified ordered trace

Today the runtime captures two parallel structures:
- `trajectory_steps: Vec<TrajectoryStep>` (tool steps, ordered) — SONA learning.
- `reasoning_text: String` (concatenated, capped 4 KB) — persisted as `ReasoningRecord`.

The concatenation is what destroys reasoning position on reopen. Unify into
one ordered trace that mirrors `blocks` (minus transient streaming flags):

```rust
enum TraceStep {
    Reasoning { text: String, source: ReasoningSource, duration_ms: u64 },
    Tool      { /* existing TrajectoryStep fields: input/output/duration/confidence */ },
    Text      { text: String },   // preamble or terminal answer text, positioned
}
// Vec<TraceStep>, built in arrival order from the streaming callback.
```

- `agent_runtime.rs` streaming callback: `ThinkingDelta` → append to the open
  `Reasoning` trace step (open a new one if the last isn't Reasoning);
  `ToolExecutionEnd` → push a `Tool` step; assistant text deltas → push/append
  a `Text` step. (Replaces the `reasoning_text` accumulator at ~line 1423.)
- `ReasoningFragment { source: "compaction" }` (published at ~1345) → a
  `Reasoning` step with `source = Compaction`.
- SONA consumes `trace.iter().filter(Tool)` — unchanged data shape.
- Persistence stores `trace` (ordered). `ReasoningRecord` and the flat
  `content` become **derived** caches (or are dropped).
- Frontend reopen: `blocks = trace.map(to_block)` — verbatim, no
  re-segmentation. **Live and reopened renders are identical** (the
  contradiction in the prior draft is resolved by making `Text` a trace step).

### 3.3 Block-id stability contract

Array-index ids break React reconciliation (insert-invalidation ⇒ churn, focus
loss, scroll jumps). All ids are stable and assigned at the moment a block
**opens**, via per-message monotonic counters:

| Block | id | assigned at |
|---|---|---|
| Reasoning | `r-${msgId}-${reasoningSeq}` | `reasoning.start` (counter++) |
| Tool | `tool_call_id` (server-stable) | `tool.start` |
| Text | `t-${msgId}-${textSeq}` | first `text.delta` of a new text block |

Counters are deterministic and survive rAF batching (the batch flush applies
patches whose ids were fixed when the event arrived). Missing `tool_call_id` ⇒
fallback id (existing `cryptoFallbackId`).

## 4. Components

`AssistantMessage` pipeline (`Thinking → ... → MarkdownMessage → ToolCallList`)
is replaced by one component:

```tsx
<BlockStream blocks={message.blocks}>
  reasoning → <ReasoningBlock/>   // per-segment collapsible.
                                  //   streaming: "추론 중 · Xs", auto-expand.
                                  //   done: auto-collapse (expand on hover/click).
                                  //   Reuses <Thinking> per segment
                                  //   (content/thinking/duration props).
  tool      → <ToolCard/>         // reuses tool-renders/registry.tsx
                                  //   (Bash/Grep/WebSearch/…). Positioned in-stream.
  text      → <TextBlock/>        // thin wrapper over <MarkdownMessage>, streamed.
</BlockStream>
```

- **`LiveActivityBar` stays**, now deriving from the trailing block: last
  reasoning streaming ⇒ "추론 중"; tool loading ⇒ "도구 실행 중"; text
  streaming ⇒ "응답 작성 중".
- **Tool-approval / path-access / ask_user** are overlay cards, **not blocks**
  — they pause the stream; the pending tool block shows `status: loading` and
  the approval card renders via its existing mechanism. On resume, `tool.end`
  arrives normally.
- **`ActivityTimeline` (orphaned)** is retired — superseded by `BlockStream`.
  Before deletion (P3), confirm *why* it was orphaned (likely the 2026-07-21
  LobeHub port replaced it and never re-wired it) and audit all `activities[]`
  call sites (LiveActivityBar, quick-ask, hydrate path, tests) — those consume
  `activities[]` derived from `blocks` during transition.

## 5. Data Flow

```
WS chunk
 → adaptChunk → ChatEvent (text.delta | reasoning.start/delta/end
                            | tool.start/progress/end | stream.stop)
 → StreamProcessor.apply(event) → { blockPatch }
      reasoning.start → append ReasoningBlock{ id=r-${msgId}-${++rSeq}, status:'streaming' }
      reasoning.delta  → append text to the LAST reasoning block
                         (only if last.type==='reasoning' && last.status==='streaming')
      reasoning.end    → mark that block done + durationMs
      tool.start       → append ToolBlock{ status:'loading' } keyed by tool_call_id
                         (merge into existing block if id already present)
      tool.progress    → merge progress into block by id
      tool.end         → merge result/status/duration by id
      text.delta       → if last block is Text: append to it;
                         else: append new TextBlock{ id=t-${msgId}-${++tSeq}, streaming:true }
      stream.stop      → text.streaming=false; all reasoning → done.
                         TERMINAL: subsequent blocks are ignored.
 → store merges patch into message.blocks[]
 → BlockStream re-renders incrementally
```

- `StreamProcessor` drops the dual emit (first-class fields + activity
  side-channel) and emits block patches only.
- **F9 rAF token batching stays** — `text.delta` / `reasoning.delta` coalesce
  before flush; ids were fixed at event arrival so batching is safe.
- **Text-block predicate** (explicit): a new `TextBlock` opens iff the last
  block is not `Text`. Contiguous deltas accumulate; any non-text block between
  starts a fresh text block.

## 6. Edge Cases

- **Mid-stream error** (`stream.stop reason:'error'`) — close open blocks;
  render `ErrorCard` at the end.
- **Tool-execution error vs stream error** — `tool.end { is_error }` marks the
  ToolBlock `error` (the tool ran, failed); `stream.stop error` means the stream
  died. Different recovery, both handled.
- **Abort / cancel** — flush, mark blocks non-streaming. Ghost placeholder (no
  blocks) dropped (existing `finaliseStreamingMessage`).
- **Auto-retry** — replaces the in-flight assistant message **in place by id**;
  old `blocks` discarded (failed turn, no persistent value). The rAF flush is
  guarded by a message-id check so a stale closure cannot ghost-render the
  superseded message.
- **Reconnect (RFC-024)** — terminal `done` carries full text + persisted
  trace; rebuild `blocks = trace`. Live reasoning during the disconnect gap is
  lost (same as today); structure preserved.
- **Reasoning overflow** — total reasoning per turn is capped at a budget
  (16 KB) across all reasoning segments; on overflow, stop appending further
  reasoning deltas and append a truncation marker to the last reasoning block.
  Bounds persisted trace size. Tool results are already truncated server-side
  (`max_tool_result_bytes`).
- **Parallel tools** — events are serialized on the wire; each `tool.start`
  appends its own ToolBlock in arrival order (best-effort, not model-declared
  order). `progress`/`end` merge by `tool_call_id`, so concurrent tools update
  independently.
- **Compaction mid-turn** — emitted as a `Reasoning` step/block with
  `source: 'compaction'`; renders inline at its position (distinguishable styling).
- **Edit / regenerate an old turn** — produces a **new** ChatMessage (new id);
  history is never mutated in place.
- **Blocks after `stream.stop`** — ignored (terminal).

## 7. Migration & Backward Compatibility

- **Load (`hydrateBlocks(msg)`)** — if `blocks` absent, synthesize from legacy:
  1. `activities[]` (already ordered) → blocks.
  2. Append `content` as a trailing `TextBlock` (if not already covered).
  3. No `activities` (very old): `[reasoning?, ...toolCalls, text]` ⇒ degrades
     to roughly today's layout.
- **Persist** — new turns write `trace`. Legacy fields derived during
  transition; removed in P3 once the read path is blocks-only.
- **Round-trip** — `hydrate(legacy) → blocks → persist trace → hydrate` is
  stable for the structured part. Reasoning position may flatten for pre-trace
  (very old) messages; acceptable.

## 8. Testing

- **Store reducer** — events ⇒ `blocks[]` (append + merge by id; multi-text
  predicate; id stability across rAF). Port `stores.test.ts` cases to assert on
  `blocks`.
- **`hydrateBlocks`** — legacy ⇒ expected block order; round-trip stability.
- **`BlockStream` render** — N blocks in order; reasoning collapse states; tool
  lifecycle (loading → success/error); compaction-source styling.
- **Backend** — `agent_runtime` builds `trace` with reasoning/tool/text
  interleaved in arrival order; overflow truncation fires at budget.

## 9. Phasing

- **Phase 1+2 — ship atomically** (same event-shape change, two endpoints):
  frontend `ChatBlock` + `BlockStream` + `StreamProcessor` rewrite +
  `hydrateBlocks`, **and** backend `TraceStep` capture + persistence. Shipping
  frontend alone would regress reopen ("looked different yesterday"); the two
  land together behind a feature flag enabled only when both are in.
- **Phase 3 — legacy removal.** Drop `content`/`reasoning`/`toolCalls`/
  `activities` after auditing all call sites. Retire `ActivityTimeline`.

## 10. Deferred (YAGNI)

- `collapseAfter` / long-trace virtualization — add when a real trace exceeds
  tolerable height.
- Declared (model-declared) tool ordering — best-effort arrival order suffices.
- A dedicated `Notice`/`System` block type — compaction is covered by
  `source` on ReasoningBlock; revisit if other synthetic content appears.

## 11. Decisions (locked, post-review)

1. `blocks[]` is the single source of truth; legacy fields derived then removed.
2. Reasoning is modeled as positioned segments (not one concatenated string);
   `source` distinguishes thinking vs compaction.
3. One renderer (`BlockStream`) replaces `Thinking` + `ToolCallList`.
4. Backend `trajectory` + `reasoning_text` unify into one ordered `TraceStep`
   trace that **includes a `Text` variant**, so reopen is verbatim and identical
   to live.
5. Block ids are counter-assigned at open (stable, never array-index); tool id
   = `tool_call_id`.
6. `content` = concat of all text blocks; terminal answer = last text block.
7. Text-block predicate: new iff last block is not Text.
8. Reasoning overflow: total 16 KB budget + truncation marker.
9. Auto-retry: in-place replace by id, discard old blocks, rAF guarded by id.
10. `LiveActivityBar` stays; approval/path-access stay overlay cards;
    `ActivityTimeline` retired (after orphan-cause + call-site audit in P3).
11. Phase 1+2 ship atomically; Phase 3 is legacy cleanup.
