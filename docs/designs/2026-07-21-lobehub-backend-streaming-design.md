# LobeHub-Aligned Backend Streaming — Design Document

> **Date:** 2026-07-21
> **Companion to:** `docs/designs/2026-07-21-lobehub-chat-port-design.md` (frontend port, shipped)
> **Scope:** Backend (Rust) changes needed to close the streaming-UX gap with LobeHub. Frontend already shipped via Phases 1–6 of the frontend design.
> **Source:** Verified against oxi-sdk 0.56.0 + oxios-kernel + oxios-gateway. Three read-only scouts + manual code verification.

---

## 0. TL;DR — Honest Scope

The frontend port delivered ~60–70% of LobeHub's chat UX using only frontend changes. Closing the remaining gap requires backend work, but **not all of it is feasible**. Classified by feasibility:

| Change | Feasibility | Cost | Impact |
|---|---|---|---|
| **A. MessageId per chunk** | ✅ Kernel-only | Low | Enables concurrent streams, removes "last assistant" heuristic |
| **B. Reasoning start/end split** | ✅ Kernel-only (half already exists in oxi-sdk) | Medium | Smooth Thinking block animation |
| **C. Tool args streaming** (`tool_call_delta`) | ❌ **Blocked on oxi-sdk upstream** | — | Live tool construction UX |
| **D. Tool intervention manifest + async approval** | ✅ Kernel-only (RFC-017 designed, not wired) | Medium-High | Custom per-tool approval UI |
| **E. Grounding chunk** | ✅ Kernel-only (data already exists in tool result) | Low | Web search results as separate panel |
| **F. Live usage counter** | ⚠️ Partial — needs oxi-sdk to expose per-token estimate | Low-Medium | Live token counter |

**Recommendation:** ship A + B + E in one PR (high impact, low risk, no upstream dep). Defer D (more invasive). Track C and F as upstream-blocked.

---

## 1. What the Frontend Already Compensates For

The shipped frontend adapter (`web/src/lib/stream/adapter.ts`) **synthesizes** the missing events:

- `messageId` — frontend uses `lastAssistantMessageId(messages)` as implicit target. Works for single-stream Phase 1–6.
- `reasoning.start` — inferred from first `reasoning.delta` seen by StreamProcessor.
- `reasoning.end` — synthesized on `done` chunk.
- `grounding` — never emitted; search results stay embedded in `tool_end` payload.

These workarounds are correct for single-stream single-user chats. They break down for:
- Concurrent streams (multiple agents, background tasks) — `messageId` collision.
- Long reasoning spans — synthesized `reasoning.end` fires too late (only at `done`).
- Rich search UX — citations buried inside tool JSON.

---

## 2. Current Pipeline (Verified)

```
oxi-sdk AgentEvent callback
    │
    ▼
agent_runtime.rs::run_streaming()  (crates/oxios-kernel/src/agent_runtime.rs:1080+)
    │  match event {
    │    AgentEvent::ToolExecutionStart { args: Value, .. } → KernelEvent::ToolExecutionStarted
    │    AgentEvent::ToolExecutionUpdate { partial_result, .. } → KernelEvent::ToolExecutionUpdate
    │    AgentEvent::ToolExecutionEnd { .. } → KernelEvent::ToolExecutionFinished
    │    AgentEvent::Text { text } → StreamDelta::Text(text)         [mpsc to gateway]
    │    AgentEvent::Thinking → StreamDelta::Thinking                [start signal, no payload]
    │    AgentEvent::ThinkingDelta { text } → StreamDelta::ThinkingDelta(text)  [batched ~50ms]
    │    AgentEvent::TokenUsage { .. } → KernelEvent::TokenUsageUpdate
    │    AgentEvent::AgentEnd { .. } → terminal
    │  }
    │
    ├─→ KernelEvent (event bus) ──→ chat.rs kernel_event_to_ws_chunk() ──→ WS JSON chunk
    │
    └─→ StreamDelta (mpsc) ──→ gateway collector ──→ partial OutgoingMessage ──→ chat.rs WS handler
```

