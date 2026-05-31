# Correlation ID Design

**Date:** 2026-05-31  
**Status:** Design (not yet implemented)  
**Scope:** Request tracing from channel → orchestrator → agent → tools

---

## 1. Current State

### 1.1 What Exists

| Component | ID Type | Scope |
|-----------|---------|-------|
| `session_id` | `Uuid` | Orchestrator session (multi-turn conversation) |
| `seed_id` | `Uuid` | Ouroboros seed spec (task definition) |
| `agent_id` | `Uuid` | Agent instance (forked process) |
| `request_id` | `Uuid` | A2A message envelope (agent-to-agent) |
| `task_id` | `Uuid` | Scheduler task (queued work item) |

### 1.2 What's Missing

There is **no single correlation ID** that flows from the initial user message through the entire processing pipeline. Each component generates its own ID independently.

**Current flow:**

```
User Message (HTTP/WebSocket/Telegram)
    │
    ├─ No request ID assigned at entry
    │
    ├─ Gateway → no ID
    │
    ├─ Orchestrator → session_id (new or resumed)
    │   │
    │   ├─ Phase: Interview → seed_id created
    │   │
    │   ├─ Phase: Execute → agent_id created
    │   │   │
    │   │   └─ AgentRuntime → tracer().start("seed-{seed_id}")
    │   │       │
    │   │       ├─ Tool calls → no parent context
    │   │       ├─ LLM calls → no parent context
    │   │       └─ Memory recall → no parent context
    │   │
    │   └─ Phase: Evaluate
    │
    └─ Response → session_id returned, but no trace of the full journey
```

### 1.3 The Gap

When debugging "why did agent X take action Y for user message Z", there's no way to:

1. Look up a single ID and see every step
2. Correlate log lines across orchestrator → agent_runtime → tools
3. Track a request from HTTP entry to final response
4. Measure end-to-end latency for a single user message

---

## 2. Design

### 2.1 Concept: `request_id` (Correlation ID)

A single `Uuid` assigned at the **gateway entry point** that flows through every processing stage.

**Properties:**
- Assigned when the user message enters the system (HTTP handler, WebSocket, Telegram)
- Propagated through: Gateway → Orchestrator → AgentRuntime → Tools
- Included in every `tracing::info!()` call as a structured field
- Logged in the AuditTrail
- Returned in API responses

### 2.2 Data Flow

```
User Message
    │
    ├─ [Web] HTTP handler / WebSocket handler
    │   └─ request_id = Uuid::new_v4()
    │   └─ tracing::info!(request_id = %request_id, "Incoming message")
    │   └─ Store in request extensions or channel metadata
    │
    ├─ [CLI] cmd_run handler
    │   └─ request_id = Uuid::new_v4()
    │
    ├─ [Telegram] message handler
    │   └─ request_id = Uuid::new_v4()
    │
    ▼
Gateway.send_to(channel, IncomingMessage { request_id, ... })
    │
    ▼
Orchestrator.handle_message(request_id, session_id, user_message)
    │
    ├─ tracing::info!(request_id = %request_id, session_id = %session_id, "Orchestrator handling message")
    │
    ├─ Phase: Interview
    │   └─ tracing::info!(request_id = %request_id, "Phase: interview")
    │
    ├─ Phase: Seed → seed_id
    │   └─ tracing::info!(request_id = %request_id, seed_id = %seed_id, "Seed created")
    │
    ├─ Phase: Execute → agent_id
    │   └─ tracing::info!(request_id = %request_id, agent_id = %agent_id, "Agent spawned")
    │   │
    │   ▼
    │   AgentRuntime.run(request_id, seed_id, ...)
    │       │
    │       ├─ tracer().start("seed-{seed_id}", SpanKind::Agent)
    │       │   └─ SpanContext includes request_id
    │       │
    │       ├─ Tool call → tracing::info!(request_id = %request_id, tool = "exec", ...)
    │       ├─ LLM call → tracing::info!(request_id = %request_id, model = %model, ...)
    │       └─ Memory recall → tracing::info!(request_id = %request_id, ...)
    │
    └─ Phase: Evaluate
        └─ tracing::info!(request_id = %request_id, score = %score, "Evaluation complete")
```

### 2.3 Implementation Points

#### A. Entry Point (Web)

