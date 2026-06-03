# Korean Translation Progress

## Date: 2026-05-31

## Status: ✅ COMPLETE — 100% Translated (879/879 keys)

## Summary

### Issue Found
The `en.json` file had been **accidentally overwritten with Korean translations** during the i18n sync commit (`1dc5a03: fix(web): i18n — bundled translations, fixed missing keys, EN/KO sync`). Both `en.json` and `ko.json` contained identical Korean text, making comparison impossible.

### Resolution
1. **Restored `en.json`** from the original English source (commit `d310a6f`, which was in TS format) and added proper English values for 203 new keys that were added after the original.
2. **Verified `ko.json`** is already fully translated to Korean — all 879 keys have proper Korean translations.

### Translation Statistics

| Metric | Value |
|--------|-------|
| Total keys | 879 |
| Korean translated | 879 (100%) |
| Intentionally same as English (technical terms) | 9 |
| New keys added (vs original) | 203 |

### Intentionally Untranslated (Technical Terms / Brand Names)
These correctly remain identical in both languages:
- `common.git` = "Git" (brand name)
- `common.oxiosBrand` = "Oxios Agent OS" (brand name)
- `settings.jsonElkLoki` = "JSON (ELK/Loki)" (technical format)
- `settings.groupAi` = "AI" (abbreviation)
- `engine.ctx` = "ctx" (abbreviation)
- `resources.cpu` = "CPU" (abbreviation)
- `sessions.id` = "ID" (abbreviation)
- `a2a.direction` = "From → To" (directional notation)
- `git.title` = "Git" (brand name)

### New Sections Added (203 keys)
These were added after the original English source and already have Korean translations:
- `common.*` — 10 new common UI strings
- `settings.routing.*` — 10 model routing config strings
- `settings.group*` — 5 setting group labels
- `engine.*` — 2 engine state strings
- `agents.*` — 37 agent detail/trace strings + `logLevel` sub-object
- `seeds.*` — 28 ouroboros phase/evaluation strings
- `sessions.*` — 3 session management strings
- `skills.*` — 13 skill management strings
- `budget.*` — 18 budget management strings
- `agentGroups` — 10 new section (agent group monitoring)
- `a2a` — 13 new section (A2A protocol monitor)
- `memory.*` — 64 memory tier/dream/management strings

### Files Modified
- `surface/oxios-web/web/src/i18n/locales/en.json` — Restored to proper English
- `surface/oxios-web/web/src/i18n/locales/ko.json` — Verified complete (no changes needed)
- `ko-translated.json` — Output copy of the complete Korean translation

---

# RFC-015 Chat Transparency

## Date: 2026-06-03

## Status: ✅ COMPLETE — Backend wire format + frontend UI + persistence

## Summary

The Web chat UI previously showed a spinner during agent execution and
revealed the final response only at the end. RFC-015 streams real-time
agent activity (tool calls, memory recall, reasoning, token usage) into
the chat as collapsible cards, with full persistence so the timeline
survives page reloads.

### Implementation Phases

#### Phase 1: Backend wire format
- 5 new `KernelEvent` variants: `ToolExecutionStarted`,
  `ToolExecutionFinished`, `MemoryRecallUsed`, `TokenUsageUpdate`,
  `ReasoningFragment`. Mapped to audit actions and `extract_agent_id`.
- `agent_runtime.rs` `run_streaming` callback publishes events; session
  ID is derived from `seed.id` and threaded through a new
  `execute_with_session` entry point.
- `events.rs` `sanitize_event` covers the new variants so they appear on
  the global `/api/events` SSE channel.
- `chat.rs` WS handler subscribes to the kernel event bus alongside
  `outgoing_rx`, biases `tokio::select!` toward gateway messages, and
  filters kernel events by `active_session_id` so unrelated agents do
  not leak.

#### Phase 2: Session persistence
- `Session.trajectory_steps: Vec<TrajectoryStepRecord>` + helper
  `extend_trajectory()`. `TrajectoryStepRecord` duplicates the relevant
  fields of `memory::sona::TrajectoryStep` to avoid a kernel → memory
  dependency cycle.
- Both POST `/api/chat` and WS `persist_session` now extract
  `tool_calls` from the response metadata and append as
  `TrajectoryStepRecord`s.
- `GET /api/sessions/:id` returns `trajectory_steps` in the JSON
  response.

#### Phase 3: Frontend
- `StreamChunk` extended with 6 new chunk types; new `ChatActivity`
  type (phase, tool_call, memory, reasoning, usage).
- `chat.ts` `handleChunk` rewritten as a `switch` statement;
  `chunkToActivity` and `trajectoryToActivity` helpers bridge backend
  chunks → frontend activity entries.
- `loadSession` reconstructs the timeline from `trajectory_steps` so the
  replay view matches the live view.