**Key files:**
- `crates/oxios-gateway/src/message.rs:124` — `OutgoingMessage` struct (has `id: Uuid` field).
- `crates/oxios-kernel/src/agent_runtime.rs:1080–1376` — `AgentEvent` match arms.
- `crates/oxios-gateway/src/gateway.rs` — collector that turns `StreamDelta` into `OutgoingMessage` partials.
- `src/api/routes/chat.rs:770–840` — WS handler translating OutgoingMessage to JSON chunks.
- `src/api/routes/chat.rs:1409–1534` — `kernel_event_to_ws_chunk` for tool/usage/memory/reasoning events.
- `crates/oxios-kernel/src/tools/gated_tool.rs:96–160` — synchronous access check (no async approval path).
- `crates/oxios-kernel/src/tools/pending_tool_approvals.rs` — oneshot registry (exists, **not wired** to GatedTool).

---

## 3. Phase A — MessageId Per Chunk (Low Risk, High Value)

### Problem

`OutgoingMessage.id` exists (`message.rs:126`) but the WS handler doesn't propagate it to chunk JSON. The frontend's `lastAssistantMessageId()` heuristic works only for single-stream chats.

### Change

**`src/api/routes/chat.rs`** — every chunk JSON built in the WS handler (`handle_chat_websocket`, ~lines 770–840) and in `kernel_event_to_ws_chunk` (~1409–1534) adds:

```rust
let chunk = serde_json::json!({
    "type": "...",
    "message_id": msg.id,                // ← NEW
    "seq": msg.seq,
    "content": msg.content,
    // ...rest unchanged
});
```

For KernelEvent path: the event already carries `session_id`. We need to also stamp `message_id` on KernelEvent variants or look it up via the session's active assistant message id.

**Two implementation options:**
1. **Thread-through**: add `message_id: Option<Uuid>` to relevant `KernelEvent` variants (`ToolExecutionStarted`, `ToolExecutionFinished`, `TokenUsageUpdate`, `ReasoningFragment`). Set it in `agent_runtime.rs` where the event is published.
2. **Lookup**: WS handler maintains `HashMap<session_id, Uuid>` mapping the current session to its active assistant message id. Updated when the first chunk for a new turn arrives.

Option 1 is cleaner (no shared state). Option 2 is less invasive. **Recommend option 1.**

### Frontend Impact

`web/src/lib/stream/adapter.ts` already accepts `ctx.msgId` — replace `lastAssistantMessageId()` heuristic with `chunk.message_id` from the wire. ~5 line change.

### Acceptance

- Backend test: `kernel_event_to_ws_chunk` includes `message_id` field for all chunk types.
- Integration: two simultaneous agent runs in same session produce chunks with distinct `message_id` values; frontend routes each to the correct message.

---

## 4. Phase B — Reasoning Start/End Split (Medium, Half Already Exists)

### What oxi-sdk Already Gives Us

Verified at `agent_runtime.rs`:
- **Line 1338**: `AgentEvent::Thinking` — signal-only, fires when model enters extended thinking. **This IS `reasoning.start`.**
- **Line 1349**: `AgentEvent::ThinkingDelta { text }` — batched reasoning text deltas (~50ms).
- **No `AgentEvent::ThinkingEnd`** — end must be synthesized.

### Current Behavior

`StreamDelta::Thinking` (start signal) is sent to the gateway collector, but the collector doesn't emit a distinct chunk for it — it just sets internal state. The frontend sees only `reasoning` deltas, never an explicit start marker.

### Change

**`crates/oxios-gateway/src/gateway.rs`** (collector) — when it sees the first `StreamDelta::Thinking` for a turn, emit a partial OutgoingMessage with:

```rust
metadata.insert("stream_kind".to_string(), "reasoning.start".to_string());
```