```rust
// surface/oxios-web/src/routes/chat.rs
pub(crate) async fn handle_chat(
    state: State<Arc<AppState>>,
    Json(body): Json<ChatRequest>,
) -> Json<ChatResponse> {
    let request_id = uuid::Uuid::new_v4();
    tracing::info!(request_id = %request_id, "Chat request received");

    let result = state.kernel.orchestrator()
        .handle_message(Some(&session_id), &request_id.to_string(), &body.message, &user_id)
        .await;

    Json(ChatResponse {
        request_id: request_id.to_string(),
        ..result
    })
}
```

#### B. Gateway Message Extension

```rust
// crates/oxios-gateway/src/lib.rs
pub struct IncomingMessage {
    pub content: String,
    pub user_id: String,
    pub session_id: Option<String>,
    /// Correlation ID for request tracing.
    pub request_id: String,  // NEW
}
```

#### C. Orchestrator Signature

```rust
// crates/oxios-kernel/src/orchestrator.rs
pub async fn handle_message(
    &self,
    session_id: Option<&str>,
    request_id: &str,  // NEW parameter
    user_message: &str,
    user_id: &str,
) -> OrchestratorResult {
    tracing::info!(
        request_id = %request_id,
        session_id = %session_id.unwrap_or("new"),
        "Orchestrator handling message"
    );
    // ... propagate request_id through all phases
}
```

#### D. AgentRuntime Span Context

```rust
// crates/oxios-kernel/src/agent_runtime.rs
let _trace_guard = crate::observability::tracer()
    .start(format!("seed-{}", &seed_id.to_string()[..8]).as_str(), SpanKind::Agent);
// Add request_id to the span context
// (depends on oxi-sdk Tracer API supporting custom attributes)
```

#### E. Tracing Subscriber (Structured Fields)

All tracing calls should include `request_id` as a structured field. When using JSON log format, this becomes queryable:

```json
{"timestamp":"2026-05-31T12:00:00Z","level":"INFO","request_id":"a1b2c3d4-...","message":"Agent spawned","agent_id":"..."}
```

### 2.4 Propagation Through Tools

Each tool execution should include the `request_id` in its tracing context:

```rust
// crates/oxios-kernel/src/tools/gated_tool.rs
tracing::info!(
    request_id = %self.request_id,  // passed during tool construction
    tool = %self.tool_name,
    "Tool executing"
);
```

The `request_id` can be stored in the `ToolRegistry` or passed as context when building tools for a specific agent run.

### 2.5 Minimal Change Set

| File | Change |
|------|--------|
| `oxios-gateway/src/lib.rs` | Add `request_id: String` to `IncomingMessage` |
| `oxios-kernel/src/orchestrator.rs` | Add `request_id: &str` parameter to `handle_message()` |
| `oxios-kernel/src/agent_runtime.rs` | Accept `request_id`, include in span context |
| `surface/oxios-web/src/routes/chat.rs` | Generate `request_id`, pass to orchestrator |
| `channels/oxios-cli/src/channel.rs` | Generate `request_id` for CLI messages |
| `channels/oxios-telegram/src/lib.rs` | Generate `request_id` for Telegram messages |

### 2.6 Backward Compatibility

- `request_id` is a new parameter — all existing callers need updating
- Default: use `Uuid::new_v4().to_string()` if not provided
- API responses should include `request_id` for client-side correlation
- No database changes needed — `request_id` is for real-time tracing only

---

## 3. Future: Distributed Tracing Integration

When OTel is active, the `request_id` should become the OTel `trace_id`. This enables:

- Jaeger/Zipkin UI showing the full request flow
- Cross-service tracing (if agents call external services)
- Automatic span creation from `tracing::info!()` macros

The `tracing-opentelemetry` bridge automatically converts `tracing` spans with the `request_id` field into OTel span links. No additional code needed beyond including the field in `tracing::*!()` calls.

---

## 4. What NOT to Do

- ❌ Do NOT store `request_id` in the database — it's ephemeral
- ❌ Do NOT change the `AuditTrail` — it has its own hash chain
- ❌ Do NOT add `request_id` to agent memory — it's not agent-relevant
- ❌ Do NOT make `request_id` mandatory for tool calls — use `tracing` spans for propagation
