# RFC-017: Runtime Tool Capability Escalation

> **Status**: Draft
> **Date**: 2026-06-07
> **Related**: RFC-016 (Questionnaire), RFC-014 (Space/CSpace)

## Problem

When an agent tries to use a tool it lacks CSpace capability for (e.g. `web_search` with a `worker` role), the `GatedTool` returns a hard error. The agent has three options:

1. Find an alternative approach (use `exec curl` instead of `web_search`)
2. Fail and tell the user
3. Confuse the user with a cryptic error message ("CSpace에 'web_search' 도구에 대한 EXECUTE capability 없음")

None of these are good UX. The user doesn't know about CSpace or capabilities — they just asked for something and got an error. In many cases the user would happily approve the tool access if asked.

**Current flow (broken):**
```
Agent calls web_search
  → GatedTool: "권한 거부"
  → Agent: tries exec curl as fallback
  → exec curl: timeout / raw HTML
  → Result: partial or failed
```

**Desired flow:**
```
Agent calls web_search
  → GatedTool: "need approval"
  → User sees: "'web_search' 권한이 필요합니다. 승인하시겠습니까?"
  → User: 승인 → tool retries → success
  → User: 거부 → agent gets error → tries alternative
```

## Proposal

When `GatedTool` denies a tool call due to missing CSpace capability, instead of returning a hard error, pause execution and ask the user for approval. This is the same oneshot-await pattern used by the `questionnaire` tool (RFC-016).

### Scope

| Denial reason | Escalatable? | Why |
|---|---|---|
| CSpace capability missing | ✅ Yes | User can grant temporary access |
| RBAC role restriction | ❌ No | Admin policy, not user decision |
| Path sandbox violation | ❌ No | Security boundary |
| Exec policy (allowlist) | ❌ No | Admin policy |

Only **CSpace capability** denials are escalated. RBAC, path sandbox, and exec policy denials remain hard errors.

## Design Decisions

### D1: Approval scope — per-call vs per-session vs permanent

**Decision: Per-session with memoization.**

When the user approves `web_search` once, all subsequent `web_search` calls within the same session are automatically allowed. The user doesn't need to approve every single call.

Implementation: after approval, inject the capability into the agent's runtime CSpace for the remainder of the session. The GatedTool's existing CSpace check will pass on subsequent calls.

This means:
- First call: denied → ask user → approved → inject capability → retry
- Second call: CSpace check passes → no dialog needed

### D2: Where to place the approval logic

**Decision: Inside `GatedTool`, not a separate tool.**

The `questionnaire` approach (agent explicitly calls a tool) doesn't work here because the agent doesn't know it lacks a capability until it tries. The denial happens at the `GatedTool` layer, so the escalation must also happen there.

However, `GatedTool` needs access to:
1. `PendingToolApprovals` — to register/await the approval
2. `EventBus` — to publish `ToolApprovalRequested`
3. `CSpace` (mutable) — to inject the approved capability

These are currently not available to `GatedTool`. They need to be passed during construction.

### D3: GatedTool construction — where to wire

**Current path:**
```
agent_runtime.rs
  → register_tools_from_cspace_gated(registry, kernel, cspace, ..., gate, context)
    → register_always_on_gated(registry, search_cache, gate, context)
      → GatedTool::new(WebSearchTool, gate, context)
```

**Proposed path:**
```
agent_runtime.rs
  → register_tools_from_cspace_gated(registry, kernel, cspace, ..., gate, context, approval_flow)
    → register_always_on_gated(registry, search_cache, gate, context, approval_flow)
      → GatedTool::new(WebSearchTool, gate, context).with_approval_flow(approval_flow)
```

Where `approval_flow` bundles the three dependencies into one struct:

```rust
struct ApprovalFlow {
    pending: Arc<PendingToolApprovals>,
    event_bus: EventBus,
    agent_id: AgentId,
    session_id: Option<String>,
    cspace: Arc<RwLock<CSpace>>,  // mutable — for capability injection
}
```

### D4: Timeout

**Decision: 120 seconds.**

Same as questionnaire's 300s is too long for a binary approval. 120s is enough for the user to read the message and decide. On timeout, the pending entry is cancelled and the agent receives a denial error.

### D5: Concurrent requests

**Decision: One approval at a time per session.**

If the agent calls multiple denied tools simultaneously (unlikely but possible), only the first triggers a dialog. The rest queue behind it. This is a natural consequence of the oneshot pattern — each tool call blocks independently, and the UI only shows one approval card at a time.

Actually, since each tool call runs independently in the agent loop, multiple approvals could be requested simultaneously. The frontend should handle this by showing only the latest approval card (replace, not stack). This matches how the questionnaire works — `activeToolApproval` is a single value, not an array.

### D6: Audit trail

**Decision: Publish `ToolApprovalRequested` and `ToolApprovalResolved` events.**

These flow through the existing event bus → SSE → audit log pipeline. No new infrastructure needed.

## Data Model

### PendingToolApprovals

Identical pattern to `PendingQuestionnaires`:

```rust
struct PendingToolApprovals {
    inner: Mutex<HashMap<Uuid, PendingEntry>>,
}

struct PendingEntry {
    tool_name: String,
    sender: oneshot::Sender<ToolApprovalResult>,
}

enum ToolApprovalResult {
    Approved,
    Denied,
}
```

### KernelEvent variants