The WS handler in `chat.rs` adds a new branch:

```rust
let is_reasoning_start = msg.metadata.get("stream_kind").map(|v| v.as_str()) == Some("reasoning.start");
let is_reasoning_end = msg.metadata.get("stream_kind").map(|v| v.as_str()) == Some("reasoning.end");

if is_reasoning_start {
    // emit { type: "reasoning.start", message_id, ... }
}
else if is_reasoning_end {
    // emit { type: "reasoning.end", message_id, duration_ms?, ... }
}
else if is_reasoning { /* existing delta path */ }
```

**Synthesizing `reasoning.end`:** the collector tracks `was_reasoning: bool`. When it sees the first `StreamDelta::Text` after `was_reasoning == true`, OR when the terminal message arrives (`partial != Some(true)`), it emits a final `reasoning.end` chunk before the text/done chunk.

### Frontend Impact

`adapter.ts` already handles `reasoning.start` / `reasoning.end` event kinds. The current synthesis (first delta = start, done = end) is replaced by the explicit wire events. Remove synthesis, keep event handling. Net frontend diff: ~10 lines.

### Acceptance

- Backend test: emit `Thinking` event → WS receives `reasoning.start` chunk → emit `Text` event → WS receives `reasoning.end` chunk → then `token` chunks.
- Animation: Thinking block expands on `reasoning.start`, collapses on `reasoning.end` (current frontend collapses only on `done`, which fires later).

---

## 5. Phase C — Tool Args Streaming (BLOCKED — Documented Only)

### Why Impossible

Verified at `agent_runtime.rs:1093`:

```rust
AgentEvent::ToolExecutionStart {
    tool_name,
    tool_call_id,
    args,                // ← fully-parsed serde_json::Value
    ..
}
```

oxi-sdk parses tool-call JSON internally before invoking the kernel callback. There is no `ToolArgsDelta` event anywhere in the `AgentEvent` enum. The kernel cannot tap into raw LLM deltas before parsing.

### What It Would Take

An upstream PR to oxi-sdk adding:

```rust
AgentEvent::ToolCallDelta { tool_call_id: String, args_delta: String }
```

emitted as the LLM streams partial JSON. Plus oxi-sdk's internal tool-call parser would need to expose the raw accumulator before final parse.

Per `AGENTS.md`: **"oxi-sdk is crates.io only. Never add as path dep. Never reimplement what it provides."** So this is firmly upstream.

### Status

Out of scope. If oxi-sdk ever exposes this, the kernel change is small (forward `ToolCallDelta` to a new WS chunk type). Until then, LobeHub's "watch the LLM construct tool args" UX is unreachable.

---

## 6. Phase D — Tool Intervention Manifest (Medium-High, RFC-017 Designed)

### Current State (Verified)

- `GatedTool::execute` (`gated_tool.rs:118–160`) does **synchronous** `AccessGate::check()`. On deny, returns `AgentToolResult::error(format_denied(...))`. No async approval path.
- `PendingToolApprovals` (`tools/pending_tool_approvals.rs`) exists as a oneshot registry, **wired to the API** (`/api/chat/tool-approval/:id/respond`) **but not called from GatedTool**.
- `ToolMeta` (`tools/registry.rs`) has `name`/`description`/`category` only. **No `human_intervention` field.**
- RFC-017 (`docs/rfc-017-tool-capability-escalation.md`) envisioned the full async approval flow. Architecture review `2026-05` flags this as gap G8 ("GatedTool→ApprovalRequested not wired").
- The `tool_approval` WS chunk IS already emitted somewhere (chat.rs handles it), but only for ad-hoc approvals — not driven by tool manifests.

### Change (Multi-step)

**D.1 — Add intervention metadata to ToolMeta:**

