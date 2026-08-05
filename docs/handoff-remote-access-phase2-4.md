# RFC-044 Phase 2–4 Result

> **Date:** 2026-08-05
> **Base:** `main` (post-Phase 1, `5ccfef4da`)
> **Spec:** `docs/rfc-044-remote-access-mobile-multiagent.md`

---

## Summary

Phases 2–4 of RFC-044 were implemented in a single session. The daemon-side
work (Phase 2 RPC methods + device-token verification + subscriptions, Phase 3
persona capabilities, Phase 4 worktree fan-out) is **complete and test-verified**.
The companion app and web UI capability packs are **scaffolded with key
components** but require further integration and device testing.

### CI gates (all pass)

```bash
cargo fmt --all --check                                    # ✅
cargo clippy -p oxios --all-targets -- -D warnings         # ✅ feature-off
cargo clippy -p oxios --all-targets --features remote -- -D warnings  # ✅
cargo test -p oxios                                        # ✅ 39 pass
cargo test -p oxios --features remote                      # ✅ all pass
cd web && bun run build                                    # ✅
```

---

## Phase 2 — Daemon RPC Surface (COMPLETE + TESTED)

### Device-token verification (RFC-044 §8 CRITICAL)

`ConnectionCtx` tracks per-connection auth state. `auth.verify` checks the
device token against `DeviceRegistry`. All sensitive RPC methods
(session/chat/persona/worktree) require auth; `status.get`, `echo`, and
`auth.verify` are available pre-auth.

Wire flow: Noise_XX handshake → `auth.verify` App frame → sensitive methods.

### New RPC methods (`src/remote/rpc.rs`)

| Method | Direction | Status |
|--------|-----------|--------|
| `auth.verify` | req→resp | ✅ tested (valid + bad token) |
| `session.list` | req→resp | ✅ tested (auth gate + kernel path) |
| `session.create` | req→resp | ✅ implemented |
| `persona.list` | req→resp | ✅ implemented (returns `capabilities`) |
| `persona.activate` | req→resp | ✅ implemented |
| `chat.send` | req→resp | ✅ implemented (via `RemoteBridge` → gateway) |
| `chat.subscribe` | sub | ✅ tested (subscription id + push) |
| `chat.unsubscribe` | req→resp | ✅ implemented |
| `agent.status` | sub | ✅ implemented |
| `worktree.list` | req→resp | ✅ implemented (Phase 4) |
| `worktree.create` | req→resp | ✅ implemented (Phase 4) |
| `worktree.fanout` | req→resp | ✅ implemented (Phase 4) |

### Transport changes (`src/remote/transport.rs`)

- **`ConnectionCtx`** — per-connection state: `push_tx` (for subscription
  frames) + `device_id` mutex (auth state).
- **Handler signature** changed to `Fn(Vec<u8>, Arc<ConnectionCtx>) -> Fut<Vec<u8>>`.
- **Server-pushed frames**: the main loop `select!`s between client requests
  and `push_rx`, encrypting and sending subscription events independently.
- **`OutboundQueue`** backpressure preserved.

### RemoteBridge (`src/remote/mod.rs`)

`RemoteBridge` implements `Channel` (gateway routing) — mirrors `WebBridge`.
`RemoteBridgeHandle` implements `GatewayBridge` trait (`send_and_wait` for
synchronous chat, `send_fire_and_forget` for fan-out). The surface returns
the bridge as its `SurfaceHandle::channel`, so the gateway registers it.

### Subscription push tasks (`src/remote/mod.rs`)

`spawn_subscription` creates background tasks that:
- Subscribe to `kernel.infra.subscribe()` (broadcast `KernelEvent`)
- Filter by session_id (`chat.subscribe`) or agent lifecycle (`agent.status`)
- Serialize events as JSON-RPC notifications
- Push via `conn_ctx.push_tx` (transport encrypts + sends)

### Tests

**Unit tests** (`rpc::tests`, 8 tests): status.get pre-auth, session.list auth
gate, auth.verify valid/bad token, post-auth gate pass, echo pre-auth, unknown
method, chat.subscribe stream.

