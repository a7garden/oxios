# RFC-038: Interactive Terminal (PTY-bridged WebSocket)

| | |
|---|---|
| **Status** | Proposed |
| **Author** | Oxios core |
| **Created** | 2026-07-08 |
| **Depends on** | RFC-024 (web daemon reliability), RFC-026 (kernel crate restructuring) |
| **Supersedes** | — |
| **Related** | `exec_api` (single-shot exec), `security_api` (WsTicket), `AccessGate` |

---

## 1. Motivation

Oxios today exposes host execution only through the agent-mediated **`exec` tool**:
LLM call → `ExecTool.execute` → one-shot process spawn → captured stdout/stderr streamed
back over the chat WebSocket. There is **no interactive, user-driven shell** — the
user can ask the agent to run commands, but cannot drive a shell themselves.

This is intentional and correct for agent workloads, but it leaves a gap for
operator workflows:

- Long-running foreground processes (`tail -f`, `cargo run`, dev servers) that
  want to be killed with Ctrl-C from the browser.
- Interactive programs (`vim`, `psql`, `python` REPL, `gh auth login`).
- Quick "what's the disk doing?" or "what port is bound?" glances.
- SSH-like operator access from any device on the LAN (phone, tablet).

We propose a **PTY-bridged WebSocket terminal** that lets the Web UI attach to
a real shell on the local host, mirroring the operator UX of VS Code's
integrated terminal, JetBrains Gateway, ttyd, or gotty — but inside the
Oxios Web UI, reusing the existing daemon, auth, and audit infrastructure.

The design reuses the daemon, `auth_enabled` + WsTicket, `AuditSink`, and the
JoinSet/keepalive transport pattern. It does **not** reuse `AccessGate` for
per-keystroke protection (a live shell cannot be gated that way); it adds a
coarser, session-boundary-only authorization model on top — see §3.

---

## 2. Goals & Non-goals

### 2.1 Goals

1. **Direct user-driven shell access** to the local host from the Oxios Web UI.
2. **Authenticated** through the existing `auth_enabled` + WsTicket model.
3. **Audited** — every PTY open and close is recorded via `AuditSink` (Merkle chain).
4. **Permissioned** — `AccessGate` gates who may **open** a session and which
   shell binary is selectable (`PtyConfig.allowed_shells`). It does **not**
   gate keystrokes inside the shell — that is the operator's responsibility.
5. **Bounded** — per-user session count cap, idle timeout, max session lifetime.
6. **Resizable** — SIGWINCH-equivalent propagated to the shell.
7. **Detachable** — user can leave and the session persists for a grace period
   (re-attach by session id) before being killed.

### 2.2 Non-goals

1. **SSH server** — we are not implementing `sshd`. The Web UI talks to the
   local daemon directly over WebSocket. External network reachability is a
   deployment concern, not a protocol concern.
2. **Multi-host fan-out** — one PTY session = one shell on the daemon's host.
   No `ProxyJump`-style chains, no `ControlMaster`-style multiplexing.
3. **Shell scripting UX** — no input autocomplete, no scrollback search,
   no copy-mode beyond what ghostty-web provides natively.
4. **Recorded session replay** — scrollback is in-memory; persisted history is
   out of scope (could become a future RFC if demand emerges).
5. **Cross-OS exotic shells** — we cover the POSIX path (zsh, bash, fish, dash)
   and Windows ConPTY via `portable-pty`. We do not implement custom
   protocol negotiation per shell.
6. **Terminal emulator** — we do not implement a VT100/xterm state machine.
   `ghostty-web` (libghostty compiled to WASM) handles ANSI parsing,
   cell rendering, scrollback, IME, mouse protocols, and palette. RFC §10.1.

---

## 3. Threat model

### 3.1 Adversaries

**Crucial framing.** A live PTY is **not** like a one-shot `exec` invocation.
`AccessGate` cannot inspect keystrokes, metacharacters, path traversals, or
command identity once the shell is running. It only gates two things at the
session boundary: **who may open** a session, and **which shell binary** is
selected. Everything inside the shell is the operator's responsibility.
This is a deliberate trade — see §7 for the controls that *do* apply, and
§15 for why we accept this asymmetry.

- **Unauthenticated network attacker**: tries to reach `/api/terminal/*`.

  Mitigated by `auth_enabled` + WsTicket + loopback bind (default).
- **WebUI XSS**: tries to send forged input frames.
  Mitigated by same-origin policy + ticket consumed at WS upgrade + per-session
  capability token (defense in depth; see §7.4).
- **Local attacker with file-system access**: could read `state/` but cannot
  forge tickets (cryptographically random, single-use).
- **Browser tab that lost connection**: orphan PTY leaks.
  Mitigated by idle timeout + max lifetime + GC sweep.

### 3.2 Out of scope

- A privileged local user who already controls the daemon process can trivially
  spawn whatever shell they want — there is no defence against root on the
  same host. We do not pretend otherwise. This is consistent with
  `AGENTS.md` "no containers, direct host execution".
- Side-channel attacks via PTY timing — out of scope.

---

## 4. Architecture

```
┌────────────────┐  WS frames   ┌────────────────────────────┐   PTY     ┌────────────┐
│ Browser        │ ───────────► │ oxios daemon (this RFC)    │ ────────► │ /bin/zsh   │
│ ghostty-web    │ ◄─────────── │                            │ ◄──────── │ (or chosen │
│ (wasm)         │  binary+text │                            │           │  shell)    │
└────────────────┘              └────────────────────────────┘           └────────────┘
       ▲                                  │
       │ HTTPS (existing)                 │
       │  POST /api/terminal/ticket       │
       │  GET  /api/terminal/stream?t=…   │
       │                                  │
       │                                  ▼
       │                         ┌────────────────────┐
       │                         │  PtyApi (new)      │
       │                         │  ├─ PtyManager     │
       │                         │  │  ├─ PtySession   │ ──► AuditSink (Merkle)
       │                         │  │  └─ PtySession   │
       │                         │  └─ idle GC        │
       │                         └────────────────────┘
       │                                  │
       │                                  ▼
       │                         ┌────────────────────┐
       │                         │ AccessGate         │
       │                         │ AccessManager RBAC │
       │                         │ AuditTrail (oxi-sdk│
       │                         │   re-exported)     │
       │                         └────────────────────┘
```

### 4.1 Component overview

