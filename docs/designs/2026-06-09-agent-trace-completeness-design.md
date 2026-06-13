# Agent Trace Completeness — Remaining Gaps

> **Date:** 2026-06-09
> **Status:** Design
> **Scope:** Per-step timestamps, project_id, token/cost tracking in `AgentInfo`

## Background

The initial agent trace implementation (same date) resolved three core issues:

1. API routes `/api/agents/{id}`, `/trace`, `/logs` were never registered
2. Handlers returned hardcoded empty responses
3. `ExecutionResult` data was discarded after completion — never stored in `AgentInfo`

Three gaps remain. This document specifies how to close them.

---

## Gap 1: Per-Step Timestamps

### Problem

`handle_agent_logs` synthesizes log entries from `AgentInfo.tool_calls`, but every
tool call gets the same timestamp (`agent.started_at`). Individual step timestamps
are not captured anywhere in the pipeline:

```
TrajectoryStep { input, output, duration_ms, confidence }  ← no timestamp
```

### Design

**Where to capture:** `ToolExecutionStart` fires with `Utc::now()` available.
The `ExecuteState` already has `tool_call_ids`, `tool_args_map`, `tool_error_map`
keyed by `tool_call_id`. Add a parallel `tool_timestamps` map.

**Changes:**

#### 1. `ExecuteState` (agent_runtime.rs)

```rust
struct ExecuteState {
    // ... existing fields ...
    /// Per-step start timestamp (UTC).
    tool_timestamps: HashMap<String, chrono::DateTime<chrono::Utc>>,
}
```

#### 2. `ToolExecutionStart` callback

```rust
AgentEvent::ToolExecutionStart { tool_call_id, .. } => {
    // ... existing code ...
    s.tool_timestamps.insert(tool_call_id.clone(), chrono::Utc::now());
}
```

#### 3. `run_agent` return type — add `tool_timestamps`

Return tuple gains `HashMap<String, DateTime<Utc>>`, following the same pattern
as `tool_args_map` and `tool_error_map`.

#### 4. `types::ToolCallRecord` — add `timestamp`

```rust
pub struct ToolCallRecord {
    pub tool: String,
    pub input: String,
    pub output: String,
    pub duration_ms: u64,
    pub is_error: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,  // ← new
}
```

This is the kernel-internal type. `oxios_ouroboros::ToolCallRecord` gets the same
field for consistency.

#### 5. Supervisor mapping

When copying `ExecutionResult.tool_calls` → `AgentInfo.tool_calls`, the timestamp
from `tool_timestamps` (keyed by `tool_call_id`) is mapped to `ToolCallRecord.timestamp`.

**Note:** `tool_call_id` is not currently in `ToolCallRecord`. We add it:

```rust
pub struct ToolCallRecord {
    pub tool: String,
    pub input: String,
    pub output: String,
    pub duration_ms: u64,
    pub is_error: bool,
    pub tool_call_id: String,              // ← new (for correlation)
    pub timestamp: Option<DateTime<Utc>>,  // ← new
}
```

#### 6. API handler `handle_agent_logs`

Replace `agent.started_at` with per-step `tc.timestamp`:

```rust
for (i, tc) in agent.tool_calls.iter().enumerate() {
    let ts = tc.timestamp
        .map(|t| t.to_rfc3339())
        .unwrap_or_default();
    entries.push(json!({
        "timestamp": ts,
        "level": if tc.is_error { "error" } else { "info" },
        "message": format!("[Step {}] {} ({}) → {}", ...),
    }));
}
```

---

## Gap 2: `project_id` Always `null`

### Problem

`handle_agent_get` returns `"project_id": null` hardcoded. The orchestrator
does project detection and stores the result in `OrchestrationResult.primary_project_id`,
but this never reaches `AgentInfo` because `spawn_and_run` only receives a `Seed`,
and `Seed` has no `project_id` field.

### Data flow (current)

```
Orchestrator
  ├── project detection → primary_project_id
  ├── spawn_and_run(seed) → AgentLifecycleManager
  │     └── supervisor.fork(seed)  → AgentInfo (no project_id)
  │     └── supervisor.run_with_seed(id, seed)
  └── OrchestrationResult { primary_project_id, agent_id: None, ... }
```

