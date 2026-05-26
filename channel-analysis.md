# Oxios Gateway & Channel Architecture Analysis

> Date: 2026-05-26
> Scope: `crates/oxios-gateway/`, `channels/oxios-web/`, `channels/oxios-cli/`, `channels/oxios-telegram/`

---

## 1. Gateway Core (`crates/oxios-gateway/`)

### 1.1 Channel Trait (`channel.rs`)

A minimal, well-designed 3-method trait:

| Method   | Purpose                                          |
|----------|--------------------------------------------------|
| `name()` | Returns channel identifier (e.g., "web", "cli")  |
| `receive()` | Poll for next incoming message (returns `Option`) |
| `send()`    | Deliver an outgoing message to the channel's users |

**Strengths**: Clean, focused, `Send + Sync + async_trait`. No unnecessary methods.

**Weaknesses**: No `start()` / lifecycle hook. Channels must be "always ready" — setup/teardown is entirely on the plugin side. There's no way for the gateway to signal a channel to initialize or shut down gracefully beyond dropping it.

### 1.2 Message Types (`message.rs`)

Two structs: `IncomingMessage` and `OutgoingMessage`, both carrying `id`, `channel`, `user_id`, `content`, `timestamp`, and `metadata: HashMap<String, String>`.

- **Correlation**: `OutgoingMessage` can preserve the `IncomingMessage.id` via `with_id()`, which the gateway uses for request-response matching.
- **Metadata**: Free-form `HashMap<String, String>` — flexible but untyped. Session IDs, space IDs, phases, chat_ids, etc. all live as stringly-typed keys.

### 1.3 Plugin System (`plugin.rs`)

`ChannelPlugin` trait with a single `setup()` method that receives a `ChannelContext` (kernel handle + config + config path) and returns a `ChannelBundle` (channel + background tasks).

**Well-designed factory pattern.** Feature-gated at compile time. Each plugin gets everything it needs without importing concrete types.

### 1.4 Gateway Event Loop (`gateway.rs`)

The gateway runs a polling loop:
1. Snapshot channel names
2. Drain all pending messages from each channel (inner loop per channel)
3. Route each message through the orchestrator
4. Send response back to the originating channel
5. Adaptive sleep: `yield_now()` when busy, 50ms pause when idle

**Issues:**
- **Polling, not event-driven**: The `receive()` method is async but the gateway polls in a tight loop with 50ms sleep. This works but wastes cycles. An event-driven approach (e.g., channels notify the gateway via `tokio::notify`) would be more efficient.
- **Write lock for receive**: The gateway acquires a write lock on the channel map just to call `receive()`, even though `receive()` only reads. This serializes message reception across all channels.
- **Sequential routing**: Messages from all channels are drained sequentially. A burst on one channel can delay others.

---

## 2. Channel-by-Channel Analysis

### 2.1 Web Channel

#### Structure
```
WebPlugin → WebChannel (mpsc bridge) → Axum Server → HTTP/WS handlers
```

#### Implementation Quality: ★★★★☆

| Aspect | Details |
|--------|---------|
| **Channel trait** | Clean implementation. `receive()` reads from mpsc, `send()` correlates + broadcasts |
| **Dual-path responses** | HTTP POST uses `send_and_wait()` with oneshot correlation; WebSocket uses broadcast subscription |
| **Session persistence** | Both HTTP and WS paths persist sessions to StateStore — duplicated but complete |
| **Error propagation** | `AppError` enum with proper HTTP status mapping. Internal errors are sanitized to `"gateway response failed"` — no leak |
| **Streaming** | WebSocket supports token-by-token streaming (`"type": "token"` chunks + `"type": "done"` finalization) |
| **Auth** | Bearer token via middleware; WS uses query param `?token=` |
| **Rate limiting** | Per-IP rate limiter middleware |

#### Inconsistencies & Issues

1. **Duplicated session persistence logic**: `handle_chat()` (POST) and `persist_session()` (WS) contain nearly identical session creation/update code (~60 lines duplicated). This should be extracted to a shared function in the kernel or a helper module.

2. **user_id hardcoded in WS**: WebSocket always uses `"web-user"` as the user_id. The POST handler uses the client-provided `user_id` (defaulting to `"default"`). This inconsistency means session data from WS and HTTP have different user identities.