| Component | Layer | New / reused |
|---|---|---|
| `portable-pty` crate (wezterm) | Backend dep | **New** |
| `crates/oxios-kernel/src/pty/` | Backend module | **New** |
| `PtyApi` | Kernel facade (14th typed API) | **New**, alongside `ExecApi` |
| `PtyManager` (sessions) | Kernel state | **New** |
| `AccessGate.check(Tool "pty")` | Security | **Reused** |
| `AuditSink` (Merkle + JSONL) | Audit | **Reused** |
| `SecurityApi.generate_ws_ticket` | Auth | **Reused** |
| `chat.ts::buildWsUrl` (ticket-first, token fallback) | Frontend WS auth | **Reused** (imported directly) |
| `chat.ts` reconnect (exp backoff `_reconnectTimer`/`_reconnectAttempts`) | Frontend WS resilience | **Extracted** to `ws-client.ts` (Phase D0), reused by both |
| `chat.ts` keepalive `_pingTimer` (RFC-024 SP2 B4) | Frontend WS keepalive | **Extracted** to `ws-client.ts` (Phase D0), reused by both |
| `chat.ts` send-queue + stale-connection teardown | Frontend WS queue | **Extracted** to `ws-client.ts` (Phase D0); chat protocol stays in chat.ts |

### 4.2 Why not reuse `ExecTool`?

`ExecTool` is intentionally **fire-and-forget** for LLM-driven single commands:

- One process, captured stdout/stderr strings, no resize, no signals.
- Owned by an `AgentContext`; tearing it down cleanly on tool timeout.

A PTY session is **stateful**:

- Bidirectional stream, SIGWINCH, Ctrl-C, raw bytes (color codes, escape sequences).
- Owned by a user identity, not an agent context.
- May live for hours, outliving any single agent.
- Multiple concurrent sessions per user.

Forcing PTY into `ExecTool` would require either (a) a parallel mode that
breaks its invariants or (b) a fork of `ExecTool` that shares little. The
cleaner cut is a sibling subsystem — same security gate, different lifecycle.

### 4.3 What we are *not* assembling from existing crates

Investigated Rust PTY↔WS ecosystem (RuTTY, termtty, etc.). They implement
PTY spawn + WS byte relay but lack: `AccessGate` RBAC, WsTicket single-use
auth, `AuditSink` Merkle, `KernelHandle` integration, hot-reloadable
config, re-attach by session id, env-strip list. We would re-wrap them
within weeks. Instead we write a thin adapter (§8, ~500 LoC) over
`portable-pty` that wires into Oxios's existing auth/audit/transport.

---

## 5. Data model

### 5.1 `PtyConfig` (added to `oxios-kernel/src/config.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PtyConfig {
    /// Master switch. Default `false`.
    #[serde(default)]
    pub enabled: bool,

    /// Default shell invoked when the client omits `shell` in the open frame.
    /// Defaults to `$SHELL` env at daemon startup, then `/bin/zsh`,
    /// then `/bin/bash`.
    #[serde(default = "default_pty_shell")]
    pub default_shell: String,

    /// Hard cap on concurrent PTY sessions per principal.
    /// Default 3.
    #[serde(default = "default_pty_max_sessions")]
    pub max_sessions: u32,

    /// Idle timeout in seconds. Resets on every input frame from the client.
    /// Default 1800 (30 min).
    #[serde(default = "default_pty_idle_secs")]
    pub idle_timeout_secs: u64,

    /// Hard lifetime in seconds. After this, the session is killed
    /// regardless of activity. Default 28800 (8 h).
    #[serde(default = "default_pty_max_lifetime_secs")]
    pub max_lifetime_secs: u64,

    /// Optional allowlist of shells. Empty = only `default_shell` allowed.
    /// Enforced via `AccessGate` Layer 3 like `ExecConfig.allowed_commands`.
    #[serde(default)]
    pub allowed_shells: Vec<String>,

    /// Optional working directory override. Empty = inherit daemon cwd.
    #[serde(default)]
    pub working_directory: Option<PathBuf>,

    /// Initial PTY size when the client doesn't send one.
    #[serde(default = "default_pty_size")]
    pub initial_size: PtySize,

    /// Environment variables passed to the shell, on top of the inherited env.
    /// `TERM=xterm-256color` is always set unconditionally.
    #[serde(default)]
    pub extra_env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PtySize {
    pub cols: u16,    // default 80
    pub rows: u16,    // default 24
    pub pixel_width: u16,
    pub pixel_height: u16,
}
```

`SharedPtyConfig = Arc<parking_lot::RwLock<PtyConfig>>` — hot-reloadable, same
pattern as `SharedExecConfig` (`exec_api.rs:12`). Patched via `PUT /api/config`
in `system.rs:1329` alongside `exec_api`.

### 5.2 `PtySessionId`

```rust
pub type PtySessionId = String; // ULID, monotonic, time-sortable.
```

ULID (not UUID) because we list/sort sessions in the UI ("recent sessions").
Crate already pulled in by oxi-sdk; no new dependency.

### 5.3 `PtySession` (in-kernel state)

```rust
pub struct PtySession {
    pub id: PtySessionId,
    pub principal: Principal,        // user/agent identity from AccessGate
    pub shell: String,               // resolved shell path
    pub created_at: Instant,
    pub last_input_at: Arc<AtomicU64>, // monotonic seconds, for idle GC
    pub state: Arc<Mutex<PtySessionState>>,
}

enum PtySessionState {
    /// Open and bound to a WebSocket client.
    Attached { ws_tx: mpsc::Sender<TerminalFrame> },
    /// No client attached; orphan, awaiting re-attach or GC.
    Detached { orphan_since: Instant },
    /// Exit code recorded; awaiting GC.
    Closed { exit_code: Option<i32>, at: Instant },
}
```

### 5.4 `TerminalFrame` (WebSocket message envelope)

All control frames are JSON (`Message::Text`); the **PTY payload is `Message::Binary`**.

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TerminalControl {
    /// Client → server, immediately after WS upgrade.
    Open {
        session_id: Option<PtySessionId>, // None = create new
        shell: Option<String>,            // None = default
        cols: u16,
        rows: u16,
    },
    /// Server → client, in response to Open.
    Opened {
        session_id: PtySessionId,
        shell: String,
        cols: u16,
        rows: u16,
    },
    /// Client → server.
    Resize { cols: u16, rows: u16 },
    /// Client → server. Server echoes back as `Exit` to confirm close.
    Close { reason: Option<String> },
    /// Server → client.
    Exit { code: Option<i32>, signal: Option<i32> },
    /// Server → client.
    Error { message: String },
}
```

PTY bytes flow as opaque WebSocket **binary** frames in both directions. No
base64, no JSON wrapping — keeps it cheap and lets the renderer consume them
directly via `term.writeBytes(Uint8Array)`.

---

## 6. Wire protocol

### 6.1 Sequence (new session)

