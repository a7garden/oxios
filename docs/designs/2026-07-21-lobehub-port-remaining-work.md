# LobeHub Chat Port — Remaining Work

> **Created:** 2026-07-21
> **Companion to:**
> - `docs/designs/2026-07-21-lobehub-chat-port-design.md` (frontend design, 6 phases — shipped)
> - `docs/designs/2026-07-21-lobehub-backend-streaming-design.md` (backend design, 6 phases — A/B/E/D shipped)
> **Status:** 15 commits on `main`. Frontend Phases 1–6 complete. Backend Phases A/B/E/D complete. Items below are explicitly deferred or blocked.

---

## 1. Blocked on oxi-sdk Upstream

### 1.1 Tool Args Streaming (`tool_call_delta`)

**What:** LobeHub streams tool-call arguments as partial JSON deltas while the LLM constructs them. The user sees the args forming token-by-token before the tool executes.

**Why blocked:** oxi-sdk parses tool-call JSON internally before invoking the kernel callback (`agent_runtime.rs:1093` — `AgentEvent::ToolExecutionStart { args: serde_json::Value }`). There is no `AgentEvent::ToolCallDelta` variant anywhere in the oxi-sdk event surface. Per `AGENTS.md`, oxi-sdk is crates.io-only and must not be reimplemented.

**What it would take:** Upstream PR to oxi-sdk adding:
```rust
AgentEvent::ToolCallDelta { tool_call_id: String, args_delta: String }
```
emitted as the LLM streams partial JSON, before the final parse. Then a small kernel change to forward it as a `tool_call_delta` WS chunk. Frontend adapter already has the `ChatEvent` type slot.

**Tracking:** File an issue against `a7garden/oxi` referencing this doc.

### 1.2 Live Usage Counter (Partial)

**What:** LobeHub shows a token counter that increments during streaming, not just at completion.

**Why partially blocked:** oxi-sdk emits `AgentEvent::TokenUsage` once at end-of-stream (`KernelEvent::TokenUsageUpdate`). Periodic mid-stream usage would need oxi-sdk to emit it more frequently.

**Workaround (kernel-only):** The kernel already counts text/reasoning deltas. A rough estimate (`deltas × per-model token/char ratio`) could be computed locally without oxi-sdk changes. Low priority — the single end-of-stream `usage` chunk is adequate.

---

## 2. Deferred — Requires Dedicated RFC

### 2.1 GatedTool Async Approval Path (Phase D.2)

**What:** When `ToolMeta.human_intervention == Required` (or Conditional + criteria match), GatedTool should pause execution, request user approval via WS, and await a response — instead of the current synchronous deny-only path.

**Current state:**
- `HumanIntervention` enum added to `ToolMeta` (Phase D.1 — shipped).
- `exec` = Required, `write`/`edit` = Conditional, rest = None.
- `PendingToolApprovals` registry exists (`crates/oxios-kernel/src/tools/pending_tool_approvals.rs`) and is wired to the API (`/api/chat/tool-approval/:id/respond`) — but **NOT called from GatedTool**.
- RFC-017 (`docs/rfc-017-tool-capability-escalation.md`) envisioned this flow. Architecture review (`docs/designs/architecture-review-2026-05/rfc-015-security-unification.md` gap G8) flagged it as unimplemented.

**Why deferred:** GatedTool's `execute()` is a hot path (called on every tool invocation). Injecting `PendingToolApprovals` + async oneshot + timeout changes the execution contract. Needs:
1. GatedTool gains `pending_approvals: Option<Arc<PendingToolApprovals>>` field.
2. Constructor updated everywhere `gate_tool()` is called (`crates/oxios-kernel/src/tools/kernel_bridge.rs`).
3. `execute()` checks `human_intervention` before the inner call; if Required → register, publish `KernelEvent::ApprovalRequested`, await oneshot (with timeout).
4. WS handler already has `tool_approval` chunk support (chat.rs).
5. Frontend 4-tier registry's `interventions` slot already typed.