3. **WebChannel.send() double-writes**: The `send()` method both delivers to the oneshot handler AND broadcasts to subscribers. If the gateway calls `send()` for an HTTP response, the broadcast also fires — any WS client gets a copy of every HTTP response. This is by design (SSE subscribers get all responses) but could leak data across sessions.

4. **Missing `space_id` in IncomingMessage**: The POST handler puts `space_id` in metadata, but the WS handler only puts `session_id` and `space_id` — no `user_id` personalization possible.

5. **Static file serving with no caching**: `Cache-Control: no-cache` on all assets, reading from filesystem every request. Intentional for auto-update, but bad for production performance.

6. **Auto-update complexity**: The `plugin.rs` is 400+ lines, with 60% being static file serving, GitHub download, zip extraction, and MIME type logic. This is tangential to the channel's core purpose and should be a separate utility module.

### 2.2 CLI Channel

#### Structure
```
CliPlugin → CliChannel (mpsc bridge) → InteractiveLoop (reedline REPL)
```

#### Implementation Quality: ★★★☆☆

| Aspect | Details |
|--------|---------|
| **Channel trait** | Clean. `receive()` reads from mpsc, `send()` prints to stdout |
| **Session management** | `Session` struct with UUID, timestamps, message count. Shared via `Arc<std::sync::Mutex<Session>>` |
| **Meta-commands** | `.quit`, `.help`, `.reset`, `.model`, `.persona`, `.clear` — well-parsed |
| **Error propagation** | `println!()` for responses — no error context shown to user |
| **No background tasks** | `tasks: vec![]` — the interactive loop is started separately |

#### Inconsistencies & Issues

1. **Response delivery is fire-and-forget**: `Channel::send()` just does `println!()`. The interactive loop sends a message and then does NOT wait for a response — it immediately goes back to the readline prompt. This means:
   - The user sees the prompt again before the response arrives
   - Multiple messages can be sent before any response shows up
   - No ordering guarantee (responses may interleave with user input)

2. **No response correlation**: Unlike WebChannel which uses oneshot channels to correlate requests and responses, CLI just prints whatever arrives. The gateway's `route()` preserves the message ID but the CLI doesn't use it.

3. **`.model` and `.persona` are stubs**: These commands print messages but have `// TODO: wire to kernel model switching` comments. They're dead commands.

4. **Plugin doesn't start the REPL**: `CliPlugin::setup()` returns the channel but the interactive loop is not in `tasks`. The caller (main binary) must separately start the interactive loop. This is noted in the doc comment but breaks the pattern — Web and Telegram plugins are fully self-contained.

5. **Mixed async/sync**: `InteractiveLoop::run()` is async but wraps `reedline::read_line()` which is blocking. This should use `spawn_blocking` (it's documented as such but the implementation doesn't).

6. **Std::sync::Mutex in async context**: `CliChannel` uses `Arc<std::sync::Mutex<Session>>` which can block the tokio runtime if held across await points. `tokio::sync::Mutex` would be safer.

7. **No error display to user**: When the orchestrator fails, `Channel::send()` prints the raw error string. There's no formatting, no indication that it's an error vs. a normal response.

### 2.3 Telegram Channel

#### Structure
```
TelegramPlugin → TelegramChannel (long polling, direct Bot API calls)
```

#### Implementation Quality: ★★★★☆

| Aspect | Details |
|--------|---------|
| **Channel trait** | `receive()` polls Telegram Bot API; `send()` posts via Bot API |
| **Session management** | Per-chat sessions with auto-rotation (time + message count). `/new` and `/session` commands |
| **User authorization** | Allowlist-based. Rejects unauthorized users with a message |
| **Message chunking** | Splits messages >4000 chars (Telegram 4096 limit) |
| **Error propagation** | Markdown parse failure falls back to plain text. User gets the message either way |
| **Korean UX** | Response messages for `/new` and `/session` are in Korean (consistent with project conventions) |

#### Inconsistencies & Issues

1. **Blocking in `receive()`**: The `receive()` method runs an infinite `loop` with 30-second long polling. This is correct for the trait but means the gateway's drain loop will block on this channel until a message arrives. Since the gateway holds a **write lock** while calling `receive()`, this blocks ALL channels for up to 30 seconds if Telegram is polled first.

   **Critical**: This is a systemic issue. The gateway's `run()` method calls `ch.receive()` while holding `self.channels.write().await`. If Telegram's long poll takes 30s, no other channel can be read or written during that time.