```
client                              server (daemon)
  │                                       │
  │ POST /api/terminal/ticket             │   (reuses existing route shape)
  │ ─────────────────────────────────────► │
  │ ◄── { ticket: "wst_…" }               │
  │                                       │
  │ GET /api/terminal/stream?ticket=…     │
  │ ─────────────────────────────────────► │   WS upgrade
  │ ◄── 101 Switching Protocols            │
  │                                       │
  │ Text: Open { cols:80, rows:24 }       │
  │ ─────────────────────────────────────► │
  │                                       │  AccessGate check → portable_pty spawn
  │ ◄── Text: Opened { … }                │
  │                                       │
  │ ◄── Binary: PTY stdout (chunk 1)      │
  │ ◄── Binary: PTY stdout (chunk 2)      │
  │                                       │
  │ Binary: PTY stdin ("ls\n")            │
  │ ─────────────────────────────────────► │
  │                                       │
  │ Text: Resize { cols:120, rows:40 }    │
  │ ─────────────────────────────────────► │
  │                                       │  master.resize(PtySize)
  │                                       │
  │ (shell exits)                         │
  │ ◄── Text: Exit { code: 0 }            │
  │                                       │  close frame
```

### 6.2 Sequence (re-attach)

```
client                                          server
  │  GET /api/terminal/stream?ticket=…            │
  │ ─────────────────────────────────────────────►│
  │ ◄── 101 Switching Protocols                    │
  │                                               │
  │  Text: Open { session_id: "01H…" }             │
  │ ─────────────────────────────────────────────►│
  │      │  PtyManager.lookup(id) → Attached      │
  │ ◄── Text: Opened { … }                        │
  │      (replays last 64 KiB scrollback buffer)   │
  │ ◄── Binary: …                                 │
```

Re-attach is only valid within `max_lifetime_secs - now`. After that, the
session is already GC'd and the server returns `Error { message: "expired" }`.

### 6.3 Idle / lifetime GC

- Every `Binary` (input) frame: `last_input_at.store(now)`.
- Background tokio task spawned in `PtyManager::start()` ticks every
  `min(idle_timeout_secs / 4, 60)` seconds:
  - For each `Attached` session: if `now - last_input > idle_timeout_secs`,
    send `SIGTERM`, transition to `Closed`.
  - For each session in any state past `max_lifetime_secs`, hard kill.
  - For `Closed` sessions older than 5 minutes, remove from map.

This is intentionally **not** in the per-connection task — a single tick task
sweeps all sessions so we don't accumulate per-connection timer futures.

---

## 7. Security

### 7.1 Authentication

Identical to chat:

```
WS upgrade request:
  /api/terminal/stream?ticket=<one-time>

server:
  if config.security.auth_enabled:
      require ticket (preferred) OR token (fallback)
      SecurityApi.validate_ws_ticket(ticket)  // single-use, 30s TTL
  else:
      // local-first default; ticket still required to bind to a session
      // but ticket endpoint is open (consistent with chat today)
```

The ticket is **bound to the WS upgrade**, not the PTY session. Re-attach
requires a fresh ticket. This prevents a leaked ticket from being reused
once the connection drops.

### 7.2 Authorization (AccessGate integration)

`PtyManager::open()` calls `AccessGate.check(CheckRequest::Tool {
context: &ctx, tool_name: "pty" })` **before** spawning anything.

- **Layer 1+2**: `AccessManager.can_use_tool(ctx.agent_name, "pty")` —
  same RBAC machinery as `exec` (`gate.rs:330`).
- **Layer 3**: shell binary in `PtyConfig.allowed_shells` (or matches
  `default_shell` if list is empty) — same allowlist pattern as
  `ExecConfig.is_binary_allowed` (`gate.rs:484`).
- **Path sandboxing**: inherited from `AccessManager.allowed_paths`. The
  shell inherits the agent's permitted paths. Trying to `cd` outside is a
  bash-level problem; we do not intercept.

`PtyApi` exposes a new check `can_open_session(&ctx) -> bool` returning
`AccessGate.check(...)` result so the `POST /api/terminal/ticket` handler
can reject early with 403.

### 7.3 PTY → shell argument hardening

- `shell` parameter from the client is treated as a **single token**, validated
  with the same `validate_mcp_command` style blocklist (`infra.rs:301`) —
  reject shell metacharacters, whitespace, path traversal. Even though we
  pass it to `portable_pty::CommandBuilder::new(shell)`, we want belt and
  braces.
- No shell arguments are accepted from the client. To run `bash -l`, the
  user picks `bash` and configures their shell profile.

### 7.4 Per-session capability token

When `Open` succeeds, the server returns `Opened { session_id, … }`. The
session id is itself the capability token (128-bit ULID, unguessable). For
defense in depth we may add an HMAC over `(session_id, principal, expiry)`
in a follow-up RFC if abuse is observed; current threat model does not
require it.

### 7.5 Environment hardening

`PtyConfig.extra_env` is **added**, not replacing. We unconditionally set:

```
TERM=xterm-256color
COLORTERM=truecolor
OXIOS_PTY_SESSION=<id>
```

We **strip** daemon-only secrets before exec:

- `OXIOS_AUTH_*` (anything matching `^OXIOS_(AUTH|TOKEN|API_KEY)`).
- `OXIOS_HOME` (if it leaks `.oxi/` paths).

(Strip list is configurable; default above.) This prevents `env | grep OXIOS`
from leaking the daemon's own credentials to the user-driven shell.

### 7.6 Audit

Every PTY lifecycle event hits `AuditSink`:

| Event | When | Payload |
|---|---|---|
| `PtyOpen` | Session created | session_id, principal, shell, cols×rows, source_ip, ticket_id |
| `PtyAttach` | WS bound to session | session_id, principal |
| `PtyDetach` | WS closed without exit | session_id, reason, last_input_at |
| `PtyResize` | (rate-limited: 1/sec) | session_id, cols, rows |
| `PtyClose` | Session terminated | session_id, exit_code, signal, duration_secs, reason |

The `PtyOpen` event is the minimum required for audit. Resize and attach/detach
are recorded for forensics but not on every byte frame (that would be
prohibitive). We **do not** record PTY byte content — the kernel has no
business keylogging the user's shell. Consistent with `ExecTool` not
logging arguments either (`exec_tool.rs:295-340`).

### 7.7 Resource limits

- `max_sessions` per principal enforced in `PtyManager::open()`.
- `idle_timeout_secs` enforced by GC tick (§6.3).
- `max_lifetime_secs` enforced by GC tick.
- PTY read buffer: bounded at 64 KiB per WS frame (let `axum` chunk it).
- Input rate limit: 1 MiB/s per session (token bucket, in-kernel). Above
  that, excess is dropped with a `Warn` audit event. Prevents a
  misbehaving client from filling the PTY buffer. Implementation: evaluate
  `governor` crate (boinkor-net, GCRA/quota, Tokio-friendly); fallback to a
  ~30-line hand-rolled token bucket if we want zero new deps.

---

## 8. Kernel API surface

### 8.1 New `PtyApi` (14th typed API, alongside `ExecApi`)