**Risk:** Getting the timeout/abort semantics wrong could hang the agent. Needs careful testing with concurrent tool calls.

### 2.2 Message Branching (`parentId` chains)

**What:** LobeHub supports branching — regenerate a response creates an alternative branch, navigable via UI arrows. `parentId` chain on messages enables this.

**Why deferred:** User explicitly excluded from scope (2026-07-21 decision). Both data model (`parentId`, `threadId` on `ChatMessage`) and backend support are absent. Would require:
1. `ChatMessage` gains `parentId?: string`, `threadId?: string`.
2. Session store persists parent-child relationships.
3. `/api/chat` or WS handler supports "regenerate from message N" producing a new branch.
4. Frontend branch navigation UI.

### 2.3 Follow-up Chips (AI-suggested questions)

**What:** After each assistant response, LobeHub shows 3-5 suggested follow-up questions.

**Why deferred:** User explicitly excluded from scope. Backend has no mechanism to generate suggestions. Would require:
1. Additional LLM call after each response to generate follow-up suggestions.
2. New WS chunk type or API endpoint.
3. Frontend `FollowUpChips` component already exists (`web/src/components/chat/follow-up-chips.tsx`) but is fed static data.

---

## 3. Partially Implemented — Polish Needed

### 3.1 Composer (ChatInput) — Phase 5 Gaps

**What was shipped:** Slash commands expanded from 3 → 10.

**What was NOT shipped (design §7 Phase 5 promised):**
- ActionBar config-map restructure — `chat-input-action-bar.tsx` still uses hardcoded button JSX instead of LobeHub's `config.ts` + `actionMap` pattern.
- @-mention category expansion — currently `knowledge`/`memory`/`mounts`. LobeHub has 6 categories (agents, members, topics, skills, tools, files). Adding `skills` and `topics` would require new Tiptap suggestion plugins and API queries.

**Why deferred:** `chat-input.tsx` is 600+ lines. Inline refactor risk was high relative to value. Existing input works fine.

**Effort:** Medium (1-2 days for config-map, 2-3 days for mention expansion).

### 3.2 Mobile Responsive Audit

**What:** Phase 6 design called for mobile viewport testing. Only verified that Tailwind responsive utilities exist.

**Known concern:** `UserMessage` textarea has `min-w-[300px]` which overflows on very narrow screens (<320px viewport). Pre-existing, not introduced by this work.

**Effort:** Low (half-day audit + targeted fixes).

### 3.3 Quick-Ask Visual Parity

**What:** `quick-ask.ts` now shares `StreamProcessor` with `chat.ts` (committed in `8d84eef6b`). But the QuickAsk dialog UI still uses the old `MessageBubble` component which delegates to `MessageView` — so it renders the new pipeline. However, the QuickAsk dialog layout (header, scroll area, footer) was not redesigned.

**Effort:** Low — mostly CSS/layout work.

### 3.4 Tool Render Coverage

**What was shipped:** 9 renders registered (FileRead, FileEdit, Bash, WebSearch, Glob, Grep, ListFiles, WebFetch + Default fallback).

**What is NOT registered (Oxios kernel tools without custom renders):**
- `knowledge` — file read/write/delete operations on knowledge base
- `project` — project listing/getting
- `persona` — persona switching
- `cron` — cron job management
- `security` — audit trail queries
- `budget` — budget management
- `resource` — system resource monitoring
- `calendar` — event CRUD
- `send_email` — email sending
- `a2a_delegate` / `a2a_send` / `a2a_query` — agent-to-agent delegation
- `mount` — mount exploration
- `kernel_agent` — agent lifecycle management
- `marketplace` — skill store
- `skill_forge` — skill creation/validation

These all fall through to `DefaultToolRender` (JSON args + output). Not broken, just not rich.

**Effort:** Low per tool (30min-1hr each). Prioritize `knowledge`, `a2a_delegate`, `send_email`.

### 3.5 Markdown Plugins