```rust
// crates/oxios-kernel/src/tools/registry.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HumanIntervention {
    /// Tool is safe to run without user confirmation.
    None,
    /// Approval required only when args match certain criteria (path outside sandbox, etc.).
    Conditional,
    /// Approval always required before execution.
    Required,
}

pub struct ToolMeta {
    pub name: &'static str,
    pub description: &'static str,
    pub category: &'static str,
    pub human_intervention: HumanIntervention,   // ← NEW
}
```

Initial assignments (conservative):
- `None`: `read_file`, `glob`, `grep`, `list_files`, `web_search`, `web_fetch`, `knowledge`, `project`, `persona`, `cron`, `marketplace`, `resource`, `security`, `mount`, `budget`, `kernel_agent`.
- `Conditional`: `write_file`, `edit_file` (path-based — already enforced by AccessGate, but UI can show "editing file X" preview).
- `Required`: `exec` (shell execution — always ask).

**D.2 — Wire GatedTool async approval path:**

When `tool_meta.human_intervention == Required` (or Conditional + criteria match), `GatedTool::execute`:
1. Generates approval_id (UUID).
2. Registers oneshot in `PendingToolApprovals`.
3. Publishes `KernelEvent::ToolApprovalRequested { approval_id, tool_name, args_summary, reason }`.
4. Awaits oneshot (with timeout).
5. On approve → proceed to inner.execute. On deny/timeout → return error.

**D.3 — Expose manifest endpoint:**

```
GET /api/tools/manifest → [{ name, description, category, human_intervention }, ...]
```

`src/api/routes/` adds a new handler returning the ToolMeta catalog. Frontend uses this to know which tools should render intervention components.

**D.4 — Frontend 4-tier interventions slot:**

The frontend `web/src/components/chat/tool-renders/registry.tsx` already has `registerToolIntervention` typed. With D.3 endpoint, the frontend can fetch the manifest at app boot and:
- Show a per-tool custom approval UI when `human_intervention == Required`.
- Show a path-scope audit preview when `Conditional`.

### Frontend Impact

- New endpoint client (`web/src/hooks/use-tools-manifest.ts`).
- Tool approval flow already partially exists (`tool_approval` WS chunk handler in chat store). Extend it to look up the manifest for richer UI.

### Acceptance

- `exec` tool invocation pauses with approval card before running.
- Manifest endpoint returns correct intervention level per tool.
- Existing synchronous deny path (path outside sandbox) still works — it's separate from the new approval flow.

### Risk

This is the most invasive change. Touches: tool registration, GatedTool hot path, event bus, API surface, frontend manifest client. Recommend a dedicated RFC before implementation.

---

## 7. Phase E — Grounding Chunk (Low, Self-Contained)

### Current State

`web_search` tool returns search results as part of `tool_result`. The frontend's `tool_end` handler in `adapter.ts` extracts them, but they're buried in the tool JSON — no dedicated UI section.

### Change

When `ToolExecutionFinished` fires for `web_search` (or `get_search_results`), `kernel_event_to_ws_chunk` in `chat.rs` (or a new dedicated translator) inspects the structured result and emits an additional `grounding` chunk:

```rust
if event.tool_name == "web_search" || event.tool_name == "get_search_results" {
    if let Some(citations) = parse_search_citations(&event.result) {
        // emit separate grounding chunk
        let grounding_chunk = serde_json::json!({
            "type": "grounding",
            "message_id": ...,
            "citations": citations,
            // optionally: image_results, search_queries
        });
        // send before/alongside the tool_end chunk
    }
}
```

`parse_search_citations` extracts URLs/titles from the tool's result format (which varies by provider — Google, Brave, Tavily).

### Frontend Impact

`adapter.ts` already maps `grounding` event to `message.search` field. AssistantMessage pipeline already renders `SearchGrounding` component when `message.search` is populated. **Zero frontend change** — just populate the field via the new chunk.

### Acceptance

- A web_search tool call results in a citations panel above the tool card (not buried inside it).

---