```rust
// crates/oxios-kernel/src/kernel_handle/pty_api.rs

pub struct PtyApi {
    manager: Arc<PtyManager>,
    config: SharedPtyConfig,
    access_manager: Arc<parking_lot::Mutex<AccessManager>>,
    audit: Arc<dyn AuditSink>,
}

impl PtyApi {
    pub fn new(
        config: SharedPtyConfig,
        access_manager: Arc<parking_lot::Mutex<AccessManager>>,
        audit: Arc<dyn AuditSink>,
    ) -> Self;

    /// Open a new PTY session. Performs AccessGate check.
    pub async fn open(
        &self,
        ctx: &AgentContext,
        shell: Option<String>,
        cols: u16,
        rows: u16,
    ) -> Result<PtySessionHandle, AccessDenied>;

    /// Re-attach an existing session by id. Validates principal matches.
    pub fn attach(
        &self,
        ctx: &AgentContext,
        session_id: &PtySessionId,
    ) -> Result<PtySessionHandle, AttachError>;

    /// Send bytes to PTY stdin. Called by the WS handler on each Binary frame.
    pub fn write(&self, session_id: &PtySessionId, bytes: &[u8]) -> Result<(), PtyError>;

    /// Resize the PTY.
    pub fn resize(
        &self,
        session_id: &PtySessionId,
        cols: u16,
        rows: u16,
    ) -> Result<(), PtyError>;

    /// Signal the session (SIGTERM by default).
    pub fn close(&self, session_id: &PtySessionId, signal: Signal) -> Result<(), PtyError>;

    /// Subscribe to PTY stdout/stderr frames (broadcast). The WS handler
    /// receives from this.
    pub fn subscribe(&self, session_id: &PtySessionId) -> broadcast::Receiver<PtyFrame>;

    /// List active sessions for the principal (for the UI).
    pub fn list_sessions(&self, ctx: &AgentContext) -> Vec<PtySessionInfo>;

    /// Snapshot of config (for the WS handler to know defaults).
    pub fn config_snapshot(&self) -> PtyConfig;
}

pub struct PtySessionHandle {
    pub session_id: PtySessionId,
    pub shell: String,
    pub cols: u16,
    pub rows: u16,
    pub scrollback: Vec<u8>,  // last 64 KiB, for re-attach replay
}
```

### 8.2 `KernelHandle` change

```rust
// crates/oxios-kernel/src/kernel_handle/mod.rs
pub use pty_api::PtyApi;
pub use pty_api::{PtySessionHandle, PtySessionInfo};

pub struct KernelHandle {
    // ... existing 13 fields ...
    pub pty: PtyApi,  // NEW
}
```

`KernelHandle::new()` gains one new parameter (the `SharedPtyConfig`).
Call site is `src/kernel.rs` — single place to update.

### 8.3 `PtyManager` (in `crates/oxios-kernel/src/pty/manager.rs`)

```rust
pub struct PtyManager {
    sessions: parking_lot::RwLock<HashMap<PtySessionId, Arc<PtySession>>>,
    by_principal: parking_lot::Mutex<HashMap<Principal, HashSet<PtySessionId>>>,
    config: SharedPtyConfig,
    audit: Arc<dyn AuditSink>,
    shutdown: tokio::sync::watch::Receiver<bool>,
}

impl PtyManager {
    pub fn new(config: SharedPtyConfig, audit: Arc<dyn AuditSink>) -> Self;

    /// Spawn the GC tick task. Returns its JoinHandle for clean shutdown.
    pub fn start_gc(self: &Arc<Self>) -> tokio::task::JoinHandle<()>;

    /// Create a session, returning the handle + master PTY pair.
    pub async fn spawn(
        self: &Arc<Self>,
        ctx: &AgentContext,
        shell: Option<String>,
        cols: u16,
        rows: u16,
    ) -> Result<(PtySessionHandle, Box<dyn MasterPty + Send>), PtyError>;

    // ... lookup, list, close, etc.
}
```

The `Box<dyn MasterPty>` returned is `portable_pty::MasterPty`. The session
state holds the **master** end; the WS handler keeps a **clone** of the
master's reader/writer for direct I/O. This split is necessary because
`portable_pty`'s reader/writer types aren't `Send + Sync` — only the
`MasterPty` handle is.

### 8.4 PTY → WS plumbing

The WS handler maintains three concurrent tasks (same shape as
`handle_chat_websocket`):

| Task | Reads from | Writes to |
|---|---|---|
| WS recv | axum `WebSocket` | `PtyManager.write(...)` (stdin) |
| PTY → WS | `MasterPty::try_clone_reader()` (wrapped in `spawn_blocking`) | axum `Message::Binary` |
| WS control → PtyManager | axum `Message::Text` JSON | `PtyManager.resize/close` |

`JoinSet` is used so any task exiting tears the connection down. The
`keepalive_timeout` of 60 s (matching `chat.rs:1138`) detects half-open
connections. When the WS closes, we transition to `Detached` (not `Closed`)
so the user can re-attach within `max_lifetime_secs`.

---

## 9. HTTP / WebSocket endpoints

### 9.1 `POST /api/terminal/ticket`

Request: empty body. Auth via existing Bearer middleware.

Response:
```json
{ "ticket": "wst_<hex>" }
```

Handler: `handle_terminal_ticket` in new `src/api/routes/terminal.rs`. Same
shape as `handle_chat_ticket` (`chat.rs:451`). Calls
`SecurityApi::generate_ws_ticket`. Optionally pre-validates with
`PtyApi::can_open_session(ctx)` and returns 403 if denied.

### 9.2 `GET /api/terminal/stream`

Upgrade to WebSocket. Same `WsParams { ticket, token }` as chat.

Response on success: `101 Switching Protocols`.
On auth failure: `401 Unauthorized`.

Handler: `handle_terminal_stream` — calls `ws.on_upgrade(...)` and passes
control to `handle_terminal_websocket`.

### 9.3 `GET /api/terminal/sessions`

List active sessions for the calling principal. Useful for the UI's
"recent sessions" panel.

```json
{
  "sessions": [
    {
      "id": "01HXY…",
      "shell": "/bin/zsh",
      "created_at": "2026-07-08T10:23:45Z",
      "state": "attached" | "detached",
      "last_input_at": "2026-07-08T10:55:12Z",
      "cols": 120,
      "rows": 40
    }
  ]
}
```

### 9.4 Route wiring

In `src/api/routes/mod.rs`, alongside the existing chat routes:

```rust
.route("/api/terminal/ticket", post(handle_terminal_ticket))
.route("/api/terminal/stream", get(handle_terminal_stream))
.route("/api/terminal/sessions", get(handle_terminal_sessions))
```

---

## 10. Frontend

### 10.1 Dependencies

