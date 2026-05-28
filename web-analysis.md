# Web Channel Backend Analysis

> Generated: 2026-05-27
> Scope: `channels/oxios-web/src/` (Rust backend only)

---

## 1. How Web Routes Handle Responses

All route handlers follow axum conventions with two primary response patterns:

### Pattern A: JSON responses via `Json<T>` wrapper

Most handlers return `Result<Json<T>, AppError>` or `Json<T>` directly:

```rust
// Success — returns 200 with JSON body
Ok(Json(ChatResponse { id, echo, reply, session_id, space_id, phase }))

// Error — AppError maps to HTTP status + JSON error body
Err(AppError::NotFound("seed not found".into()))
```

### Pattern B: SSE (Server-Sent Events) for streaming

`GET /api/events` uses axum's `Sse` wrapper with a `BroadcastStream`:

```rust
pub(crate) async fn handle_events(...) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>>
```

Events are sanitized (sensitive content like LLM responses is stripped) and sent as JSON-formatted SSE data with 30-second keepalive pings.

### Pattern C: Raw responses with status codes

Some handlers (like workspace file reads) return `impl IntoResponse` with explicit status + headers:

```rust
Ok((StatusCode::OK, [(header::CONTENT_TYPE, mime)], content))
```

### Pattern D: Empty unit responses

Agent kill returns `Result<(), AppError>` — 200 with empty body on success.

### Response format summary

| Return type | HTTP body | Example handlers |
|---|---|---|
| `Json<T>` | `{"field": "value"}` | chat, status, config, skills, memory |
| `Sse<...>` | SSE stream of JSON events | events endpoint |
| `impl IntoResponse` | Raw text/file content | workspace file get |
| `()` | Empty body | agent kill |
| `StatusCode` | Empty body | session delete, approval actions |

---

## 2. Session Persistence Mechanism

### Storage backend

Sessions are persisted as **JSON files** on disk via `StateStore`:

```
<workspace>/sessions/<session-id>.json
```

The `StateStore` uses atomic writes (write to `.tmp`, then rename) for crash safety.

### Session lifecycle

**Creation / update** happens inside `handle_chat` (POST) and `handle_chat_websocket` (WS):

1. Message sent to gateway → orchestrator processes → response comes back
2. Session ID extracted from `response.metadata["session_id"]`
3. `state.kernel.state.load_session(&session_id)` — tries to load existing
4. If exists → appends `UserMessage` + `AgentResponse` to existing session
5. If not found → creates new `Session` with that ID, adds messages
6. `state.kernel.state.save_session(&session)` — writes to disk

### Session structure (on disk)

```json
{
  "id": "uuid-string",
  "user_id": "default",
  "user_messages": [
    { "content": "...", "timestamp": "2026-05-27T..." }
  ],
  "agent_responses": [
    {
      "content": "...",
      "session_id": "uuid-string",
      "seed_id": "uuid-string | null",
      "phase_reached": "Execute",
      "evaluation_passed": true,
      "timestamp": "2026-05-27T..."
    }
  ],
  "active_seed_id": "uuid-string | null",
  "active_persona_id": "uuid-string | null",
  "created_at": "2026-05-27T...",
  "updated_at": "2026-05-27T...",
  "metadata": {
    "space_id": "uuid-string"
  }
}
```

### Auto-pruning

- Configurable via `session.auto_prune`, `session.max_sessions`, `session.ttl_hours`
- Throttled to once per hour via `PruneThrottle` / global `AtomicU64`
- TTL-based: removes sessions older than `ttl_hours`
- Count-based: keeps only `max_sessions` most recent (sorted by `updated_at` descending)
- Manual trigger: `POST /api/sessions/prune`

### Multi-turn continuity

Clients pass `session_id` in request body or metadata. The orchestrator looks up the existing session and continues the conversation. If no `session_id` is provided, a new session is created.

---

## 3. Current Response Format (JSON Shape)

### Chat response (`POST /api/chat`)

```json
{
  "id": "uuid-of-original-message",
  "echo": "original user message",
  "reply": "agent's response content",
  "session_id": "uuid-or-null",
  "space_id": "uuid-or-null",
  "phase": "Execute|Interview|Seed|Evaluate|Evolve"
}
```

### Paginated list response (agents, sessions, seeds, skills, memory)