```rust
ToolApprovalRequested {
    id: Uuid,
    agent_id: AgentId,
    session_id: Option<String>,
    tool_name: String,
    reason: String,
}

ToolApprovalResolved {
    id: Uuid,
    approved: bool,
}
```

### WS chunk (backend → frontend)

```json
{
  "type": "tool_approval",
  "id": "uuid",
  "tool_name": "web_search",
  "reason": "CSpace에 'web_search' 도구에 대한 EXECUTE capability 없음"
}
```

### API endpoint (frontend → backend)

```
POST /api/chat/tool-approval/{id}/respond
Body: { "approved": true }
Response: { "status": "ok" }
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│ GatedTool::execute()                                    │
│                                                         │
│  1. gate.check(tool)                                    │
│     → Err(AccessDenied { layer: Capability, .. })       │
│                                                         │
│  2. Is escalation eligible?                             │
│     - layer == CSpace?                                  │
│     - approval_flow configured?                         │
│     - tool not already memoized?                        │
│                                                         │
│  3. YES → request_approval_and_retry()                  │
│     a. pending.register(tool_name)                      │
│     b. event_bus.publish(ToolApprovalRequested)         │
│     c. await rx (120s timeout)                          │
│     d. if Approved:                                     │
│        - inject capability into CSpace                  │
│        - retry inner.execute()                          │
│     e. if Denied:                                       │
│        - return error to agent                          │
│                                                         │
│  3. NO → return hard error (as before)                  │
└─────────────────────────────────────────────────────────┘
          │                              │
          │ ToolApprovalRequested        │ ToolApprovalResolved
          ▼                              │
┌─────────────────────┐                 │
│ WS chunk → frontend │                 │
│ renders approval    │                 │
│ card                │                 │
│                     ◀─── POST ────────┘
│ user clicks approve │
│ or deny             │
└─────────────────────┘
```

## Frontend Design

### ToolApprovalCard component

```
┌──────────────────────────────────────────────────┐
│ ⚠️ 도구 권한 승인                     web_search │
├──────────────────────────────────────────────────┤
│                                                  │
│ 이 에이전트에게 'web_search' 도구 사용 권한이     │
│ 없습니다. 이 세션에서 허용하시겠습니까?           │
│                                                  │
│              [거부]  [이 세션에서 승인 ✅]         │
└──────────────────────────────────────────────────┘
```

- Appears inline in the chat flow (same position as questionnaire card)
- Replaces the typing indicator
- Chat input is disabled while the approval card is active
- On approve: card disappears, tool call resumes, streaming indicator returns
- On deny: card disappears, agent error message appears

### Chat store state

```typescript
activeToolApproval: {
  id: string;
  toolName: string;
  reason: string;
} | null;
```

Single value — not an array. New approvals replace existing ones.

## Wire-up: agent_runtime.rs

The `ApprovalFlow` is constructed in `agent_runtime.rs` during `spawn_and_run()`:

```rust
let approval_flow = ApprovalFlow {
    pending: kernel.infra.pending_tool_approvals(),
    event_bus: kernel.infra.event_bus(),
    agent_id,
    session_id: Some(session_id),
    cspace: Arc::new(RwLock::new(cspace.clone())),
};
```

Passed to `register_tools_from_cspace_gated()` → `register_always_on_gated()` → each `GatedTool`.

## Implementation Plan

### Phase 1: Backend core

1. **`pending_tool_approvals.rs`** — new registry (oneshot pattern)
2. **`event_bus.rs`** — add `ToolApprovalRequested` / `ToolApprovalResolved`
3. **`infra_api.rs`** — add `pending_tool_approvals` field
4. **`gated_tool.rs`** — add `with_approval_flow()`, `request_approval_and_retry()`
5. **`registration.rs`** — thread `ApprovalFlow` through `register_always_on_gated()`
6. **`agent_runtime.rs`** — construct `ApprovalFlow`, pass to registration

### Phase 2: Backend API

7. **`chat.rs`** — add `kernel_event_to_ws_chunk` mapping for new events
8. **`chat.rs`** — add `POST /api/chat/tool-approval/{id}/respond` handler
9. **`events.rs`** — add SSE mapping
10. **`routes/mod.rs`** — register route

### Phase 3: Frontend

11. **`types/index.ts`** — add `tool_approval` to `StreamChunk.type`
12. **`stores/chat.ts`** — add `activeToolApproval` state + `resolveToolApproval` action + chunk handler
13. **`tool-approval-card.tsx`** — new component
14. **`routes/chat.tsx`** — render the card

### Phase 4: Capability injection

15. After approval, inject `ResourceRef::KernelDomain { domain: tool_name }` with `Rights::EXECUTE` into the agent's runtime CSpace so subsequent calls pass the gate automatically.

## Open Questions

1. **Should approval persist across sessions?** Current design: no. Each new session starts with the CSpace from the template. If users keep approving the same tool, consider adding it to the persona/role template.

2. **Should the agent know it was approved?** Yes — the tool call succeeds and returns a result. The agent doesn't need to know it was escalated.

3. **What about the `always_on` gate fix?** The bug where `web_search` was denied despite being always-on is fixed separately (adding it to the gate's skip list). This RFC handles the case where a tool is genuinely not in the CSpace — e.g., `exec` for a read-only agent.

4. **Multi-channel?** Web first. Telegram/CLI follow the same event pattern — render inline keyboard or stdin prompt, resolve via the same pending registry.