## 8. Phase F — Live Usage Counter (Partial, Upstream-Locked)

### Current State

`AgentEvent::TokenUsage { .. }` exists, fires once at end of stream → `KernelEvent::TokenUsageUpdate`. Frontend sees `usage` chunk once.

### What's Missing

LobeHub shows a live token counter that increments during streaming. This requires either:
- oxi-sdk to emit `TokenUsage` periodically during the stream (currently only at end), OR
- Oxios to estimate usage locally from token count × model pricing.

Option 2 is feasible without oxi-sdk changes. The kernel already counts text deltas; multiplying by per-model token/char ratio gives a rough estimate.

### Status

Low priority. Document as future work. The single end-of-stream `usage` chunk is adequate for most use cases.

---

## 9. Implementation Order

```
[Phase A] MessageId propagation
    ↓  (unblocks concurrent stream UI)
[Phase B] Reasoning start/end split
    ↓  (unblocks smooth Thinking animation)
[Phase E] Grounding chunk
    ↓  (unblocks rich search UX)
─────── 以上、1 PR で出せる。低リスク・高効果。 ───────
[Phase D] Tool intervention manifest
    ↓  (別 PR。より侵襲的。RFC で設計を固めるべき)
[Phase C] Tool args streaming  ❌ BLOCKED — oxi-sdk upstream
[Phase F] Live usage counter   ⏸ DEFERRED — low value, partial upstream lock
```

**First PR scope:** Phases A + B + E. Estimated 200–400 lines of Rust across `chat.rs`, `gateway.rs`, `agent_runtime.rs` event publishers. No oxi-sdk changes. No new endpoints.

**Second PR (after RFC):** Phase D. Touches more files, changes GatedTool hot path. Don't bundle with A+B+E.

---

## 10. Test Strategy

- **Unit**: extend `kernel_event_to_ws_chunk` tests in `chat.rs` (already has `reasoning_emits_reasoning_chunk` pattern at line 1758). Add assertions for `message_id` field, `reasoning.start`/`reasoning.end` chunk types, `grounding` chunk shape.
- **Integration**: spin up a real agent run with a reasoning-capable model, capture WS chunks, assert lifecycle ordering: `model → reasoning.start → reasoning.delta* → reasoning.end → token* → tool_start → tool_end → grounding → done`.
- **Frontend regression**: existing 42 stores tests still pass. New test: chunk with explicit `message_id` routes to the correct message even when another stream is in flight.

---

## 11. Migration & Compatibility

All changes are **additive**:
- New chunk types (`reasoning.start`, `reasoning.end`, `grounding`) — old frontends ignore unknown types (parseChunk returns null, store drops).
- New `message_id` field on existing chunks — old frontends ignore unknown fields.
- No chunk type is removed or renamed.

So the backend can ship before the frontend adapter is updated, and vice versa. The frontend already has fallback synthesis for missing events.

---

## 12. What This Design Does NOT Address

- **Multimodal content parts** (`image`/`video`/`audio` chunks) — needs oxi-sdk + agent runtime support for multimodal models. Out of scope.
- **12 message role types** (supervisor, agentCouncil, verify, etc.) — Oxios architecture is single-agent-by-default. Multi-agent deliberation is a separate concern.
- **Branching** (`parentId` chains) — explicitly out of scope per user decision.
- **Follow-up chips auto-generation** — needs backend LLM call to generate suggestions. Separate feature.

These are all reachable in principle but each is a larger effort than Phases A+B+E combined.

---

## 13. Next Action

If the user approves this design:
1. Implement Phases A + B + E as one PR (~1–2 days).
2. Open RFC for Phase D (tool intervention) — design GatedTool async path, ToolMeta schema, manifest endpoint.
3. File upstream issue against oxi-sdk for Phase C (ToolArgsDelta event).
4. Defer Phase F until usage estimation is requested.

If the user wants to proceed, say "A+B+E 착수" or specify a different starting point.
