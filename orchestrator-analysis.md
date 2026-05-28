# Orchestrator Response Analysis

> Thorough analysis of how the Oxios orchestrator produces responses, what metadata it generates, and how those flow to channels.

## 1. How the Orchestrator Currently Formats Responses for Channels

The orchestrator does **not** directly interact with channels. It returns a structured `OrchestrationResult` to its caller, and the **Gateway** translates it into an `OutgoingMessage` for channel delivery.

### Response formatting functions

The orchestrator produces the `response: String` field via three helper functions depending on the path taken:

| Path | Formatter | Output |
|------|-----------|--------|
| Non-task (chat/greeting) | Inline string | `interview.chat_response` or `"Hello! How can I help you today?"` |
| Interview needs clarification | `format_questions()` | `"I'd like to understand your request better. Could you help clarify:\n\n1. ...\n2. ..."` |
| Single-agent execution | `format_execution_result()` | `"✅ '<goal>'"` or `"⚠️ '<goal>을(를) 시도했지만 완전히 성공하지 못했습니다."` + truncated output (500 char preview) |
| Multi-agent execution | `format_result_combined()` | `"Multi-agent execution completed:\n\n<combined results>"` |

**Key observation:** The `response` field is always a **human-readable string** formatted for display. The raw `output` (agent's full text output) is carried separately in the `output: Option<String>` field.

### Gateway → Channel translation

The Gateway (`gateway.rs::dispatch()`) constructs an `OutgoingMessage` from the `OrchestrationResult`:

```rust
// From gateway.rs dispatch()
let outgoing = OutgoingMessage::with_id_and_metadata(
    msg.id,                          // preserves request message ID
    &msg.channel,
    &msg.user_id,
    &orchestration.response,         // human-readable formatted string
    response_metadata,               // HashMap<String, String>
);
```

The metadata HashMap is populated with:
- `"session_id"` → from `orchestration.session_id`
- `"space_id"` → from `orchestration.space_id` (UUID to string)
- `"phase"` → from `orchestration.phase_reached.to_string()`
- `"evaluation_passed"` → from `orchestration.evaluation_passed.to_string()`

**Not included in gateway metadata** (but available in OrchestrationResult):
- `seed_id` — present in `OrchestrationResult` but NOT added to gateway metadata
- `agent_id` — present in `OrchestrationResult` but NOT added to gateway metadata
- `space_tag` — present in `OrchestrationResult` but NOT added to gateway metadata
- `output` — raw agent output, NOT forwarded through gateway metadata

### Web channel consumption

The web channel's `ChatResponse` JSON body returns:
```json
{
  "id": "msg-uuid",
  "echo": "user's original message",
  "reply": "formatted response string",
  "session_id": "session-uuid | null",
  "space_id": "space-uuid | null",
  "phase": "interview | execute | null"
}
```

WebSocket sends two JSON chunks per response:
1. `{ type: "token", content, session_id, space_id }`
2. `{ type: "done", session_id, space_id, phase, evaluation_passed }`

## 2. Metadata Already Produced

### OrchestrationResult structure (full)

```rust
pub struct OrchestrationResult {
    pub session_id: Option<String>,       // UUID for multi-turn
    pub space_id: Option<Uuid>,           // Space partition ID
    pub space_tag: Option<String>,        // e.g. "[🔧 oxios]"
    pub response: String,                 // Human-readable formatted text
    pub seed_id: Option<Uuid>,            // Ouroboros seed ID
    pub agent_id: Option<AgentId>,        // Always None currently
    pub phase_reached: Phase,             // Interview | Seed | Execute | Evaluate | Evolve
    pub evaluation_passed: bool,          // true/false (false if skipped)
    pub output: Option<String>,           // Raw agent output
}
```

### What's populated in practice

| Field | Interview (non-task) | Interview (clarification) | Execute (single) | Execute (multi-agent) |
|-------|---------------------|--------------------------|-------------------|-----------------------|
| `session_id` | ✅ Some(uuid) | ✅ Some(uuid) | ✅ Some(uuid) | ✅ Some(uuid) |
| `space_id` | ✅ Some(uuid) | ✅ Some(uuid) | ✅ Some(uuid) | ✅ Some(uuid) |
| `space_tag` | ✅ Some(tag) | ✅ Some(tag) | ✅ Some(tag) | ✅ Some(tag) |
| `response` | ✅ chat text | ✅ questions | ✅ formatted result | ✅ combined result |
| `seed_id` | ❌ None | ❌ None | ✅ Some(uuid) | ✅ Some(uuid) |
| `agent_id` | ❌ None | ❌ None | ❌ None | ❌ None |
| `phase_reached` | Phase::Interview | Phase::Interview | Phase::Execute | Phase::Execute |
| `evaluation_passed` | false | false | bool (exec success) | bool (all passed) |
| `output` | ❌ None | ❌ None | ✅ Some(raw) | ✅ Some(combined) |

### CLI `--json` output shape (cmd_run.rs)

The CLI adds extra metadata not available to channels:
```json
{
  "response": "...",
  "session_id": "uuid | null",
  "space_id": "uuid-string | null",
  "space_tag": "string | null",
  "seed_id": "uuid-string | null",
  "agent_id": "uuid-string | null",
  "phase_reached": "interview | seed | execute",
  "evaluation_passed": true,
  "exit_code": 0,
  "duration_ms": 3500
}
```

**Note:** The CLI gets `seed_id`, `agent_id`, `space_tag`, and `duration_ms` from `OrchestrationResult` directly. Channels do NOT receive `seed_id` or `agent_id` through the gateway.

### Session persistence (state store)

Both the POST and WebSocket handlers persist `AgentResponse` with:
```rust
AgentResponse {
    content: response.content,
    session_id: Some(...),
    seed_id: metadata.get("seed_id").cloned(),  // always None from gateway!
    phase_reached: metadata.get("phase").cloned(),
    evaluation_passed: metadata.get("evaluation_passed").and_then(|v| v.parse().ok()),
    timestamp: chrono::Utc::now(),
}
```

**Bug:** `seed_id` in `AgentResponse` is always `None` because the gateway does not include `seed_id` in the metadata HashMap.

## 3. Current Response Type/Structure

### Type chain

```
Orchestrator::handle_message()
  → Result<OrchestrationResult>           (oxios-kernel)

Kernel::execute_prompt_with_session()
  → Result<OrchestrationResult>           (passthrough)

Gateway::dispatch()
  → OutgoingMessage::with_id_and_metadata (oxios-gateway)
    .content = orchestration.response     (String)
    .metadata = HashMap<String, String>   (session_id, space_id, phase, evaluation_passed)

Channel::send()
  → OutgoingMessage                       (channel-specific rendering)
```

### Phase enum values

```rust
pub enum Phase {
    Interview,   // "interview"
    Seed,        // "seed"
    Execute,     // "execute"
    Evaluate,    // "evaluate"  (not currently reached in handle_message)
    Evolve,      // "evolve"    (not currently reached in handle_message)
}
```

**Note:** `Phase::Evaluate` and `Phase::Evolve` are defined in the Ouroboros protocol but are never set as `phase_reached` in the current `handle_message()` implementation. The orchestrator only publishes events for these phases but the return value only reaches `Interview` or `Execute`.

### ExecutionResult (from Ouroboros)

```rust
pub struct ExecutionResult {
    pub output: String,           // Full text output from agent
    pub steps_completed: usize,   // Number of tool-calling steps
    pub success: bool,            // Whether execution succeeded
}
```

## 4. How Errors Flow from Orchestrator to Channels

### Error propagation chain

```
Orchestrator::handle_message()
  → returns Result<OrchestrationResult>
  → Err(anyhow::Error) on any failure

Gateway::dispatch()
  → match on result:
    Ok(orchestration) → OutgoingMessage with metadata
    Err(e) → OutgoingMessage with error content
```

### Error response formatting

When the orchestrator returns `Err`, the gateway constructs:

```rust
// gateway.rs dispatch() — error path
let outgoing = OutgoingMessage::with_id(
    msg.id,
    &msg.channel,
    &msg.user_id,
    format!("An error occurred: {e}"),   // Raw error message to user
);
```

**Key differences from success path:**
- No metadata is attached (no `session_id`, `phase`, `evaluation_passed`)
- Error message is a plain string with no structure
- The original `anyhow::Error` is stringified directly — no error code, no classification
- The error is logged at `error!` level before sending

### Where errors originate

Errors can come from any phase:

| Phase | Error source | Example |
|-------|-------------|---------|
| Interview | `ouroboros.interview()` | LLM provider failure, timeout |
| Seed | `ouroboros.generate_seed()` | LLM provider failure |
| Execute | `lifecycle.spawn_and_run()` | Agent runtime failure, tool error |
| Delegate | A2A / lifecycle execution | Subtask failure, circuit breaker open |
| State | `state_store.save_json()` | Disk I/O failure |

All are unified as `anyhow::Error` — no typed error hierarchy.

### Web channel error handling

**POST handler:**
```rust
// chat.rs — gateway send_and_wait failure
Err(e) => Err(AppError::Internal("gateway response failed".into()))
```
Returns a generic 500 error. The original error message from the orchestrator is lost — only the gateway-level error is surfaced.

**WebSocket handler:**
If `channel.send()` fails, the WebSocket loop breaks silently (`break`). No error message is sent to the frontend.

### Timeout / cancellation

There is no explicit timeout on `handle_message()`. The `config.security.max_execution_time_secs` is set on the `AgentLifecycleManager` but not on the orchestrator itself. Long-running orchestrations block the gateway dispatch task until completion (limited by the concurrency semaphore).

## Summary of Findings

### Gaps and Issues

1. **Missing gateway metadata:** `seed_id`, `agent_id`, and `space_tag` are in `OrchestrationResult` but NOT forwarded through gateway metadata. This means channels (web, telegram) never see seed IDs.

2. **Agent ID always None:** `agent_id` is never populated in any code path — the field exists but is unused.

3. **Evaluate/Evolve phases unreachable:** These phases exist in the enum but are never set as `phase_reached`. The orchestrator only returns `Interview` or `Execute`.

4. **Error responses lack structure:** Error messages are plain strings with no metadata. No error codes, no session_id (so multi-turn fails on error).

5. **No duration tracking in orchestrator:** `cmd_run.rs` measures `duration_ms` itself. The `OrchestrationResult` has no timing field.

6. **`steps_completed` from ExecutionResult is lost:** The number of agent steps is available from `ExecutionResult` but not included in `OrchestrationResult`.

7. **CLI has richer output than channels:** The CLI's JSON output includes `duration_ms`, `exit_code`, `seed_id`, `agent_id`, and `space_tag` — none of which reach channels through the gateway.