The `agent_id` is also `None` in the result because `spawn_and_run` manages
fork+run internally and doesn't expose the forked agent ID.

### Design

**Approach:** Add `project_id: Option<Uuid>` to `Seed`. The orchestrator sets
it before calling `spawn_and_run`. `BasicSupervisor.fork()` copies it into `AgentInfo`.

This is the minimal change because `Seed` already carries metadata from the
orchestrator to the supervisor. Adding project_id follows the existing pattern
(`cspace_hint` is an orchestrator→runtime hint already).

#### 1. `Seed` (oxios-ouroboros/src/seed.rs)

```rust
pub struct Seed {
    // ... existing fields ...
    /// Project ID detected by the orchestrator, passed through to AgentInfo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<uuid::Uuid>,
}
```

`#[serde(skip_serializing_if)]` ensures backward compatibility — existing
serialized seeds without this field deserialize as `None`.

#### 2. Orchestrator — set `project_id` on seed before execution

In all three orchestration paths (ouroboros, chat, direct), set `seed.project_id`
after detection:

```rust
let mut seed = Seed::from_message(user_message);
seed.project_id = primary_project_id;
let result = self.lifecycle.spawn_and_run(&seed, Priority::Normal).await?;
```

#### 3. `BasicSupervisor.fork()` — copy `project_id`

```rust
let info = AgentInfo {
    // ... existing fields ...
    project_id: spec.project_id,
};
```

#### 4. `AgentInfo` — add `project_id` field

```rust
pub struct AgentInfo {
    // ... existing fields ...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<uuid::Uuid>,
}
```

#### 5. `handle_agent_get` — use actual value

```rust
"project_id": agent.project_id.map(|id| id.to_string()),
```

---

## Gap 3: `tokens_used: 0`, `cost_usd: 0.0`

### Problem

Token usage is tracked in two places:

1. `ExecuteState` callback for `AgentEvent::Usage` — records to `cost_tracker`
   and publishes `KernelEvent::TokenUsageUpdate`
2. `cost_tracker()` — global singleton, keyed by `(agent_label, model)`

But `ExecuteState` never accumulates the totals, and `AgentInfo` has no
fields for them. The API handler returns `0` / `0.0` hardcoded.

### Design

**Approach:** Accumulate token counts in `ExecuteState`, compute cost, propagate
through `ToolCallRecord` (no — those are per-step). Instead, add usage fields to
a new return type or extend `ExecutionResult`.

The cleanest path: `ExecuteState` accumulates `total_input_tokens` and
`total_output_tokens`. These are returned alongside the existing `run_agent`
tuple and stored in `AgentInfo`.

#### 1. `ExecuteState` — accumulate tokens

```rust
struct ExecuteState {
    // ... existing fields ...
    total_input_tokens: u64,
    total_output_tokens: u64,
}
```

In the `AgentEvent::Usage` callback:

```rust
AgentEvent::Usage { input_tokens, output_tokens } => {
    s.total_input_tokens += input_tokens as u64;
    s.total_output_tokens += output_tokens as u64;
    // ... existing cost_tracker recording stays ...
}
```

#### 2. `run_agent` return type — add token totals

Return tuple gains `(u64, u64)` for `(input_tokens, output_tokens)`.

#### 3. `ExecutionResult` — add usage fields

```rust
pub struct ExecutionResult {
    pub output: String,
    pub steps_completed: usize,
    pub success: bool,
    pub tool_calls: Vec<ToolCallRecord>,
    /// Total input tokens consumed during execution.
    #[serde(default)]
    pub tokens_input: u64,
    /// Total output tokens generated during execution.
    #[serde(default)]
    pub tokens_output: u64,
}
```

`#[serde(default)]` for backward compat.

#### 4. `AgentInfo` — add usage fields

```rust
pub struct AgentInfo {
    // ... existing fields ...
    /// Total input tokens consumed.
    #[serde(default)]
    pub tokens_input: u64,
    /// Total output tokens generated.
    #[serde(default)]
    pub tokens_output: u64,
    /// Estimated cost in USD.
    #[serde(default)]
    pub cost_usd: f64,
}
```

