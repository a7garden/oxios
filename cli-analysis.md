# CLI Channel Analysis

## Source Files Analyzed

| File | Lines | Purpose |
|------|-------|---------|
| `channels/oxios-cli/Cargo.toml` | 17 | Dependencies |
| `channels/oxios-cli/src/lib.rs` | 15 | Public API surface |
| `channels/oxios-cli/src/channel.rs` | 148 | Channel trait impl, mpsc bridge, session handle |
| `channels/oxios-cli/src/commands.rs` | 112 | Meta-command parser (`.quit`, `.help`, `.reset`, etc.) |
| `channels/oxios-cli/src/interactive.rs` | 132 | Readline REPL loop (reedline) |
| `channels/oxios-cli/src/plugin.rs` | 46 | ChannelPlugin factory for daemon mode |
| `channels/oxios-cli/src/session.rs` | 60 | Session struct (id, timestamps, message count) |
| `src/cmd_run.rs` | 160 | `oxios run` subcommand (single-shot execution) |

Supporting files also read:
- `crates/oxios-gateway/src/gateway.rs` — Gateway dispatch logic
- `crates/oxios-gateway/src/message.rs` — IncomingMessage / OutgoingMessage types
- `crates/oxios-gateway/src/channel.rs` — Channel trait definition
- `crates/oxios-kernel/src/orchestrator.rs` — OrchestrationResult struct

---

## 1. How CLI Sends Messages and Receives Responses

There are **two distinct code paths** for CLI → Kernel communication:

### Path A: `oxios run` (single-shot, `src/cmd_run.rs`)

```
User prompt → cmd_run() → kernel.execute_prompt_with_session()
                               ↓
                        orchestrator.handle_message("cli", prompt, session_id)
                               ↓
                        OrchestrationResult (awaited directly)
                               ↓
                        println!() to stdout
```

This is a **direct function call** to the kernel. It bypasses the gateway entirely. The call is `await`ed synchronously within the async runtime. The result is printed to stdout as either:
- **JSON** (`--json` flag): Structured object with `response`, `session_id`, `space_id`, `space_tag`, `seed_id`, `agent_id`, `phase_reached`, `evaluation_passed`, `exit_code`, `duration_ms`.
- **Human-readable** (default): Just the response text, plus optional seed/session IDs and evaluation warnings to stderr.

### Path B: `oxios chat` (interactive REPL, `channels/oxios-cli/`)

```
User types line → InteractiveLoop::run()
                        ↓
                MetaCommand::parse() → if dot-command, handle locally
                        ↓ (else)
                CliChannelHandle::send_user_message()
                        ↓
                mpsc::Sender<IncomingMessage> → CliChannel::start() loop
                        ↓
                GatewayInbox (channel_name, IncomingMessage) → Gateway::run()
                        ↓
                Gateway::dispatch() → tokio::spawn → orchestrator.handle_message()
                        ↓
                OutgoingMessage → Channel::send() → CliChannel::send()
                        ↓
                println!("{}", msg.content)
```

The flow is:
1. `InteractiveLoop` reads a line via `reedline`
2. If not a meta-command, calls `handle.send_user_message(content)` which pushes an `IncomingMessage` into an mpsc channel
3. `CliChannel::start()` (a background task) receives from that mpsc and forwards into the Gateway's shared mpsc
4. Gateway's `dispatch()` spawns a task that calls `orchestrator.handle_message()`
5. On completion, the Gateway calls `channel.send(OutgoingMessage)` which in `CliChannel` simply does `println!()`

---

## 2. Fire-and-Forget vs Correlated

