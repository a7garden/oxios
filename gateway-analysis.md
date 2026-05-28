# oxios-gateway Crate Analysis

## 1. Message Types

### IncomingMessage

**File:** `crates/oxios-gateway/src/message.rs`

| Field | Type | Description |
|-------|------|-------------|
| `id` | `uuid::Uuid` | Unique message identifier (auto-generated `new_v4()`) |
| `channel` | `String` | Name of the source channel (e.g., "web", "cli", "telegram") |
| `user_id` | `String` | Identifier for the user who sent the message |
| `content` | `String` | Message content (the user's prompt/text) |
| `timestamp` | `DateTime<Utc>` | Auto-set to `Utc::now()` on creation |
| `metadata` | `HashMap<String, String>` | Optional key-value metadata (e.g., `session_id` for multi-turn) |

Derives: `Debug, Clone, Serialize, Deserialize, Default`

Constructor: `IncomingMessage::new(channel, user_id, content)` — auto-generates id and timestamp.

### OutgoingMessage

**File:** `crates/oxios-gateway/src/message.rs`

| Field | Type | Description |
|-------|------|-------------|
| `id` | `uuid::Uuid` | Unique message identifier |
| `channel` | `String` | Name of the target channel |
| `user_id` | `String` | Identifier for the receiving user |
| `content` | `String` | Response content |
| `timestamp` | `DateTime<Utc>` | Auto-set to `Utc::now()` on creation |
| `metadata` | `HashMap<String, String>` | Optional metadata (e.g., `session_id`, `phase`, `evaluation_passed`, `space_id`) |

Derives: `Debug, Clone, Serialize, Deserialize`

Constructors:
- `OutgoingMessage::new(channel, user_id, content)` — new UUID
- `OutgoingMessage::with_id(id, channel, user_id, content)` — preserves a specific ID (for correlation)
- `OutgoingMessage::with_metadata(channel, user_id, content, metadata)` — with metadata map
- `OutgoingMessage::with_id_and_metadata(id, channel, user_id, content, metadata)` — both specific ID and metadata

---

## 2. Response Flow: Gateway → Channels

The full response flow is:

```
User Input → Channel (HTTP/stdin/Telegram API)
          → Channel::start() pushes (channel_name, IncomingMessage) into shared mpsc
          → Gateway::run() event loop receives from mpsc
          → Gateway::dispatch() spawns tokio::task per message (semaphore-bounded to 32 concurrent)
          → Inside dispatch task: orchestrator.handle_message(user_id, content, session_id) is called
          → On success: OutgoingMessage built with metadata (session_id, space_id, phase, evaluation_passed)
          → On error: OutgoingMessage with error text
          → channel.send(outgoing) is called on the registered channel
```

**Key detail in `dispatch()` (gateway.rs):**

1. **Success path**: Calls `OutgoingMessage::with_id_and_metadata(msg.id, ...)` — the outgoing message ID is set to the **incoming message's ID**. This is the correlation mechanism.
2. **Metadata populated from orchestration result:**
   - `session_id` (if present)
   - `space_id` (if present, converted to string)
   - `phase` (phase_reached as string)
   - `evaluation_passed` (boolean as string)
3. **Error path**: Calls `OutgoingMessage::with_id(msg.id, ...)` — same ID correlation, no metadata.

Each channel's `send()` implementation then handles the `OutgoingMessage` in its own way:
- **CLI**: `println!("{}", msg.content)` — raw text to stdout
- **Web**: Two-path delivery:
  1. Looks up `msg.id` in a `HashMap<Uuid, oneshot::Sender>` for HTTP request-response correlation
  2. Broadcasts to all WebSocket/SSE subscribers via `broadcast::Sender`
- **Telegram**: Extracts `chat_id` from metadata, sends text via Telegram API, optionally replies to a `message_id`

---

## 3. ChannelFormatter / Response Formatting

**There is NO existing ChannelFormatter, ResponseFormatter, or any response formatting abstraction.**

The search for `ChannelFormatter`, `ResponseFormatter`, `format_response`, `format_outgoing`, `MessageFormatter` across all crates returned zero results.

Response formatting is **entirely ad-hoc** and channel-specific:
- **CLI**: Raw `msg.content` printed directly — no formatting at all
- **Web**: `msg.content` passed through as-is to HTTP response or WebSocket broadcast — the frontend handles rendering
- **Telegram**: `msg.content` sent as plain text via `send_text()` — no Markdown/HTML conversion visible in the gateway

The `content` field of `OutgoingMessage` is always the raw string from `orchestration.response` (or an error message). No transformation or channel-specific formatting is applied by the gateway or by any shared formatter.

**This is a clear gap**: if channels need different presentation formats (e.g., Telegram needs HTML parsing, CLI wants ANSI colors, Web wants structured JSON), there's no mechanism for it today.

---

## 4. Channel Trait Definition

**File:** `crates/oxios-gateway/src/channel.rs`

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    /// Returns the name of this channel (e.g., "web", "telegram").
    fn name(&self) -> &str;

    /// Start the channel's background receive loop.
    async fn start(
        &self,
        tx: mpsc::Sender<GatewayInbox>,
        shutdown: watch::Receiver<bool>,
    ) -> Result<JoinHandle<()>>;

    /// Send a response message through this channel.
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;
}
```

**Trait contract:**
- `name()` — returns a static string identifier
- `start(tx, shutdown)` — must spawn a background task that pushes `(name, IncomingMessage)` tuples into `tx`, and exits when `shutdown` changes to `true`. Returns the task's `JoinHandle`.
- `send(msg)` — delivers an `OutgoingMessage` to the channel's users

**ChannelPlugin trait (plugin.rs):**

```rust
#[async_trait]
pub trait ChannelPlugin: Send + Sync {
    fn name(&self) -> &str;
    async fn setup(&self, ctx: ChannelContext) -> Result<ChannelBundle>;
}
```

Where `ChannelContext` provides:
- `kernel: Arc<KernelHandle>` — kernel subsystem handle
- `config: Arc<RwLock<OxiosConfig>>` — hot-reloadable config
- `config_path: PathBuf` — path to config file

And `ChannelBundle` returns:
- `channel: Box<dyn Channel>` — for gateway registration
- `tasks: Vec<JoinHandle<()>>` — background tasks (e.g., axum server)

This is a factory pattern — the main binary discovers plugins via a registry and calls `setup()` for each enabled channel based on feature flags.

---

## 5. Correlation ID / Response Tracking

### Correlation via Message ID

The primary correlation mechanism is **ID matching**: `OutgoingMessage.id` is set to `IncomingMessage.id`.

In `gateway.rs::dispatch()`:
```rust
let outgoing = OutgoingMessage::with_id_and_metadata(
    msg.id,       // ← incoming message ID reused as outgoing ID
    &msg.channel,
    &msg.user_id,
    &orchestration.response,
    response_metadata,
);
```

This is a "reflect-back" pattern — the response carries the same ID as the request.

### Web Channel: Request-Response Correlation Map

The Web channel implements the most sophisticated response tracking:

```rust
// In WebChannel:
responses: Arc<RwLock<HashMap<uuid::Uuid, oneshot::Sender<OutgoingMessage>>>>
```

**Flow:**
1. HTTP handler calls `send_and_wait(msg)`, which:
   - Creates a `oneshot::channel()`
   - Registers `(msg.id, oneshot_sender)` in the correlation map
   - Sends the `IncomingMessage` into the gateway pipeline
   - Awaits the `oneshot_receiver`
2. When the gateway routes the response back via `channel.send(outgoing)`:
   - Looks up `outgoing.id` in the correlation map
   - If found, sends the response through the `oneshot_sender` → unblocks the HTTP handler
   - Also broadcasts to WebSocket/SSE subscribers

### No Explicit Correlation Tracking in Gateway

The gateway itself does **not** maintain a correlation map or request tracking. It relies entirely on:
1. The incoming message being dispatched to a `tokio::spawn` task
2. That task having a cloned `Arc<RwLock<HashMap<String, ChannelEntry>>>` to look up the channel
3. The `Channel::send()` implementation handling correlation internally

There is **no**:
- `correlation_id` field (the `msg.id` serves this purpose implicitly)
- Response timeout or deadline at the gateway level
- Request/response tracking map in the gateway
- Retry or deduplication mechanism
- `in_reply_to` or `reply_to` field

### Telegram: Metadata-based Reply

Telegram uses metadata for reply threading:
```rust
let reply_to = msg.metadata.get("message_id").and_then(|id| id.parse().ok());
self.send_text(chat_id, &msg.content, reply_to).await
```

The `message_id` metadata from the incoming Telegram message is reflected back to create a reply thread.

---

## Summary of Findings

| Aspect | Status |
|--------|--------|
| **Message types** | Clean, symmetric `IncomingMessage`/`OutgoingMessage` with identical field shapes |
| **Correlation** | Implicit via ID reuse (`OutgoingMessage.id = IncomingMessage.id`); Web channel has explicit correlation map with oneshot channels |
| **Channel trait** | Minimal 3-method trait: `name()`, `start()`, `send()` |
| **ChannelPlugin** | Factory pattern with `setup()` receiving `ChannelContext` |
| **Response formatting** | **None exists** — no `ChannelFormatter`, no format abstraction. Raw `content` string passed through to each channel |
| **Metadata enrichment** | Gateway adds `session_id`, `space_id`, `phase`, `evaluation_passed` to outgoing metadata |
| **Concurrency** | Semaphore-bounded (32 concurrent routes), mpsc buffer of 1024 |
| **Shutdown** | `watch::channel(bool)` per channel + gateway-wide shutdown signal |