**What was shipped:** `rehype-thinking` (recognizes `<think>`/`<thinking>`/`<reasoning>` HTML tags → collapsible `<details>`).

**What LobeHub has but we don't:**
- `LobeArtifact` — inline artifact cards (code blocks with title, language, download button)
- `Mention` — inline @-mention rendering in assistant prose
- `Skill` — inline skill invocation rendering
- `Tool` — inline tool-call rendering embedded in markdown
- `Link` — link card previews (OpenGraph-style)

**Effort:** Medium (each plugin is a rehype transformer + React component, ~100-200 lines).

---

## 4. Architecture Debt from This Work

### 4.1 `active_message_id` Tracking is Per-Connection

**What:** Phase A tracks `active_message_id` in the WS handler's recv loop. It's updated when streaming chunks arrive and used to stamp KernelEvent-sourced chunks.

**Limitation:** If two streams target the same connection simultaneously (multi-agent, background tasks), the last-seen `active_message_id` wins. Proper fix: thread `message_id` through `KernelEvent` variants themselves (design doc §3 option 1). Avoided in Phase A to limit match-arm churn.

**Effort:** Medium (add `message_id: Option<String>` to 5 KernelEvent variants, update ~20 construction sites, update 3 match consumers).

### 4.2 Grounding URL Extraction is Heuristic

**What:** Phase E extracts URLs from `output_summary` string via simple `http` prefix scanning. No structured citation data.

**Limitation:** Misses URLs embedded in JSON structures, markdown without `http` prefix, or provider-specific formats (Brave, Tavily, Google CSE). Titles only extracted from `[title](url)` markdown pattern.

**Proper fix:** Have the `web_search` tool publish structured results (URL, title, snippet) as a separate event or in the `ToolExecutionFinished` payload.

**Effort:** Low-Medium (modify tool result serialization + update extraction).

### 4.3 Reasoning `end` Synthesis Timing

**What:** Phase B synthesizes `reasoning.end` when:
1. First `StreamDelta::Text` arrives after reasoning, OR
2. Stream ends while still reasoning (loop exit).

**Limitation:** If the model interleaves reasoning and text (reason → text → more reason), only the first transition emits `reasoning.end`. The second reasoning span won't get a new `reasoning.start` because `AgentEvent::Thinking` only fires once per turn in most models. This matches LobeHub's behavior (single thinking block per message) but differs from models that produce multiple reasoning spans.

---

## 5. Testing Gaps

### 5.1 No Backend Integration Test for New Chunk Types

**What:** Phases A/B/E added new WS chunk fields/types. Verified via:
- Unit tests for `kernel_event_to_ws_chunk` (11 existing tests pass)
- Unit tests for `grounding_from_event` URL extraction (not yet written)
- Frontend unit tests for StreamProcessor (42 tests pass)

**Missing:** End-to-end test that spins up a real WS connection, sends a chat message, and asserts the chunk sequence: `model → reasoning.start → reasoning.delta* → reasoning.end → token* → tool_start → tool_end → grounding? → done`. All with correct `message_id` values.

**Effort:** Medium (requires test harness with mock oxi-sdk or a real agent run).

### 5.2 No Visual Regression Test

**What:** No automated test for Thinking block animation, tool inspector expand/collapse, or error card rendering. These are visual behaviors that unit tests can't capture.

**Mitigation:** Manual browser verification by user.

---

## 6. Priority Recommendations

If continuing this work, suggested order:

1. **Grounding structured data** (§4.2) — high user-visible value, low effort
2. **GatedTool async approval** (§2.1) — completes the tool intervention UX loop
3. **Tool renders for remaining kernel tools** (§3.4) — incremental, each is independent
4. **Markdown plugins** (§3.5) — Artifact and Link cards are highest value
5. **MessageId in KernelEvent variants** (§4.1) — only needed when concurrent streams become real
6. **Composer ActionBar + mentions** (§3.1) — lower urgency, current input works
7. **File oxi-sdk upstream issue** (§1.1) — unblocks tool args streaming