### `oxios run` — **Correlated (synchronous)**
The call is fully synchronous (from the caller's perspective): `cmd_run` awaits the orchestrator, gets back the `OrchestrationResult`, and prints it. There is a 1:1 request-response relationship. The process then exits.

### `oxios chat` — **Fire-and-forget (asynchronous)**
The interactive loop sends a message and **does not wait** for a response. The code comment in `interactive.rs` explicitly states:

```rust
// NOTE: The response will arrive asynchronously via the
// Channel::send() implementation (printed to stdout).
// In a future iteration, we could wait for a response here
// for a synchronous feel, but for now the gateway routes
// the response back through the channel.
```

The Gateway does correlate messages — `OutgoingMessage::with_id()` preserves the original `msg.id` — but the interactive loop doesn't use this correlation. The response simply arrives asynchronously and is printed to stdout whenever the orchestration finishes.

**Implication:** The user can type another message before the previous response arrives. The Gateway dispatches each message independently (concurrent tasks). Responses may arrive out of order.

---

## 3. Error Handling and Display

### `oxios run`
- **File read errors** (`--context-file`): Returns `anyhow::anyhow!("failed to read context file '{}': {}", path, e)` — exits with error message.
- **Empty stdin**: Returns `anyhow::anyhow!("stdin is empty, no context to read")`.
- **Orchestration failure**: The `?` operator propagates errors up to main, which will print them via anyhow's default handler.
- **Evaluation failure**: If `--exit-code` is set and `evaluation_passed` is false, the process returns exit code 1. A warning is printed to stderr: `⚠️  Evaluation did not fully pass.` with optional notes.
- **Audit**: Every `run` call is audited via `kernel.handle().security.audit()` with the first 100 chars of the prompt.

### `oxios chat`
- **Readline errors**: Logged via `tracing::error!` and the loop breaks.
- **Message send failure**: `send_user_message` returns `Err` propagated with `anyhow!` — the loop's `?` will break.
- **Gateway dispatch failure**: On orchestration error, the Gateway constructs an error message: `"An error occurred: {e}"` and sends it back through the channel. The user sees it as a printed line.
- **Channel send failure** (within Gateway): Logged via `tracing::error!` — **silently dropped** from user's perspective.
- **Channel not found**: Logged via `tracing::warn!` — silently dropped.

**Gaps:**
- No user-visible error formatting in `oxios chat`. Errors appear as plain text lines with no visual distinction from normal responses.
- No timeout handling — if the orchestrator hangs, the user gets no feedback.

---

## 4. Session Handling

### `oxios run`
- **`--session <ID>`**: Passes an existing session ID to continue a multi-turn conversation. The session ID comes from a previous run's JSON output.
- **No session ID**: Starts a new session. The response includes a `session_id` that can be used for follow-ups.
- **Persistence**: Sessions live in the orchestrator's in-memory `ConversationBuffer` (inside the Space manager). They are **ephemeral** — lost on process restart. The AGENTS.md confirms: "Sessions live in orchestrator memory. Process restart loses them."
- **JSON output**: Includes `session_id` for programmatic chaining.

### `oxios chat`
- **In-process session**: `Session` struct (`session.rs`) tracks `id` (UUID v4), `label`, `created_at`, `last_active`, `message_count`.
- **Session ID propagation**: `send_user_message()` injects `session_id` into `IncomingMessage.metadata`.
- **Reset**: `.reset` command creates a new session UUID, but this only resets the CLI-side tracker. It does NOT reset the kernel-side conversation buffer (no message is sent to the kernel to clear its state).
- **Touch**: Every message increments `message_count` and updates `last_active` — purely cosmetic.
- **Not persisted**: The `Session` struct is `Arc<std::sync::Mutex<Session>>` in memory only.

**Gap:** The `.reset` meta-command only resets the CLI-side session UUID. The kernel-side conversation state may persist based on the new session ID eventually resolving to a different Space/buffer, but the old conversation isn't explicitly cleaned up.

---

## 5. Space Support

### `oxios run`
Full Space support via `OrchestrationResult`:
- `space_id`: The Space that handled the message (if any)
- `space_tag`: Human-readable tag (e.g., `"[🔧 oxios]"`)
- Included in JSON output and used for audit/context

The orchestrator's `handle_message()` performs Space detection/creation via the SpaceManager. The CLI doesn't control which Space is used — it's automatic.

### `oxios chat`
- **No explicit Space support**. The interactive loop doesn't set any Space metadata on `IncomingMessage`.
- The Gateway's `dispatch()` extracts `session_id` from metadata but not Space. Space resolution happens server-side in the orchestrator.
- The `OutgoingMessage` from the Gateway includes `space_id` and `space_tag` in its metadata, but `CliChannel::send()` ignores all metadata and just prints `msg.content`.

**Gap:** Space metadata is available in the response but not displayed to the interactive user.

---

## 6. Streaming Support

### `oxios run`
**No streaming.** The entire orchestration runs to completion, then the full response is printed at once.

### `oxios chat`
**No streaming.** The response arrives as a single `OutgoingMessage` and is printed in one `println!()` call.

The underlying engine (`OxiosEngine` wrapping `oxi_sdk::Oxi`) may support streaming at the SDK level, but neither CLI path uses it. The Gateway dispatch waits for the full `OrchestrationResult` before calling `channel.send()`.

---

## 7. Progress Indicators

### `oxios run`
- **Duration tracking**: `std::time::Instant` measures total execution time, included in JSON output as `duration_ms`.
- **No spinner/progress bar**: The process blocks silently until complete.
- **Logging**: `tracing::info!` logs prompt length and session ID.

### `oxios chat`
- **No progress indicator at all.** After the user sends a message, there is no visual feedback until the response arrives.
- The interactive loop is `async` but `reedline` is synchronous (blocking). The loop calls `self.handle.send_user_message()` and immediately goes back to `editor.read_line()`.
- No spinner, no "thinking..." message, no status indicator.

---

## Summary Table

| Feature | `oxios run` | `oxios chat` |
|---------|-------------|--------------|
| **Communication** | Direct kernel call | Gateway mpsc pipeline |
| **Correlation** | Synchronous (1:1) | Fire-and-forget (async) |
| **Error display** | anyhow propagation + stderr warnings | Plain println, no formatting |
| **Session** | `--session` flag, ephemeral | In-memory UUID, `.reset` command |
| **Space** | Auto-detected, shown in JSON | Auto-detected, response metadata ignored |
| **Streaming** | None | None |
| **Progress** | Duration timer (JSON only) | None |
| **Meta-commands** | N/A | `.quit`, `.help`, `.reset`, `.model`, `.persona`, `.clear` |

## Notable Gaps & Observations

1. **No response correlation in interactive mode**: The user can type faster than responses arrive. Messages are processed concurrently by the Gateway. No sequence numbers or ordering guarantees.

2. **`.reset` is cosmetic**: Only resets the CLI-side session UUID. The kernel's conversation buffer is not explicitly cleared.

3. **`.model` and `.persona` are stubs**: Both print a message but have `// TODO: wire to kernel model switching` comments. They don't actually change anything.

4. **No streaming**: Both paths wait for full orchestration completion. For long-running agent tasks, this means the user sees nothing for potentially minutes.

5. **No progress feedback in chat**: After sending a message in `oxios chat`, there's no "thinking..." or spinner. The prompt reappears immediately (reedline doesn't block on the response), which could be confusing.

6. **Session metadata lost on response**: The `OutgoingMessage` carries rich metadata (session_id, space_id, phase, evaluation_passed) but `CliChannel::send()` discards it all and only prints `content`.

7. **No retry/resilience**: If a message fails in the Gateway dispatch, it's logged and dropped. No retry, no dead letter queue.

8. **`oxios run` bypasses Gateway entirely**: It calls `kernel.execute_prompt_with_session()` directly. This means it doesn't go through the channel plugin system, doesn't benefit from Gateway's concurrency semaphore, and doesn't trigger channel-level lifecycle events.