#### 5. Supervisor — compute cost and store

```rust
// In run_with_seed, after successful execution:
let cost_usd = crate::kernel_handle::engine_api::estimate_cost(
    &model_id, // need to thread this through
    result.tokens_input,
    result.tokens_output,
);
agent.tokens_input = result.tokens_input;
agent.tokens_output = result.tokens_output;
agent.cost_usd = cost_usd;
```

**Threading `model_id`:** The supervisor doesn't currently know which model
was used. Options:

- **A.** Add `model_id: String` to `ExecutionResult` — set by `AgentRuntime.execute()`
- **B.** Compute cost inside `AgentRuntime` and return `cost_usd` directly

**Option A** is better because `model_id` is useful metadata on its own (UI can
show "executed with claude-sonnet-4").

```rust
pub struct ExecutionResult {
    // ... existing fields ...
    /// Model ID used for this execution.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model_id: String,
}
```

#### 6. `handle_agent_get` — use actual values

```rust
"tokens_used": agent.tokens_input + agent.tokens_output,
"cost_usd": agent.cost_usd,
"model_id": agent.model_id,
```

---

## Summary of All Changes

### Files to modify

| File | Changes |
|------|---------|
| `oxios-ouroboros/src/protocol.rs` | `ToolCallRecord`: add `tool_call_id`, `timestamp`. `ExecutionResult`: add `tokens_input`, `tokens_output`, `model_id` |
| `oxios-ouroboros/src/seed.rs` | `Seed`: add `project_id: Option<Uuid>` |
| `oxios-kernel/src/types.rs` | `AgentInfo`: add `project_id`, `tokens_input`, `tokens_output`, `cost_usd`, `model_id`. `ToolCallRecord`: add `tool_call_id`, `timestamp` |
| `oxios-kernel/src/agent_runtime.rs` | `ExecuteState`: add `tool_timestamps`, `total_input_tokens`, `total_output_tokens`. `run_agent`: return new fields. `ToolExecutionStart`: capture timestamp. `AgentEvent::Usage`: accumulate tokens. Build `ToolCallRecord` with `tool_call_id` + `timestamp`. `ExecutionResult`: populate `model_id`, `tokens_*` |
| `oxios-kernel/src/supervisor.rs` | `fork()`: copy `project_id` from `Seed`. `run_with_result`: store token/cost data, `model_id`. Map new `ToolCallRecord` fields |
| `oxios-kernel/src/orchestrator.rs` | Set `seed.project_id` before `spawn_and_run` in all 3 paths |
| `oxios-web/src/routes/system.rs` | `handle_agent_get`: real `project_id`, `tokens_used`, `cost_usd`, `model_id`. `handle_agent_logs`: per-step timestamps from `tc.timestamp`. `handle_agent_trace`: per-step timestamps |
| `oxios-web/web/src/types/agent.ts` | `AgentDetail`: add `model_id` |
| `oxios-web/web/src/routes/agents/$agentId.tsx` | Show model, tokens, cost in meta card |
| `oxios-kernel/tests/*` | Update `AgentInfo` constructors with new fields |
| `oxios-ouroboros/tests/*` | Update `ExecutionResult` / `ToolCallRecord` constructors |

### Backward compatibility

All new fields use `#[serde(default)]` or `#[serde(skip_serializing_if)]`:

- Old clients ignore new fields
- New clients gracefully handle old data (defaults to `None`/`0`/`""`)
- Existing serialized `Seed`, `ExecutionResult`, `ToolCallRecord` deserialize
  without error

### Implementation order

1. **Per-step timestamps** — self-contained, no cross-crate type changes beyond
   `ToolCallRecord` (already modified in this session)
2. **Project ID** — touches `Seed` (oxios-ouroboros) + `AgentInfo` (kernel) +
   orchestrator wiring
3. **Token/cost tracking** — extends `ExecutionResult` (oxios-ouroboros) +
   `ExecuteState` + supervisor cost computation

Each step is independently deployable and testable.