**Renderer choice: `coder/ghostty-web`.** libghostty (Ghostty's Zig core)
compiled to WASM, xterm.js API compatible. Reasons: GPU-accelerated
rendering, more accurate VT100/xterm spec coverage (Kitty graphics, Sixel,
ligature), and a single zero-dep ~400 KB wasm blob. Trade-off: ~400 ms
vs ~50 ms cold-load on `/terminal` (lazy-loaded so other pages are
unaffected).

```json
{
  "dependencies": {
    "ghostty-web": "^0.x"
  }
}
```

CSS import is bundled inside the wasm consumer (no separate CSS file like
xterm.js).

**Fallback plan for `@xterm/addon-fit`-equivalent.** ghostty-web is API
compatible but does not bundle a fit addon out of the box. Phase D step 1
verifies the community fit addon for ghostty-web; if absent or broken, we
write a ~20-line manual fallback inside `useTerminalSocket`:
`ResizeObserver(container) → measure-char-width × cols/rows → ws.send(resize)`.
This fallback lives *inside the hook*, so the renderer abstraction stays
clean regardless of which addon library we end up using.

**Why this is not contingent on validation (§15.9 RESOLVED).** User
explicitly chose ghostty-web despite the cold-load trade-off. The
FitAddon risk is absorbed into a known-cost task inside Phase D step 1
(see §15.9), rather than making the renderer choice itself contingent
on validation results.

### 10.2 Transport plumbing: extract once, use everywhere

**Insight.** The WS lifecycle in `web/src/stores/chat.ts` mixes two concerns:
the *transport* (WebSocket instance, reconnect timer, keepalive timer, stale-
connection teardown) and the *protocol* (chunk parsing, send queue, message
routing). The transport is generic — it knows nothing about chat messages.
The protocol is chat-specific. We need only the transport; the protocol stays
in `chat.ts` exactly as it is.

**Elegant solution:** extract the transport into `web/src/lib/ws-client.ts`
as a single hook `useWebSocketTransport(path, hooks)`. The hook owns `_ws`,
`_reconnectTimer`, `_reconnectAttempts`, `_pingTimer` — the four fields and
their lifecycle. It exposes: a stable `connected: boolean`, a `send()` that
works for both text JSON and binary `Uint8Array`, and a teardown returned
from the hook. Nothing about the chat protocol moves.

Then both call sites are one-liners:

```ts
// chat.ts (post-extraction): the entire `connect`/`disconnect`/`stopPingTimer`
// block becomes one hook call. The protocol layer above it (handleChunk,
// sendMessage, _sendQueue, seq cursor replay) is untouched.
export const useChatStore = create<ChatState>((set, get) => ({
  ...initial,
  _wsCtrl: null as null | WebSocketController,  // from useWebSocketTransport
  connect: async () => {
    const ctrl = useWebSocketTransport('/api/chat/stream', {
      onOpen:    (ws) => { /* replay cursor if any */ },
      onMessage: (msg) => { /* unchanged chat protocol dispatch */ },
      onClose:   () => set({ connected: false }),
      onError:   () => set({ connected: false }),
    })
    set({ _wsCtrl: ctrl })
  },
  disconnect: () => { get()._wsCtrl?.close(); set({ _wsCtrl: null }) },
  sendMessage: (content) => {
    const ctrl = get()._wsCtrl
    if (!ctrl?.isOpen()) { /* enqueue as today */ return }
    ctrl.send(JSON.stringify({ type: 'message', content }))
  },
  // ... rest of chat protocol stays put
}))
```

```ts
// web/src/components/terminal/useTerminalSocket.ts (new)
const ws = useWebSocketTransport('/api/terminal/stream', {
  onOpen: (ctrl) => ctrl.send(JSON.stringify({ type:'open', cols, rows })),
  onMessage: (msg) => {
    if (typeof msg === 'string') handleControlFrame(JSON.parse(msg))
    else term.writeBytes(msg)  // binary PTY chunk
  },
  // onClose/onError update local status state — see §10.3
})

// Hooks for the renderer (ghostty-web; xterm.js API compatible):
term.onData(str)      → ws.send(new TextEncoder().encode(str))
term.onResize(({cols, rows})) → ws.send(JSON.stringify({type:'resize', cols, rows}))
```

**Why this is the right shape:**

| Property | Status |
|---|---|
| Chat protocol (chunk parsing, seq replay, send queue) | Untouched |
| Reconnect / keepalive / teardown logic | One place only |
| Binary frame support | Built into the transport (controller exposes `send(data: string \| Uint8Array)`) |
| RFC-024 SP2 B4 keepalive semantics | Preserved bit-for-bit (extracted, not rewritten) |
| Future WS surfaces (voice, live agent monitor) | Use the same hook |
| Lines moved | ~150 out of chat.ts into ws-client.ts; chat.ts net -120 |
| Lines added for terminal | ~40 (terminal-specific protocol only) |
| Test coverage | ws-client.ts has its own unit tests; both call sites get them for free |

**Risk and safety net.** Extraction is a **behavior-preserving refactor** —
the *goal*, not a guarantee. We carry every existing field (`_sendQueue`,
`connected`, `isStreaming`, …) and method (`sendMessage`, `handleChunk`, …)
verbatim into the hook surface, wired through `_wsCtrl` in chat.ts. RFC-024
SP2 B4 keepalive and the existing exp-backoff reconnect algorithm are moved
unchanged into `ws-client.ts`. The risk surface is precisely where this
RFC's net value is: a single line moved wrong silently changes when chat
reconnects.

**Defence.** The existing `web/src/__tests__/stores.test.ts` (chat store
coverage: ~360 lines, `chat.ts:64AE`) is the regression net. We add a
dedicated `web/src/__tests__/ws-client.test.ts` covering reconnect backoff,
ping interval timing, teardown cancellation, and binary-frame send.
**PR1 lands only when both `stores.test.ts` and `ws-client.test.ts` are green.**
Until then chat.ts is untouched.

**Sequencing (PR1 then PR2, never bundled).**
- **PR1 — extract + migrate.** `ws-client.ts` lands; `chat.ts` migrates to
  use the hook; chat e2e green; PR can ship ahead of RFC-038 entirely. PR
  contains zero RFC-038 functionality.
- **PR2 — terminal feature.** Only after PR1 has been in main for ≥ 1 day
  with no chat regressions. PR2 introduces `useTerminalSocket`/`Terminal.tsx`
  consuming the hook. If PR1 produces a regression discovered later, PR2
  is unaffected and can be reverted independently.

This sequencing is the same shape as §16's rejected option C ("bundle both")
— the difference is that we *separate* them, not bundle them. The DRY win
is real; the safety win is sequencing.

**Order of operations (corollary):** Phase D0 = PR1; Phase D = PR2; never
merged into one PR, never landed in the same day.


### 10.3 `web/src/components/terminal/Terminal.tsx`

Layered on top of the transport hook. Responsibilities:

- Mount ghostty-web's `Terminal` on a `<div ref={containerRef}>`.
- Load the community fit addon (or fall back to manual ResizeObserver
  per §10.1; ~20 lines inside `useTerminalSocket`).
- `term.onData(str)` → `wsCtrl.send(new TextEncoder().encode(str))`
  (xterm emits strings; shell expects raw bytes).
- `wsCtrl.onMessage`: dispatch on `typeof msg === 'string'`
  → parse JSON control frame; otherwise → `term.writeBytes(msg)`.
- On control `Exit` → write a one-line notice to the terminal; status = `exited`.
  (the renderer cannot infer the shell exited from input alone).
- On control `Error` → surface to header status.
- `term.onResize(({ cols, rows }) => wsCtrl.send(JSON.stringify({type:'resize',
  cols, rows})))`.
- Re-attach: on `Detached` reconnect, send
  `{type:'open', session_id:<stored>, cols, rows}` so the server rebinds the
  existing PTY instead of spawning a new one.

Encoding note: the server side decodes UTF-8 back to bytes. This is the
convention ttyd uses and works for the common case (all printable shell
input is ASCII/UTF-8).

### 10.4 `web/src/routes/terminal.tsx`

A new TanStack Router route at `/terminal`. Renders a full-screen layout:

```
┌──────────────────────────────────────────────────┐
│  [Sidebar]   │   Terminal header (shell, status) │
│              │ ─────────────────────────────────│
│              │                                  │
│              │       <Terminal />                │
│              │                                  │
│              │                                  │
└──────────────────────────────────────────────────┘
```

- Header shows: shell path, session id (truncated), attach/detach indicator,
  reconnect button if detached, kill button.
- On mount: `POST /api/terminal/ticket` → open WS → send `Open`.
- On unmount: send `Close` if exit code is 0; else leave `Detached`.

`AppLayout` adds one branch to match Chat's full-bleed behavior:

```ts
const isTerminal = pathname.startsWith('/terminal')
// ...
) : isChat || isTerminal ? (
  <main className="flex-1 min-h-0 overflow-hidden">
    <Outlet />
  </main>
) : ...
```

### 10.5 Nav entry

Add one item to the existing **Operations** group in
`web/src/components/layout/sidebar.tsx:135` (no new group):

```ts
{
  labelKey: 'common.operations',
  items: [
    { labelKey: 'common.cronJobs', href: '/cron-jobs', icon: <Timer /> },
    { labelKey: 'common.cost', href: '/budget', icon: <Wallet /> },
    { labelKey: 'common.tokenMaxing', href: '/token-maxing', icon: <Flame /> },
    { labelKey: 'common.terminal', href: '/terminal', icon: <Terminal /> },
  ],
},
```

Reuses the existing `itemBase`/`itemActive`/`itemInactive` design tokens.

### 10.6 i18n

Both `web/src/i18n/locales/en.json` and `ko.json` (AGENTS.md bilingual):

- `common.terminal`: "Terminal" / "터미널"
- `terminal.title`: "Terminal" / "터미널"
- `terminal.connecting`: "Connecting…" / "연결 중…"
- `terminal.attached`: "Connected" / "연결됨"
- `terminal.detached`: "Detached (reconnect?)" / "연결 끊김 (재연결?)"
- `terminal.exited`: "Process exited" / "프로세스 종료"
- `terminal.killButton`: "Kill" / "강제 종료"
- `terminal.reconnectButton`: "Reconnect" / "재연결"
- `terminal.disabled`: "Terminal is disabled in settings" / "설정에서 터미널이 비활성화됨"
- `terminal.permissionDenied`: "Not authorized to open a terminal" / "터미널을 열 권한이 없습니다"

### 10.7 Accessibility note

xterm.js is canvas-based; screen-reader hostile by nature. Documented in
§15.6. Full a11y would require a DOM-based terminal (`termless`-class) which
is out of scope.

---

## 11. Configuration

### 11.1 `share/default-config.toml` diff

```toml
# Interactive Terminal (RFC-038)
[pty]
enabled = false
default_shell = "/bin/zsh"   # macOS default; daemon overrides with $SHELL if set
max_sessions = 3
idle_timeout_secs = 1800
max_lifetime_secs = 28800
allowed_shells = []          # empty = only default_shell
initial_size = { cols = 80, rows = 24 }
extra_env = {}

# Optional hardening
[pty.env_strip_prefixes]
prefixes = ["OXIOS_AUTH_", "OXIOS_TOKEN_", "OXIOS_API_KEY_", "OXIOS_HOME"]
```

### 11.2 Hot reload

`PtyConfig` joins `ExecConfig` in the `HOT_RELOADABLE_SECTIONS` table at
`system.rs:1381`:

```rust
("pty", "pty_api"),
```

`PATCH /api/config` deep-merges the new section and `system.rs:1329`-style
lines apply the change:

```rust
*state.kernel.pty.shared_config().write() = updated.pty.clone();
```

Each phase ends with a mergeable, demoable state.

### 12.0 Dependency ledger

```
backend (oxios-kernel/Cargo.toml):
  portable-pty = "0.8"            # wezterm, MIT
  governor    = "0.7"             # rate-limit PTY stdin (TBM in Phase A)

frontend (web/package.json):
  "ghostty-web"             = "^0.x"   # libghostty WASM, xterm.js API compatible (§10.1)

No new deps for: WebSocket (axum native), ULID (oxi-sdk pulls),
sync↔async (spawn_blocking), metrics (already wired), auth/audit/transport
(§4.1 reused).
```
New internal modules (no new external deps):
  web/src/lib/ws-client.ts       # useWebSocketTransport() — extracted from chat.ts (Phase D0)
  web/src/components/terminal/   # Terminal.tsx, useTerminalSocket.ts (Phase D)
  web/src/routes/terminal.tsx    # TanStack Router route (Phase D)
  crates/oxios-kernel/src/pty/   # PtyManager, PtySession (Phase A)
  crates/oxios-kernel/src/kernel_handle/pty_api.rs  # PtyApi (Phase A)
  src/api/routes/terminal.rs     # handlers (Phase B)

### Phase A — Backend skeleton (1 day)

1. Add `portable-pty = "0.8"` to `crates/oxios-kernel/Cargo.toml`.
   Decide on `governor` here (yes/no); if no, implement ~30-line token bucket.
2. Implement `crates/oxios-kernel/src/pty/`:
   - `mod.rs`, `manager.rs`, `session.rs`, `config.rs` (mirrors `exec`).
3. Add `PtyApi` to `kernel_handle/`, wire into `KernelHandle::new`.
4. New `[pty]` section in `OxiosConfig` with `enabled = false` default.
5. Unit tests: spawn `echo hello` → capture stdout → assert exit code.
6. Update `src/kernel.rs` to construct `PtyApi`.

**Acceptance**: `cargo test -p oxios-kernel pty::` passes. `cargo build` succeeds
with the new dep. PTY code is reachable but no HTTP routes yet.

### Phase B — HTTP routes (0.5 day)

1. `src/api/routes/terminal.rs` with three handlers.
2. Wire in `routes/mod.rs`.
3. Reuse `WsParams`, `handle_chat_ticket` flow.
4. Integration test with `axum::Router` + a mock WS client.

**Acceptance**: smoke test from `curl`/websocat hits `/api/terminal/stream`
and gets echoed stdout from `echo hello`.

### Phase C — Lifecycle polish (0.5 day)

1. GC tick task + idle/max-lifetime enforcement.
2. Re-attach path.
3. `GET /api/terminal/sessions` listing.
4. `AuditSink` integration tests.

**Acceptance**: open session, kill WS, re-open with same id, see scrollback.
Let it sit past `idle_timeout_secs`, see SIGTERM delivered.

### Phase D0 — Extract transport hook (0.5 day, can ship ahead)

1. Create `web/src/lib/ws-client.ts` with `useWebSocketTransport(path, hooks)`.
   Moves `_ws`, `_reconnectTimer`, `_reconnectAttempts`, `_pingTimer`,
   `connect()`, `disconnect()`, `stopPingTimer()` from `chat.ts`. Preserves
   RFC-024 SP2 B4 keepalive semantics bit-for-bit.
2. Migrate `chat.ts` to use the hook. **Behavior-preserving goal** (§10.2,
   §15.8): every field, method, and timing constant moves verbatim into the
   hook surface. The regression net is the existing `stores.test.ts` plus
   the new `ws-client.test.ts`. PR1 lands only when both are green.
3. Unit tests for `ws-client.ts` (reconnect backoff, ping interval, teardown
   cancellation, binary-frame send).

**Acceptance**: PR lands; chat e2e green; `ws-client.ts` covered by unit tests.
This PR is a self-contained refactor with **zero RFC-038 functionality** —
it can ship ahead of Phase D.

### Phase D — Frontend terminal page (1 day)

1. `bun add ghostty-web` in `web/`. Verify FitAddon-equivalent: try the
   community fit addon; if absent or broken, fall back to the ~20-line
   manual `ResizeObserver` implementation inside `useTerminalSocket`
   (§10.1).
2. `useTerminalSocket.ts` (uses `useWebSocketTransport` from Phase D0).
3. `Terminal.tsx` component.
4. `terminal.tsx` route.
5. Sidebar nav entry under Operations.
6. i18n keys (both locales).

**Acceptance**: open `/terminal`, see a working zsh prompt, type `ls`,
see output, Ctrl-C kills a foreground process. **Chat still works.**

### Phase E — Settings UI for `pty.*` (0.5 day)

1. Add `pty` section to `web/src/components/settings/field-defs.ts`.
2. `enabled` toggle, `max_sessions` range, `idle_timeout_secs` range,
   shell picker.
3. Hot-reload wired in.

**Acceptance**: toggle `enabled` in settings → next `/api/terminal/ticket`
call returns 200/403 accordingly without daemon restart.

### Phase F — Hardening & docs (0.5 day)

1. Rate limit on input.
2. Env-strip list.
3. `docs/api-reference.md` update.
4. `docs/USER-GUIDE.md` section on Terminal.
5. Threat model doc update.

**Acceptance**: `cargo clippy --workspace --all-features -- -D warnings` clean,
`cargo test --workspace` green, `bun run typecheck` green.

---

## 13. Failure modes & mitigations

| Failure | Detection | Mitigation |
|---|---|---|
| `portable-pty` fails to allocate (macOS `forkpty` denied) | `spawn()` returns `Err` | 500 + audit + clear error to client |
| Shell exits before client connects | PtyManager.poll exit watcher | Mark `Closed`, deliver `Exit` on attach |
| Client crashes mid-session | WS close frame / keepalive timeout | Transition `Detached`, GC after `max_lifetime_secs` |
| PTY buffer fills (slow consumer) | Backpressure on `mpsc` | Drop frames, audit `PtyBackpressure` once/sec |
| Daemon crashes mid-session | n/a | OS reaps PTY child, kernel next GC finds it gone |
| `auth_enabled=true` but no ticket | 401 at WS upgrade | Same as chat |
| Idle timeout while user is reading | Idle resets only on input, not output | Documented; user can ping ` ` to reset |
| Two clients race to attach same session | `Mutex<PtySessionState>` | Loser gets `Error { message: "busy" }` |
| Shell escapes to paths outside `allowed_paths` | bash-level concern | Document; future RFC could add syscall filter |
| Frontend WS plumbing duplicated in chat + terminal | Phase D0 extracts `useWebSocketTransport` first; chat migration is a separate PR with `stores.test.ts` as the regression net (§10.2, §15.8) |

---

## 14. Test plan

### 14.1 Unit (`crates/oxios-kernel/src/pty/`)

- `spawn_echo` — spawn `/bin/echo hello`, assert stdout == "hello\n", exit 0.
- `spawn_invalid_shell` — pass nonexistent shell, expect error.
- `allowed_shell_enforced` — set `allowed_shells=["/bin/bash"]`, try `/bin/zsh`, deny.
- `idle_gc` — set `idle_timeout_secs=1`, attach, sleep 2s, expect `Closed`.
- `max_lifetime_gc` — set `max_lifetime_secs=1`, expect close regardless of activity.
- `rate_limit` — flood input at 2 MiB/s, expect `Warn` audit, no OOM.
- `re_attach_replays_scrollback` — write 1000 lines, detach, re-attach, see first/last.

### 14.2 Integration (`crates/oxios-kernel/tests/pty_api.rs`)

- End-to-end through `PtyApi::open → write → subscribe → ...`.
- `AccessGate::check(Tool "pty")` denies an agent without permission.
- `AuditSink` mock receives `PtyOpen` and `PtyClose`.

### 14.3 HTTP (`src/api/routes/terminal.rs` test module)

- `ws_roundtrip` — mock WS client sends `Open`, receives `Opened`, sends
  bytes, receives echo.
- `auth_required` — no ticket → 401.
- `permission_denied` — agent with no `pty` tool → 403 on ticket.

### 14.4 Frontend (`web/src/__tests__/terminal.test.tsx`)

- Mount with mocked WS, assert `Open` sent.
- Mock incoming binary → assert `term.writeBytes` called.
- Mock `Exit` → assert header shows "Process exited".
- Resize → assert WS frame sent.
- `connectWs` extracted helper — covers chat.ts migration + terminal.

### 14.5 E2E (manual or playwright)

- Open `/terminal` in browser.
- Type `ls -la /tmp` — output appears.
- Run `top` in foreground, Ctrl-C — exits cleanly.
- Open second tab, attach to same session id — sees scrollback + live.
- Close daemon — see "Process exited" with signal 9.

---

## 15. Open questions

1. **Should PTY sessions survive daemon restart?**
   No in this RFC (process tree dies with daemon). If desired, we could
   serialize session metadata and `setsid` the child under PID 1 — but that
   contradicts "no containers" and adds significant complexity. **Decision
   proposed: no.**

2. **Recording for replay?**
   Already in non-goals. If demanded later, an `pty_recording: Option<PathBuf>`
   config field could turn on a binary log of PTY in/out, played back in
   xterm via a custom addon.

3. **Multi-user?**
   Oxios is single-user per daemon. `Principal` in this RFC means "the
   authenticated principal" — typically one. Multi-user is a bigger
   redesign and not on the roadmap.

4. **Windows ConPTY edge cases.**
   `portable-pty` wraps Windows ConPTY and behaves slightly differently
   around resize signals. We accept whatever the crate does; tested on
   macOS and Linux CI.

5. **Should the chat live activity show PTY sessions?**
   No — they're orthogonal. The agent doesn't see the user's shell
   unless the user pipes it back through chat, which is a separate UX.

6. **Accessibility.**
   xterm.js is canvas-based, screen-reader hostile. Full a11y requires a
   DOM-based renderer (out of scope). Document and provide keyboard
   shortcuts (`Ctrl+Shift+T` to focus terminal, `Ctrl+L` to clear).

7. **Per-session approval prompt?**
   Optional future enhancement: `pty.require_approval = true` makes the
   ticket endpoint return a `pending_approval` token; the user confirms in
   the Web UI, server then accepts the WS upgrade. Reuses the existing
   `tool-approval` machinery (`chat.rs:1551`). Adds ~50 LoC + UI. Not in
   this RFC.

8. **PR sequencing for the transport-hook extraction (§10.2).**
   The extraction of `useWebSocketTransport` from `chat.ts` is a
   behavior-preserving *goal*, not a guarantee. The risk surface is
   exactly where RFC-038's value is: a single line moved wrong silently
   changes when chat reconnects. **Mitigation by sequencing, not by
   hope.** PR1 = extract + migrate chat.ts (covered by existing
   `stores.test.ts` plus new `ws-client.test.ts`). PR1 ships to main and
   sits ≥ 1 day with no chat regressions *before* PR2 = terminal feature
   starts. PR1 and PR2 are never bundled. This is the same shape as §16's
   rejected option C, except *separated* in time.
   **Open:** if PR1 produces a regression discovered after main merge,
   revert PR1 (PR2 unaffected). If that's likely, consider landing PR1
   behind a feature flag (`useWebSocketTransport` opt-in for chat first)
   so it can be toggled off without revert. **Decision proposed:** no flag
   initially; add if PR1 regression rate ≥ 1/week during dogfood.

9. **Renderer choice: ~~xterm.js vs ghostty-web — validate before committing~~ (DECIDED: ghostty-web).**
   Was a validation gate; user explicitly chose ghostty-web despite the
   trade-offs (cold-load ~400 ms vs ~50 ms, bundle ~400 KB vs ~250 KB, no
   production validation history vs 12 years). §10.1 commits ghostty-web
   as the renderer for RFC-038. The FitAddon risk is absorbed into a
   known-cost task in Phase D step 1: try the community fit addon; if
   absent or broken, write a ~20-line manual `ResizeObserver` fallback
   inside `useTerminalSocket`. The fallback lives inside the hook so the
   renderer abstraction stays clean.

---

## 16. Alternatives considered

### A. WebSSH (browser-based SSH client → external sshd)

Rejected. Adds a hop, requires a real sshd, requires key distribution, and
gives no advantage over a direct WebSocket to the daemon. The user wanted
"ssh 같은 UX"; the *protocol* doesn't have to be SSH to give that UX.

### B. Polling-based output (no WebSocket)

Rejected. SSE-only is unidirectional; we need bidirectional for input and
resize. Long-poll is worse on latency. WebSocket is the obvious choice
and already in the stack.

### C. Extend `ExecTool` with a `pty: true` flag

Rejected (§4.2). Different lifecycle, different ownership, different audit
shape. Cleaner as a sibling.

### D. Use `ttyd` as a sidecar

Rejected. Process management overhead, duplicate auth (ttyd has its own
basic-auth cookie), no audit integration, no AccessGate.

### E. Use `wasm-terminal` running a full shell in WASM in the browser

Rejected. Doesn't actually run on the host — defeats the purpose. Also
limited to JS shells, not the user's real `$SHELL`.

### F. Adopt an existing Rust PTY-WS crate (RuTTY, termtty, etc.)

Rejected (§4.3). They implement the relay but lack Oxios-specific auth,
audit, session, and config integration. Re-wrapping would cost more than
the ~500-line adapter we already plan to write.

---

## 17. Rollout

1. Land Phase A–C behind `[pty] enabled = false` default.
2. Land Phase D (frontend) gated behind a feature flag in `web/src/main.tsx`
   so the nav entry doesn't show until explicitly enabled.
3. Manual dogfood on local macOS for 1 week.
4. Enable by default for `auth_enabled = false` (local-first installs),
   leave disabled when `auth_enabled = true` until ops signs off.
5. After 2 weeks, document in CHANGELOG and announce.

---

## 18. References

- `crates/oxios-kernel/src/tools/exec_tool.rs` — closest existing pattern.
- `crates/oxios-kernel/src/kernel_handle/exec_api.rs` — sibling API shape.
- `crates/oxios-kernel/src/access_manager/gate.rs:484` — allowlist enforcement.
- `src/api/routes/chat.rs:584` — JoinSet + keepalive pattern.
- `src/api/routes/chat.rs:442` — `WsParams` ticket model.
- `web/src/stores/chat.ts:511` (`buildWsUrl`), `:842` (reconnect), `:560` (keepalive) — WS plumbing to extract.
- `crates/oxios-kernel/src/kernel_handle/security_api.rs:59` — ticket generation.
- `src/api/routes/system.rs:1329` — hot-reload config wiring.
- `web/src/components/layout/app-layout.tsx:104` — `isChat` full-bleed branch (mirror for `isTerminal`).
- `web/src/components/layout/sidebar.tsx:135` — Operations nav group.
- `AGENTS.md` "no containers, direct host execution" — threat model alignment.
- `wezterm/portable-pty` — backend PTY crate (MIT).
- `coder/ghostty-web` — committed WASM terminal renderer (§10.1; libghostty, xterm.js API compatible).
- `governor` — Rust rate-limit crate (GCRA, Tokio-friendly).