2. **No background task**: Like CLI, `tasks: vec![]`. The polling happens inside `receive()` which the gateway calls synchronously. A better design would spawn a background polling task that feeds an mpsc channel, making `receive()` non-blocking.

3. **Stringly-typed metadata**: `chat_id` and `message_id` are stored as strings in the metadata HashMap and parsed back to integers in `send()`. This is fragile — if the metadata format changes, messages silently fail to deliver.

4. **No retry on send failure**: If `send_text()` fails (network error, rate limit), the error bubbles up but the message is lost. No retry or queue mechanism.

5. **`/new@me` vs `/new`**: The code checks for `/new@me` and `/new@me` but only partially handles bot username suffixes. Other commands like `/session` only check `/session` and `/session@me` without the actual bot name.

---

## 3. Cross-Channel Comparison

### 3.1 Feature Matrix

| Feature | Web (HTTP) | Web (WS) | CLI | Telegram |
|---------|-----------|----------|-----|----------|
| Multi-turn sessions | ✅ (session_id in metadata) | ✅ (session_id in metadata) | ✅ (Session struct) | ✅ (ChatSession per chat) |
| Session persistence | ✅ (StateStore) | ✅ (StateStore) | ❌ (in-memory only) | ❌ (in-memory only) |
| Streaming responses | ❌ (request/response) | ✅ (token chunks) | ❌ (fire-and-forget) | ❌ (fire-and-forget) |
| Space context | ✅ (space_id param) | ✅ (space_id param) | ❌ | ❌ |
| User authentication | ✅ (Bearer token) | ✅ (query param token) | ❌ | ✅ (user allowlist) |
| Rate limiting | ✅ (per-IP middleware) | ✅ (shared middleware) | ❌ | ❌ |
| Error formatting | ✅ (AppError → JSON) | ✅ (WS error frames) | ❌ (raw println) | ⚠️ (fallback plain text) |
| Meta/channel commands | N/A | N/A | ✅ (dot-commands) | ✅ (slash-commands) |
| Response correlation | ✅ (oneshot channel) | ✅ (message ID matching) | ❌ | ❌ |
| Request validation | ✅ (64KB limit) | ❌ | ❌ | ❌ |
| Auto-session rotation | ❌ | ❌ | ✅ (.reset) | ✅ (time + count) |
| Background tasks | ✅ (Axum server) | — | ❌ | ❌ |
| Knowledge UI | ✅ (full SPA) | — | ❌ | ❌ |
| Session listing | ✅ (API endpoint) | — | ❌ | ❌ (only /session for self) |
| SSE events | ✅ (KernelEvent stream) | — | ❌ | ❌ |

### 3.2 Consistency Analysis

#### Message Flow Consistency

The general flow is consistent:
```
User Input → IncomingMessage → Channel.receive() → Gateway.route() → Orchestrator → OutgoingMessage → Channel.send() → User
```

But the **quality of the round-trip** varies dramatically:

| Channel | Round-trip | User Experience |
|---------|-----------|-----------------|
| Web HTTP | Synchronous (send_and_wait) | ✅ Reliable, ordered |
| Web WS | Asynchronous (broadcast) | ✅ Streamed, session-tracked |
| CLI | Asynchronous (fire-and-forget) | ⚠️ Prompt returns before response |
| Telegram | Asynchronous (long poll) | ✅ Reply-as-response model works |

#### Error Handling Consistency

| Channel | Orchestrator Error | Network Error | Validation Error |
|---------|-------------------|---------------|-----------------|
| Web HTTP | `AppError::Internal("gateway response failed")` | Connection timeout | `AppError::PayloadTooLarge` |
| Web WS | Not explicitly handled | WS disconnection | N/A |
| CLI | Raw `println!("An error occurred: ...")` | N/A | N/A |
| Telegram | Raw text via Bot API | `anyhow::bail!` in receive | N/A |

**Problem**: CLI users see raw internal error strings. Web users see sanitized errors. Telegram users get whatever the Bot API sends. No unified error formatting.

#### Metadata Consistency

Channels use metadata keys inconsistently:

| Key | Web HTTP | Web WS | CLI | Telegram |
|-----|---------|--------|-----|----------|
| `session_id` | ✅ | ✅ | ✅ | ✅ |
| `space_id` | ✅ | ✅ | ❌ | ❌ |
| `chat_id` | ❌ | ❌ | ❌ | ✅ |
| `message_id` | ❌ | ❌ | ❌ | ✅ |
| `phase` | ✅ (response) | ✅ (response) | ❌ | ❌ |
| `evaluation_passed` | ✅ (response) | ✅ (response) | ❌ | ❌ |