```json
{
  "items": [...],
  "total": 42,
  "page": 1,
  "limit": 50
}
```

### Status response

```json
{
  "service": "oxios",
  "status": "running",
  "version": "0.x.y",
  "channels": ["web"],
  "uptime": "1h 23m 45s",
  "components": {
    "state_store": { "healthy": true, "detail": null },
    "event_bus": { "healthy": true, "detail": null },
    "memory": { "enabled": true, "index_size": 0, "total_entries": 0 },
    "agents": {
      "active_count": 0,
      "total_forked": 0,
      "total_completed": 0,
      "total_failed": 0
    },
    "spaces_active": 0
  }
}
```

### Health response

```json
{ "status": "ok", "version": "0.x.y" }
```

### Readiness response

```json
{
  "status": "healthy|degraded",
  "version": "0.x.y",
  "uptime_secs": 5025,
  "components": {
    "state_store": { "healthy": true },
    "git": { "healthy": true },
    "memory": { "healthy": true, "index_size": 0, "total_entries": 0 }
  }
}
```

### Session detail (`GET /api/sessions/:id`)

```json
{
  "id": "uuid",
  "user_id": "default",
  "space_id": "uuid-or-null",
  "user_messages": [...],
  "agent_responses": [...],
  "active_seed_id": "uuid-or-null",
  "active_persona_id": "uuid-or-null",
  "created_at": "RFC3339",
  "updated_at": "RFC3339",
  "metadata": { ... }
}
```

### Skill response

```json
{
  "name": "skill-name",
  "description": "...",
  "author": "...",
  "version": "...",
  "emoji": "...",
  "homepage": "...",
  "source": "bundled|managed|workspace",
  "bundled": true,
  "status": "ready|needs_setup|disabled",
  "eligible": true,
  "always": false,
  "user_invocable": true,
  "file_path": "~/.oxios/.../SKILL.md",
  "requirements": { "bins": [], "anyBins": [], "env": [], "config": [] },
  "missing": { "bins": [], "anyBins": [], "env": [], "config": [] },
  "os": [],
  "install": [...],
  "config_checks": [...],
  "format": "..."
}
```

### Generic action response

```json
{ "status": "created|deleted|enabled|disabled|approved|rejected|pruned", "name|id": "..." }
```

---

## 4. How Errors Are Returned

### `AppError` enum → HTTP status mapping

All route errors go through `AppError` which implements `IntoResponse`:

| Variant | HTTP Status | Example |
|---|---|---|
| `NotFound(String)` | 404 | Seed not found, file not found |
| `BadRequest(String)` | 400 | Invalid config, invalid memory_type |
| `Unauthorized(String)` | 401 | Auth failures |
| `Forbidden(String)` | 403 | Path traversal denied |
| `Internal(String)` | 500 | Gateway failures, I/O errors |
| `PayloadTooLarge { size, limit }` | 413 | Chat >64KB, skill >64KB, memory >32KB |

### Error response body shape

```json
{ "error": "human-readable message" }
```

### Status-code-only responses

Some handlers (session delete, approval actions) return raw `StatusCode` instead of `AppError`:

```rust
Err(StatusCode::NOT_FOUND)
Err(StatusCode::BAD_REQUEST)
Err(StatusCode::INTERNAL_SERVER_ERROR)
```

These return **empty bodies** with only the status code.

### Rate limiting

Rate limiter returns `429 Too Many Requests` as a raw `StatusCode` (no body).

### Auth middleware

Auth failures return `401 Unauthorized` as a raw `StatusCode` (no body).

### anyhow → AppError conversion

`anyhow::Error` auto-converts to `AppError::Internal(message)`.

---

## 5. WebSocket Handling

### Endpoint

`GET /api/chat/stream` — upgraded via `WebSocketUpgrade`

### Authentication

- If `auth_enabled` is true, the WS handshake requires a `?token=<bearer>` query parameter
- Token is validated against `kernel.security.validate_token()`
- Returns `401` as plain status code if auth fails

### Protocol (bidirectional JSON)

**Frontend → Backend (incoming):**

```json
{ "type": "message", "content": "user text", "session_id": "optional", "space_id": "optional" }
```

**Backend → Frontend (token chunk):**

