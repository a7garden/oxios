# Telegram Channel Analysis

> Thorough analysis of `channels/oxios-telegram/` implementation.
> Source files: `Cargo.toml`, `src/lib.rs`, `src/plugin.rs`

---

## 1. How Telegram Sends/Receives Messages

### Receiving (Inbound)

**Method:** Long polling via Telegram Bot API `getUpdates`.

- `TelegramChannel::poll_updates()` POSTs to `https://api.telegram.org/bot{token}/getUpdates` with:
  - `timeout: 30` (long-poll wait up to 30 seconds)
  - `limit: 100` (max updates per call)
  - `offset` (tracks last acknowledged `update_id` to avoid re-processing)
- The `start()` method (from `Channel` trait) spawns a `tokio::spawn` loop that:
  1. Calls `poll_updates()` in a `tokio::select!` against a shutdown signal
  2. Extracts messages from `update.message`, `update.channel_post`, or `update.edited_message`
  3. Extracts `chat_id`, `user_id`, `text`, `message_id`
  4. Skips empty-text messages
  5. Runs user permission check
  6. Handles built-in commands (`/new`, `/session`)
  7. Skips any other `/command` messages
  8. Gets or creates a per-chat session (with auto-rotation)
  9. Constructs an `IncomingMessage` with metadata (`chat_id`, `message_id`, `session_id`)
  10. Pushes `(channel_name, incoming)` into the gateway's `mpsc::Sender<GatewayInbox>`

**Gateway routing:** The `Gateway` receives from the shared mpsc, dispatches each message to `orchestrator.handle_message()` in a semaphore-bounded tokio task (max 32 concurrent).

### Sending (Outbound)

- `TelegramChannel::send()` implements `Channel::send()`.
- Extracts `chat_id` from `msg.metadata["chat_id"]` (falls back to `msg.user_id`).
- Extracts optional `reply_to` from `msg.metadata["message_id"]`.
- Delegates to `send_text()`.
- `send_text()` POSTs to `sendMessage` with:
  - `chat_id`, `text`, `parse_mode: "Markdown"`, optional `reply_to_message_id`

---

## 2. Current Error Handling

### Polling Errors
- **`poll_updates()` failure** → `tracing::warn!` + `tokio::time::sleep(5s)` before retry. No exponential backoff, no max-retry limit. This means transient API failures are handled, but persistent failures will loop forever at 5-second intervals with warning logs.

### Sending Errors
- **`send_text()` Markdown failure** → If the `sendMessage` response status is not success, it **retries without `parse_mode`** (sends as plain text). This is a sensible Telegram-specific fallback since malformed Markdown causes API rejections.
- **Message too long (>4000 chars)** → Split into 4000-byte chunks. **Bug:** The chunking uses `as_bytes().chunks(4000)` which can split in the middle of a multi-byte UTF-8 character. `String::from_utf8_lossy` replaces broken characters with `\u{fffd}`, corrupting content.

### Permission Errors
- **Unauthorized user** → `tracing::warn!` + sends "Unauthorized. Your user ID is not in the allowed list." reply.

### Gateway-Level Errors
- **Orchestration failure** → Gateway sends `"An error occurred: {e}"` as plain text back to the user.
- **Channel send failure** → `tracing::error!` logged, message is lost. No retry.
- **Gateway receiver closed** → Channel breaks out of loop gracefully.
- **Semaphore closed** → Message is dropped with a warning.

### Plugin Setup Errors
- **Missing bot token** → `anyhow::anyhow!` with descriptive message about which env var to set.
- **Config validation** → Config's `validate()` checks if telegram is enabled but token env var is missing and warns.

### Error Gaps
1. **No exponential backoff** on poll failures — could hit rate limits aggressively
2. **No circuit breaker** — if Telegram API is down, it'll poll forever
3. **No dead letter queue** — failed sends are silently dropped
4. **UTF-8 chunking bug** — can corrupt messages at byte boundaries
5. **No timeout on HTTP requests** — `reqwest::Client::new()` uses default timeout (none), so `send_text` and `poll_updates` could hang indefinitely

---

## 3. Response Formatting