**Problem**: CLI and Telegram don't expose phase or evaluation status to users. CLI doesn't support space context.

---

## 4. Architectural Issues

### 4.1 Critical: Gateway Write Lock During Polling

```rust
// gateway.rs run() method
let msg = {
    let mut channels = self.channels.write().await;  // WRITE LOCK
    if let Some(ch) = channels.get_mut(name) {
        ch.receive().await.ok().flatten()  // BLOCKS UP TO 30s (Telegram)
    } else {
        break;
    }
};
```

The gateway acquires a **write lock** on the entire channel map just to poll a single channel. Combined with Telegram's 30-second long poll, this means:
- All other channels are blocked from reading/writing
- No messages can be routed while waiting for Telegram
- The CLI `send()` (which just prints) would be blocked

**Fix**: Use separate locks per channel, or switch to an event-driven model where channels push messages to a shared mpsc rather than being polled.

### 4.2 Moderate: No Channel Lifecycle Management

The `Channel` trait has no `start()`, `stop()`, or `health()` methods. Channels are expected to be "always on." This means:
- No graceful shutdown signaling (except the gateway-level AtomicBool)
- No health checks per channel
- No way to detect a broken channel and remove it

### 4.3 Moderate: Duplicated Session Persistence

The web channel has ~120 lines of duplicated session persistence code between `handle_chat()` (HTTP) and `persist_session()` (WS). This should be a shared utility.

### 4.4 Low: Untyped Metadata

`metadata: HashMap<String, String>` is flexible but error-prone. Keys like `"session_id"`, `"space_id"`, `"phase"` are used across the codebase without constants or typed accessors. A typo in a metadata key silently breaks functionality.

### 4.5 Low: CLI Interactive Loop Not Self-Contained

The CLI plugin returns `tasks: vec![]` and relies on the caller to separately start the interactive loop. This breaks the plugin contract where `setup()` should return a fully operational channel.

---

## 5. Channel-Specific Hacks

| Channel | Hack | Description |
|---------|------|-------------|
| Web | Auto-update in plugin.rs | 200+ lines of GitHub release downloading, zip extraction, and filesystem serving mixed into the channel plugin |
| Web | Double-write in `Channel::send()` | Every outgoing message is both delivered to the HTTP oneshot AND broadcast to all WS/SSE subscribers |
| Web | Static file serving | Full SPA static file server embedded in the channel — tangential to messaging |
| CLI | Fire-and-forget responses | `send()` just `println!()` with no back-pressure or correlation |
| CLI | Std::sync::Mutex in async | `Arc<std::sync::Mutex<Session>>` used across await points |
| Telegram | Blocking `receive()` | 30-second long poll blocks the gateway's event loop |
| Telegram | Stringly-typed chat_id | `chat_id` stored as string in metadata, parsed back to i64 in send |

---

## 6. Recommendations

### Short Term
1. **Fix gateway locking**: Switch from a single `RwLock<HashMap>` to per-channel polling or an mpsc-based push model. This unblocks Telegram's long poll.
2. **Extract session persistence**: Create a `SessionPersistence` helper in the kernel or web module to deduplicate the HTTP/WS persistence code.
3. **Add metadata constants**: Define `const SESSION_ID: &str = "session_id"` etc. to prevent typos.
4. **CLI response correlation**: Add a oneshot channel like WebChannel's `send_and_wait()` so the interactive loop can display responses in order.

### Medium Term
5. **Channel lifecycle trait**: Add `start()`, `stop()`, `health()` to the `Channel` trait or a new `LifecycleChannel` extension.
6. **Telegram background polling**: Spawn a background task that polls Telegram and feeds an mpsc, making `receive()` non-blocking.
7. **Unified error formatting**: Define a `ChannelErrorFormatter` trait that each channel implements for user-facing error display.
8. **Extract web static serving**: Move auto-update + static file serving to a separate module or utility crate.

### Long Term
9. **Event-driven gateway**: Replace the polling loop with `tokio::select!` over per-channel mpsc receivers.
10. **Typed metadata**: Replace `HashMap<String, String>` with a structured `MessageMetadata` type.
11. **Feature parity audit**: Ensure CLI and Telegram support `space_id`, expose `phase`/`evaluation_passed` to users.