```json
{ "type": "token", "content": "chunk text", "session_id": "uuid", "space_id": "uuid" }
```

**Backend → Frontend (done signal):**

```json
{
  "type": "done",
  "session_id": "uuid",
  "space_id": "uuid",
  "phase": "Execute",
  "evaluation_passed": "true"
}
```

### Architecture

Two spawned tasks per connection:

1. **recv_task**: Subscribes to `WebChannel.outgoing_tx` (broadcast), forwards each `OutgoingMessage` as a "token" chunk followed by a "done" chunk
2. **send_task**: Reads from the WebSocket, parses JSON, constructs `IncomingMessage`, sends to `WebChannel.incoming_tx`

Message correlation uses a shared `pending_user_msg: Arc<Mutex<Option<(Uuid, PendingMessage)>>>` — the send_task records the message ID + content before forwarding; the recv_task matches it when the response arrives.

### Session persistence for WS

Session persistence happens in the **recv_task** (gateway → client direction), mirroring the POST handler logic. It persists the session to disk *before* forwarding to the WS client, ensuring durability even if the connection drops mid-stream.

---

## 6. User ID Handling

### POST `/api/chat`

The `ChatRequest` has an optional `user_id` field with **default value `"default"`**:

```rust
#[serde(default = "default_user")]
user_id: String,

pub(crate) fn default_user() -> String {
    "default".into()
}
```

So if the frontend doesn't send a `user_id`, it becomes `"default"`. The `IncomingMessage` is constructed as:

```rust
IncomingMessage::new("web", &body.user_id, &body.content)
```

### WebSocket `/api/chat/stream`

The WS handler **hardcodes user_id to `"web-user"`**:

```rust
let mut incoming = IncomingMessage::new("web", "web-user", content.clone());
```

And in the pending message tracking:

```rust
*pending = Some((incoming.id, PendingMessage {
    content,
    user_id: "web-user".to_string(),
}));
```

### Summary

| Path | user_id | Source |
|---|---|---|
| `POST /api/chat` (no user_id sent) | `"default"` | `ChatRequest::default_user()` |
| `POST /api/chat` (user_id sent) | Whatever client sends | Request body |
| `WS /api/chat/stream` | `"web-user"` | Hardcoded |

**The answer to "is it web-user?"**: Only for WebSocket connections. The POST endpoint defaults to `"default"` unless the client explicitly provides a `user_id`. The system does NOT use `"web-user"` for REST chat.

### Auth context

There is **no user authentication tied to user_id**. The auth middleware validates a bearer token (API key) but doesn't extract a user identity from it. The `user_id` is purely a client-provided label for session ownership.

---

## Architecture Summary

```
Frontend
  │
  ├─ POST /api/chat ─────────────┐
  │   (Json body)                 │
  │                               ▼
  ├─ WS /api/chat/stream ──── WebChannel (mpsc + broadcast + correlation map)
  │   (WebSocket JSON)            │
  │                               ▼
  └─ GET /api/events ──────── SSE (kernel event bus broadcast)
       (SSE stream)               │
                                  ▼
                           Gateway (dispatch with semaphore)
                                  │
                                  ▼
                           Orchestrator (Ouroboros protocol)
                                  │
                                  ▼
                           Response → correlation match → HTTP handler or WS broadcast
```

### Key files reference

| File | Responsibility |
|---|---|
| `plugin.rs` | WebPlugin factory, static assets, SPA, auto-download UI |
| `server.rs` | AppState definition |
| `channel.rs` | WebChannel + WebChannelHandle (mpsc/broadcast/correlation) |
| `error.rs` | AppError enum → HTTP status mapping |
| `middleware.rs` | Rate limiting + bearer token auth |
| `routes/mod.rs` | Route registration, pagination helpers |
| `routes/chat.rs` | POST chat + WS streaming + session persistence |
| `routes/events.rs` | SSE events + session CRUD + approvals |
| `routes/system.rs` | Health, readiness, status, agents, config |
| `routes/workspace.rs` | File tree, seeds, skills, memory CRUD |
| `crates/oxios-gateway/src/message.rs` | IncomingMessage / OutgoingMessage types |
| `crates/oxios-gateway/src/gateway.rs` | Gateway dispatch + concurrency control |
| `crates/oxios-kernel/src/state_store.rs` | Session persistence, pruning |