- New components:
  - `ActivityCard` — single collapsed card (icon, label, duration, error
    badge). Expands to show tool I/O, memory details, reasoning text,
    or token counts.
  - `ActivityTimeline` — wrapper that summarises N activities and a
    tool/token count in the header, then renders the cards. Collapses
    automatically when >8 activities.

#### Phase 4: Polish
- 20 new i18n keys under `chat.transparency.*` in both `en.json` and
  `ko.json` (16 base + 4 plural variants).
- 2 new unit tests in `event_bus::tests` for the new variants:
  `test_rfc015_event_round_trip_json` (round-trip stability) and
  `test_rfc015_extract_agent_id` (audit mapping).

### Files Modified
- `crates/oxios-kernel/src/event_bus.rs` — 5 new variants + audit map
- `crates/oxios-kernel/src/agent_runtime.rs` — `execute_with_session`, run_agent signature
- `crates/oxios-kernel/src/state_store.rs` — `TrajectoryStepRecord`, `Session::extend_trajectory`
- `surface/oxios-web/src/routes/chat.rs` — WS event subscription, kernel_event_to_ws_chunk
- `surface/oxios-web/src/routes/events.rs` — sanitize_event RFC-015 entries, GET session trajectory
- `surface/oxios-web/web/src/types/index.ts` — ChatActivity, StreamChunk extensions
- `surface/oxios-web/web/src/stores/chat.ts` — handleChunk switch, helpers
- `surface/oxios-web/web/src/components/chat/activity-card.tsx` — new
- `surface/oxios-web/web/src/components/chat/activity-timeline.tsx` — new
- `surface/oxios-web/web/src/components/chat/message-bubble.tsx` — embed timeline
- `surface/oxios-web/web/src/i18n/locales/{en,ko}.json` — 20 new keys
- `docs/rfc-015-chat-transparency.md` — design doc

### Verification
- `cargo check --workspace` — passes
- `cargo test -p oxios-kernel --lib event_bus` — 7/7 pass
- `bun run typecheck` — no new errors
- `bun run build` — succeeds

### Design Doc
- `docs/rfc-015-chat-transparency.md` — full design rationale,
  protocol shapes, and migration plan

---

# RFC-015 Chat Transparency — Polish & Test Pass

## Date: 2026-06-03 (continued)

## Status: ✅ COMPLETE — markdown highlighting, i18n, unit tests

## Summary

Follow-up pass on RFC-015. Closes out the "optional" work items with full
test coverage and a polished UI.

### Phase 5: Markdown syntax highlighting
- Added `rehype-highlight` + `highlight.js` (github-dark theme).
- `message-bubble.tsx` renders fenced code blocks with the language tag
  detected by `rehype-highlight`. Theme CSS imported in `index.css`.
- `rehype-highlight` runs alongside `remark-gfm` in the ReactMarkdown
  pipeline; no other code changes required.

### Phase 6: i18n on chat transparency components
- `ActivityCard` and `ActivityTimeline` switched from hardcoded English
  to `t('chat.transparency.*')` calls. The 20 keys added in Phase 4 are
  now actually consumed; plural variants (`_one` / `_other`) wired up
  via i18next's automatic count-based selection.
- Korean translations live in `ko.json`; the rest of the UI already
  follows the same `t()` convention.

### Phase 7: Unit tests
- `crates/oxios-kernel/src/event_bus.rs` (already in Phase 4):
  `test_rfc015_event_round_trip_json` and
  `test_rfc015_extract_agent_id` — 7/7 pass.
- `crates/oxios-web/src/routes/chat.rs`: 8 new tests in
  `rfc015_tests` module covering:
  - `tool_started_emits_tool_start` — wire format
  - `tool_finished_emits_tool_end` — wire format
  - `memory_recall_emits_memory_chunk` — wire format
  - `token_usage_emits_usage_chunk` — wire format
  - `reasoning_emits_reasoning_chunk` — wire format
  - `foreign_session_is_filtered` — security/correctness
  - `no_active_session_passes_session_scoped_events` — behaviour
  - `lifecycle_events_are_skipped` — keeps WS stream clean
- `surface/oxios-web/web/src/__tests__/stores.test.ts`: 7 new tests for
  `useChatStore.handleChunk` covering every chunk type and the
  tool_start/tool_end merge behaviour. Also fixed a behavioural bug in
  the merge logic uncovered by the test (tool_start + tool_end with the
  same `toolCallId` now correctly merge into a single activity, rather
  than the second being silently dropped).

### Verification
- `cargo check --workspace` — passes
- `cargo test -p oxios-kernel --lib event_bus` — 7/7 pass
- `cargo test -p oxios-web --lib rfc015_tests` — 8/8 pass
- `bun run typecheck` — no new errors (pre-existing
  `AiDetectionState`/`err` warnings belong to other sessions)