- **Parse mode:** `Markdown` (Telegram's legacy Markdown v1)
- **No HTML support:** Only Markdown is attempted
- **Fallback:** If Markdown parse fails, message is re-sent as plain text (no parse_mode)
- **No formatting sanitization:** Agent output containing characters like `_`, `*`, `[`, `` ` `` that are not valid Markdown will cause the first send to fail, then the fallback will send raw text
- **Reply threading:** Responses include `reply_to_message_id` pointing to the user's original message, creating threaded conversations in Telegram

### Response Metadata (from Gateway)
The gateway adds metadata to outgoing messages:
- `session_id` — from orchestration result
- `space_id` — from orchestration result  
- `phase` — e.g., "Execute"
- `evaluation_passed` — boolean as string

This metadata is **not used** by `TelegramChannel::send()` — it only reads `chat_id` and `message_id`. All orchestration metadata is discarded in the Telegram response.

---

## 4. Session Handling

### Architecture
- **Per-chat sessions:** `HashMap<i64, ChatSession>` keyed by Telegram `chat_id`
- **In-memory only:** `Arc<RwLock<HashMap<...>>>` — sessions are lost on process restart
- **Session ID:** UUID v4, generated on first message per chat

### `ChatSession` Fields
| Field | Purpose |
|-------|---------|
| `session_id` | UUID for multi-turn conversation continuity |
| `created_at` | Session creation timestamp |
| `last_active_at` | Last message timestamp (for rotation) |
| `message_count` | Messages exchanged in this session |

### Session Rotation
Two rotation triggers (configurable):
1. **Time-based:** After `rotation_hours` of inactivity (default: 2 hours). Checks `Utc::now() - last_active_at`.
2. **Count-based:** After `max_messages_per_session` messages (default: 0 = unlimited).

On rotation, a new UUID is generated, counters reset.

### Manual Commands
| Command | Action |
|---------|--------|
| `/new` or `/new@me` | Force-rotate to new session, reply with first 8 chars of new session ID |
| `/session` or `/session@me` | Display current session info (ID prefix, message count, timestamps) |

### Session Flow
```
Message arrives → get_or_create_session(chat_id)
  → If session exists and should_rotate() → auto-rotate
  → touch() (update last_active_at, increment message_count)
  → session_id passed in IncomingMessage.metadata["session_id"]
  → Gateway passes to orchestrator.handle_message(user_id, content, Some(session_id))
  → Orchestrator uses session_id for multi-turn context
```

### Configuration (`config.toml`)
```toml
[channels.telegram.session]
rotation_hours = 2       # hours of inactivity before auto-rotate
max_messages = 0         # messages per session (0 = unlimited)
```

---

## 5. Space Support

### Current State: **Minimal / Indirect**

- The Telegram channel itself has **no Space awareness**. It does not set `space_id` in `IncomingMessage.metadata`.
- The `IncomingMessage` struct has a `metadata` field but it only carries `chat_id`, `message_id`, and `session_id`.
- The orchestrator's `handle_message()` receives `(user_id, content, session_id)` — **no space_id parameter**.
- When the orchestration completes, the gateway extracts `space_id` from the result and puts it in the outgoing `OutgoingMessage.metadata`, but the Telegram channel's `send()` ignores it.

### Implications
- There is no way for a Telegram user to switch between Spaces or specify which Space their message targets.
- The orchestrator may create/assign a Space internally, but the Telegram user has no visibility or control over this.
- No `/space` command or Space-related UI exists in the Telegram channel.

---

## 6. Progress Indicators / Streaming

### Current State: **None**

- **No typing indicator:** The channel never calls `sendChatAction` with `typing` action.
- **No streaming:** The gateway waits for `orchestrator.handle_message()` to complete fully, then sends the complete response in one shot.
- **No partial responses:** The user sees nothing until the entire orchestration finishes.
- **No "thinking" or progress messages:** No intermediate updates during the interview → seed → execute → evaluate → evolve cycle.

### User Experience Impact
For complex queries that go through the full Ouroboros protocol (potentially minutes of processing), the Telegram user sees complete silence until the final response arrives. This can be confusing — the user may think the bot is broken or has timed out.

### What Could Be Added
1. `sendChatAction` with `typing` action on message receipt
2. Periodic "still working..." messages for long-running tasks
3. Intermediate progress updates for each Ouroboros phase
4. Streaming via Telegram's `editMessageText` (send initial message, then edit as tokens arrive)

---

## File Inventory

| File | Lines | Purpose |
|------|-------|---------|
| `Cargo.toml` | 18 | Dependencies (reqwest, serde, tokio, gateway) |
| `src/lib.rs` | ~420 | Core: `TelegramChannel`, `ChatSession`, `Channel` impl, tests |
| `src/plugin.rs` | ~50 | `TelegramPlugin` factory (implements `ChannelPlugin`) |

### Dependencies
- `oxios-gateway` — Channel trait, message types, plugin system
- `reqwest 0.12` (json feature) — HTTP client for Telegram Bot API
- `tokio` (full) — async runtime, mpsc, watch, RwLock
- `serde` / `serde_json` — JSON serialization
- `chrono` — timestamps
- `uuid` — session ID generation
- `anyhow` — error handling
- `async-trait` — trait async methods
- `tracing` — structured logging

### Notable: No `teloxide` or `frankenstein`
The implementation uses raw HTTP calls to the Telegram Bot API via `reqwest`. No Telegram framework library is used. This is minimal but means all API interaction, error parsing, and retry logic must be hand-maintained.

---

## Summary of Issues & Gaps

| Issue | Severity | Description |
|-------|----------|-------------|
| UTF-8 chunking bug | **High** | `as_bytes().chunks(4000)` can split multi-byte chars, corrupting messages |
| No typing indicator | **Medium** | User sees silence during processing |
| No streaming | **Medium** | Long-running orchestrations give no feedback |
| No exponential backoff | **Medium** | Poll errors retry at fixed 5s interval |
| No HTTP timeout | **Medium** | `reqwest::Client::new()` has no timeout set |
| No Space support | **Medium** | No way to select or manage Spaces from Telegram |
| Sessions in-memory only | **Low** | Lost on restart (acceptable for Telegram) |
| Orchestration metadata discarded | **Low** | `phase`, `evaluation_passed`, `space_id` not shown to user |
| No media handling | **Low** | Only text messages processed; images, files, voice ignored |
| No webhook mode | **Low** | Long polling only; no webhook support for behind-NAT scenarios |