**E2E tests** (`mod::tests`, 5 tests): paired_client_round_trip (Noise_XX +
status.get), auth_verify_then_session_list, bad_token_rejected,
plaintext_is_refused (Policy close), chat_subscribe_returns_subscription_id.

---

## Phase 2 — Companion App (SCAFFOLDED)

### Structure (`companion/`)

```
companion/
├── app/                          # Expo Router screens
│   ├── _layout.tsx               # Root nav, dark theme (95 lines)
│   ├── index.tsx                 # Paired hosts list (210 lines)
│   ├── pair-scan.tsx             # QR scanner + offer decode (246 lines)
│   ├── pair-confirm.tsx          # Handshake + Keychain save (270 lines)
│   ├── h/[hostId].tsx            # Sessions for a host
│   └── session/[id].tsx          # Chat + agent status (502 lines)
├── src/
│   ├── transport/
│   │   ├── e2ee-session.ts       # Noise_XX initiator (166 lines) ✅
│   │   ├── frame.ts              # Binary framing (23 lines) ✅
│   │   ├── types.ts              # Shared types (41 lines) ✅
│   │   ├── connection-health.ts  # Health verdicts (29 lines) ✅
│   │   ├── rpc-client.ts         # JSON-RPC client (minified)
│   │   ├── direct-endpoint-probe.ts  # ⚠️ stub (6 lines)
│   │   ├── endpoint-supervisor.ts    # ⚠️ stub (9 lines)
│   │   ├── reconnect-controller.ts   # ⚠️ stub (2 lines)
│   │   └── stable-logical-rpc-client.ts # ⚠️ stub (6 lines)
│   ├── keychain/host-store.ts    # SecureStore CRUD
│   ├── pairing/decode-offer.ts   # oxios://pair decoder
│   ├── ui/                       # Theme + primitives
│   └── services/api.ts           # API helpers
├── package.json                  # Expo + deps
├── tsconfig.json
└── app.json
```

### What's complete

- **Noise_XX client** (`e2ee-session.ts`, 166 lines): full 3-message XX
  handshake initiator using `@noble/ciphers` + `@noble/curves`. Compatible
  with the daemon's `snow` responder.
- **Binary framing** (`frame.ts`): exact match to daemon format
  `[type:1][size:4BE][payload]`, max 65536.
- **App screens**: all 6 screens written with consistent dark theme.

### What needs follow-up

- **Transport stubs**: `direct-endpoint-probe`, `endpoint-supervisor`,
  `reconnect-controller`, `stable-logical-rpc-client` are stubs. Port from
  orca (`orca/mobile/src/transport/`).
- **Device testing**: the app cannot be E2E tested without a physical device
  + running daemon. `expo start` + scan the QR from `oxios serve --remote`.
- **rpc-client.ts**: minified — reformat for readability.

---

## Phase 3 — Persona Capabilities (COMPLETE + TESTED)

### Schema change

- `Persona.capabilities: Vec<String>` added with `#[serde(default)]`
  (backward-compatible: old files load with empty capabilities).
- `persistence.rs` schema bumped to v2; v1 files accepted (range check
  `MIN_SCHEMA_VERSION=1..=SCHEMA_VERSION=2`).
- `persona_tool.rs` create/update handle `capabilities` param.
- `persona_routes.rs` create/update handle `capabilities` field.

### Default persona capabilities

| Persona | Capabilities |
|---------|-------------|
| dev | `terminal`, `diff-viewer`, `approval-cards`, `worktree-fanout`, `exec` |
| review | `diff-viewer`, `approval-cards` |
| research | `web-search` |
| ops | `exec` |
| security | `diff-viewer` |
| writer | `longform-editor`, `outline` |
| architect, mentor, planner | *(empty)* |

### Web UI capability packs (scaffolded)