- `bun run test` — 135/135 pass (was 122; +13 RFC-015 tests)
- `bun run build` — succeeds

### Files Modified
- `crates/oxios-kernel/src/event_bus.rs` (already in Phase 4; tests in
  Phase 7 verified the wire format is stable)
- `surface/oxios-web/src/routes/chat.rs` — 8 new tests
- `surface/oxios-web/web/src/components/chat/activity-card.tsx` — i18n
- `surface/oxios-web/web/src/components/chat/activity-timeline.tsx` — i18n
- `surface/oxios-web/web/src/components/chat/message-bubble.tsx` —
  rehype-highlight
- `surface/oxios-web/web/src/index.css` — highlight.js theme import
- `surface/oxios-web/web/src/stores/chat.ts` — tool_start/tool_end merge
- `surface/oxios-web/web/src/__tests__/stores.test.ts` — 7 new tests
- `surface/oxios-web/web/package.json` — `rehype-highlight`,
  `highlight.js`
- `bun.lock` (or `bun.lockb`) — locked new deps

# RFC-T1-C: Live Operations Dashboard

## Date: 2026-06-04

## Status: ✅ COMPLETE — MVP delivered (3 of 6 widgets; 3 deferred per scope)

## Summary

Rewrote the home dashboard from a static 4-card overview into a
"Live Operations Center" pattern (TweetDeck/Grafana-lite). The MVP
ships the 3 most impactful widgets per RFC §Scope; the remaining 3
(Resource Trends, Active Agents with traces, Scheduler Next) are
deferred to RFC-T1-D.

### Widgets delivered (3)
1. **5 KPI cards with sparkline + delta** (`components/dashboard/stat-card.tsx`)
   - Total Agents · Running Agents · Tokens/min · CPU · Pending Approvals
   - Sparkline driven by `useResourceHistory(30)` for CPU and a new
     `useTokenRate` hook for tokens/min (derived from SSE `token_usage_update`).
2. **Live Activity Feed** (`components/dashboard/live-activity-feed.tsx`)
   - Subscribes to the existing singleton SSE store via `useEvents`.
   - Filters to ~20 interesting event types (agent.fork/kill/done,
     tool.start/end, memory.recall, approval.requested/resolved, etc.)
   - 200-item cap, event-type filter dropdown, ⏸ Pause toggle that
     snapshots the list for analysis.
3. **Approvals Queue (inline actions)** (`components/dashboard/approvals-queue.tsx`)
   - Uses `useApprovals` with optimistic TanStack Query mutations.
   - Hides the entire card when there are 0 pending.

### Supporting infrastructure
- `lib/event-formatter.ts` — central mapping of SSE event types to
  icon / color / one-line summary + click-routing. Reusable beyond
  the dashboard (e.g. the existing `/events` page or notifications).
- `hooks/use-approvals.ts` — shared approvals hook with optimistic
  approve/reject (also used by the `/approvals` page).
- `hooks/use-resource-history.ts` — `useResourceHistory(lastN)` +
  `seriesFromSnapshots` + `computeDelta` helpers.
- `hooks/use-token-rate.ts` — derives tokens/min from the SSE stream.
- `routes/index.tsx` — DashboardLayout rewrite. Layout: stat row →
  (Live Activity Feed | Active Agents preview) → Approvals Queue →
  System Health → Model Usage → Quick Links.

### i18n
- ~15 new keys added under `dashboard.*` in `en.json` and `ko.json`
  (e.g. `tokensPerMin`, `liveActivity`, `pause`, `resume`,
  `pendingApprovals`, `needsAttention`, `moreEvents`, …).

### Tests
- 3 Playwright smoke tests in `e2e/dashboard.spec.ts` — all 3 pass
  against a running oxios daemon. The CI workflow intentionally does
  NOT run Playwright (per `.github/workflows/ci.yml`); e2e is for
  local dev only.
- 135/135 unit tests still pass.
- `bun run typecheck` — 0 new errors introduced (baseline 54
  pre-existing, unchanged after my changes).
- `bun run build` — succeeds.

### Deferred to follow-up (per RFC scope, RFC-T1-D)
- **Resource Trends area chart** (3-series CPU/MEM/TOK with threshold
  lines) — slot reserved at top-right of the activity row.
- **Active Agents list with traces** (per-agent elapsed time, tokens,
  click-through to trace view).
- **Scheduler Next widget** (next 3 cron jobs with countdown).
- Widget on/off customization, animated transitions.

### Files Modified
- `surface/oxios-web/web/src/routes/index.tsx` — DashboardLayout
  rewrite (replaces static 4-card view)
- `surface/oxios-web/web/src/i18n/locales/en.json` — +15 keys
- `surface/oxios-web/web/src/i18n/locales/ko.json` — +15 keys