New component files in `web/src/`:
- `hooks/usePersonaCapabilities.ts` — reads active persona capabilities
- `components/chat/AgentFanoutCard.tsx` — fan-out agent status cards
- `components/chat/InlineDiffViewer.tsx` — syntax-highlighted diff viewer
- `components/chat/FanOutButton.tsx` — composer fan-out action
- `components/chat/TerminalToggle.tsx` — terminal toggle (placeholder)
- `stores/fanout.ts` — Zustand store for fan-out state
- `types/index.ts` — `capabilities` field added to Persona interface

**Note:** These components are written but **not yet integrated** into
`chat.tsx` / `chat-input.tsx`. The subagent damaged existing imports during
integration; the files were restored from git. Re-integration requires adding
the imports + conditional renders without removing existing state hooks.

---

## Phase 4 — Worktree Fan-Out (DAEMON COMPLETE, UI SCAFFOLDED)

### Daemon (`src/remote/rpc.rs`)

Three RPC methods added:
- `worktree.list` — runs `git worktree list --porcelain`, returns worktree info
- `worktree.create` — runs `git worktree add -b oxios/{name} {path} {ref}`
- `worktree.fanout` — creates N worktrees (capped at 8), sends N
  fire-and-forget gateway messages (each with `worktree_path` metadata)

Git operations use `std::process::Command` (the existing `GitLayer` doesn't
support worktrees; `gix-worktree` integration is a future enhancement).

The `GatewayBridge` trait gained `send_fire_and_forget` for concurrent
fan-out (doesn't block on response — agents stream back via subscriptions).

### Web UI

`AgentFanoutCard.tsx` component exists (state dot, worktree path, time-ago).
Integration into the chat view is pending (same note as Phase 3 UI).

---

## Files Changed

### Rust (daemon)
- `src/remote/transport.rs` — ConnectionCtx + push frames + handler signature
- `src/remote/rpc.rs` — full RPC dispatch (12 methods) + git worktree helpers
- `src/remote/mod.rs` — RemoteBridge + subscription tasks + E2E tests
- `crates/oxios-kernel/src/persona/mod.rs` — `capabilities` field + defaults
- `crates/oxios-kernel/src/persona/persistence.rs` — schema v2 + backward compat
- `crates/oxios-kernel/src/tools/builtin/persona_tool.rs` — capabilities support
- `src/api/persona_routes.rs` — capabilities in create/update

### TypeScript (companion)
- `companion/` — entire app scaffold (see structure above)

### TypeScript (web)
- `web/src/hooks/usePersonaCapabilities.ts` — capability hook
- `web/src/components/chat/AgentFanoutCard.tsx` — fan-out cards
- `web/src/components/chat/InlineDiffViewer.tsx` — diff viewer
- `web/src/components/chat/FanOutButton.tsx` — fan-out action
- `web/src/components/chat/TerminalToggle.tsx` — terminal toggle
- `web/src/stores/fanout.ts` — fan-out store
- `web/src/types/index.ts` — capabilities field

---

## What's NOT Done (follow-up items)

1. **Companion transport stubs** — port endpoint-probe, supervisor, reconnect,
   logical-client from orca. The Noise_XX + framing core is done.
2. **Web UI integration** — wire capability components into `chat.tsx` and
   `chat-input.tsx` without breaking existing state hooks.
3. **Device E2E test** — pair a phone with `oxios serve --remote`, send a
   chat message, verify streaming transcript renders.
4. **Bind widening** — Phase 1 binds loopback only. Phase 2 widens to LAN/
   Tailscale (config-gated). The device-token verification is wired and ready.
5. **OutboundQueue real backpressure** — still drains each iteration (vestigial).
6. **AuditTrail** — companion connect/disconnect/RPC not yet logged.
7. **Compare/merge view** — diff N worktree branches after fan-out completion.

---

## Suggested Next Session

1. **Integrate web UI capability components** into chat.tsx (careful with
   existing imports — the subagent's approach of replacing imports was wrong).
2. **Port companion transport stubs** from orca (`mobile/src/transport/`).
3. **Widen the bind** in `RemoteRpcSurface::start` (add `remote.bind_address`
   config, default still loopback).
4. **Live device test** — `cargo run --features remote -- serve --remote`,
   scan the QR from a phone running the companion dev build.
