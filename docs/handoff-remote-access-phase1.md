# Phase 1 Handoff — Remote Access / Mobile Companion / Multi-Agent (RFC-044)

> **Status:** Handoff document at the Phase 1 → Phase 2 boundary of the RFC-044 program.
> **Phase 1:** ✅ complete and merged to `main` (HEAD `5ccfef4da`).
> **Spec:** [`docs/rfc-044-remote-access-mobile-multiagent.md`](rfc-044-remote-access-mobile-multiagent.md) (read §5–§6, §9, §10, §12 before starting Phase 2).
> **Reference project:** `orca` cloned at `/Volumes/MERCURY/PROJECTS/orca` (MIT). Join `lobehub-analysis/` as a living reference. Orca's companion transport (`mobile/src/transport/`) is the porting source for Phase 2.

---

## 1. Where Phase 1 Left Off

Phase 1 shipped the **daemon-side `RemoteRpcSurface`**: an opt-in, E2EE WebSocket surface that a paired client reaches directly over LAN/Tailscale, with QR pairing, device registry, Noise_XX transport, and an RPC method set. It is verified by an in-process E2E test (`status.get` over Noise_XX with a firing server-static pin). **No companion app exists yet** — Phase 2 builds it.

### Shipped (19 commits, `6443d06c3..5ccfef4da`, on `main`)

| Commit | What |
|--------|------|
| `980d443f1` | Scaffold `RemoteRpcSurface` + `RemoteConfig` + opt-in `remote` cargo feature |
| `713d5954e` | fmt + clippy cleanliness |
| `fe9e56db4` | Persistent Noise static keypair + device id (`identity.rs`) |
| `a79a50609` | `DeviceRegistry` — tokens hashed at rest |
| `b8c51ebdf` | Atomic write + propagate save errors (revoke/touch → `Result`) |
| `2c9fc6d4e` | Pairing offer encode/decode + QR (`pairing.rs`) |
| `64c0c00f0` | Endpoint enumeration + Tailscale classification (`endpoints.rs`) |
| `04e303a56` | Order-preserving dedup (Tailscale-first) |
| `6f5cabf1d` | Noise_XX responder + frame format (`noise.rs`) |
| `dc2a7453e` | Restore orphaned `pub mod pairing` |
| `faf7094b1` | WS transport + frame gate + backpressure (`transport.rs`) |
| `823f8dc65` | Tolerate transient accept errors in `run_listener` |
| `9b03fc660` | RPC dispatch + `status.get` + version gate (`rpc.rs`) |
| `c10b77487` | Wire `RemoteRpcSurface::start` + plaintext refusal (`mod.rs`) |
| `e002737b4` | `serve --remote` + readiness JSON + pairing-address (`serve.rs`, `main.rs`) |
| `0e7bddf5f` | Imply foreground when `--remote` |
| `6a6166328` | Gate `--remote` imply-foreground by the feature (restore feature-off) |
| `eaafcadc7` | In-process Noise_XX + `status.get` E2E + `examples/remote_probe.rs` |
| `5ccfef4da` | Harden identity 0600 + debug-first-run + drop redundant qrcode svg |

### Contracts established in Phase 1 (Phase 2 must consume these unchanged)

| Contract | Signature / value |
|---|---|
| Pairing offer URL | `oxios://pair?code=<base64url json {v,endpoint,device_id,public_key_b64,endpoints,scope}>` |
| Frame format | `[type:1][size:4 BE][payload]`, `FRAME_MAX = 65536`; `FrameType{Noise=0x01,App=0x02,Ping=0x03,Pong=0x04,Close=0x05}` |
| E2EE | `Noise_XX_25519_ChaChaPoly_SHA256` (snow). First frame MUST be `Noise` (else WS Close 1008). AEAD via implicit counters. |
| RPC | JSON-RPC over encrypted `App` frames. `status.get` → `{protocol_version:1, min_client_version:1, device_id, paired_count}`. Unknown method → `RpcError{code:-32601,message}`. |
| Surface gate | `RemoteRpcSurface::start` no-ops unless `config.remote.enabled`. Loopback bind only (Phase 1). |
| Registry | `DeviceRegistry::revoke/touch -> Result<...>`; `verify(device_id, token) -> bool` exists but is **not yet called on the wire** (see §8 CRITICAL). |
| Handler seam | `build_rpc_handler(Arc<RpcCtx>)` — shared by `start()`, the E2E test, and `remote_probe`. |

### CI gates (must pass before each commit; run BOTH feature configs)

```bash
cargo fmt --all --check
cargo clippy -p oxios --all-targets -- -D warnings                 # feature-off
cargo clippy -p oxios --all-targets --features remote -- -D warnings
cargo test -p oxios                                                  # default: 156 pass
cargo test -p oxios --features remote                                # remote: 188 pass
cd web && bun run build
```

The `remote` feature is **opt-in** (not in `default = ["web","cli","browser","sqlite-memory"]`). Default builds must stay byte-for-byte unaffected.

---

## 2. Phase 2 Scope — Native Companion (RN/Expo) MVP

Per RFC-044 §9 Phase 2: build the React Native + Expo companion that pairs with the daemon and gives mobile chat access. **Exit criterion:** real chat-from-phone over the tailnet/LAN (no terminal yet) within ~30 s of pairing.

### Sub-sessions (suggested decomposition)

| Sub-session | Scope | Est. commits | Difficulty |
|---|---|---|---|
| **2.A** Repo + transport port | `companion/` Expo Router scaffold; port orca `stable-logical-rpc-client` + `direct-endpoint-probe` + `endpoint-supervisor` (TS, nearly verbatim); Noise_XX client session + framing | ~6 | 🟡 |
| **2.B** Pairing flow | `pair-scan` (QR) → decode `oxios://pair` → E2EE handshake against `offer.endpoint`, pinning `public_key_b64`; persist host profile (Keychain) | ~4 | 🟡 |
| **2.C** Host list + session list | `index` (paired hosts), `h/[hostId]` (sessions via `session.list`/`session.create` — **new RPC methods, see §3**) | ~4 | 🟢 |
| **2.D** Chat screen | `session/[id]`: `chat.send` + `chat.subscribe` typed-block transcript stream; agent status; connection-health verdicts + Tailscale hint | ~6 | 🟡 |

### ⚠ Critical reconciliation before Phase 2 code

1. **Wire device-token verification (CRITICAL, before any sensitive RPC).** Phase 1's `dispatch` accepts any client that completes Noise_XX; `DeviceRegistry::verify` is not called. This is acceptable **only while the bind is loopback**. The moment Phase 2 widens the bind to LAN/Tailscale, **wire token verification into the connection path** (client presents its device token post-handshake; server verifies against the registry) BEFORE shipping `chat.send` or any sensitive method. This is a Phase 2.A prerequisite, not optional.
2. **New RPC methods needed** (§3): `session.list`, `session.create`, `chat.send`, `chat.subscribe`, `agent.status`. Phase 1 only implemented `status.get`/`echo`. The streaming methods (`chat.subscribe`) require a subscription/frame model — extend `rpc::dispatch` + the transport send path to push server-initiated frames.
3. **Noise client lib for RN** (open question, RFC §11.1): tweetnacl-based bespoke (orca style) vs a Noise_XX JS/JSI binding. The wire format (Phase 1 §6.3–6.4) is fixed; the client crypto lib is an implementation choice — spike in 2.A.

---

## 3. RPC Method Mapping (Phase 2 adds these to `rpc.rs`)

| Method | Direction | Phase 1 | Phase 2 | Notes |
|---|---|---|---|---|
| `status.get` | req→resp | ✅ | — | protocol version-gate |
| `echo` | req→resp | ✅ | — | test |
| `session.list` / `session.create` | req→resp | — | **add** | sessions carry `active_persona_id` |
| `persona.list` / `persona.activate` | req→resp | — | Phase 3 | multi-agent |
| `chat.send` | req→resp | — | **add** | enqueue user msg into a session (via gateway/orchestrator) |
| `chat.subscribe` / `unsubscribe` | **sub** | — | **add** | streaming typed-block transcript (reuse block-stream transparency pipeline, RFC-015/033) |
| `agent.status` | **sub** | — | **add** | working/blocked/waiting/done |
| `terminal.subscribe` | sub | — | Phase 3 (capability-gated) | coding persona only |

**Daemon work for 2.C/2.D:** extend `rpc::dispatch` with these methods; `chat.send`/`chat.subscribe` route through the existing gateway/orchestrator (the `WebSurface` already does this over its WS — mirror that bridge in the handler). Subscriptions need server-pushed frames: extend the transport loop to send encrypted frames not just in reply to requests.

---

## 4. Concrete File Layout

Phase 1 (shipped, `src/remote/`):
```
src/remote/
├── mod.rs         # RemoteRpcSurface + build_rpc_handler + start() + E2E tests
├── identity.rs    # Noise static keypair (0600 atomic)
├── devices.rs     # DeviceRegistry (hashed tokens, atomic write)
├── pairing.rs     # PairingOffer encode/decode + qr_svg
├── endpoints.rs   # enumerate_direct + classify (Tailscale/LAN)
├── noise.rs       # FrameType + encode/decode_frame + Responder + Transport
├── transport.rs   # run_listener(TcpListener,...) + OutboundQueue + plaintext refusal
├── rpc.rs         # RpcCtx + dispatch + RpcError + PROTOCOL_VERSION
└── serve.rs       # resolve_advertised + ReadinessContract
examples/remote_probe.rs   # live paired-client smoke probe
```

Phase 2 (new, `companion/` — monorepo subdir, mirroring how `web/` lives in-tree):
```
companion/
├── app/                       # Expo Router
│   ├── _layout.tsx
│   ├── index.tsx              # paired hosts list
│   ├── pair-scan.tsx          # QR scan + E2EE handshake
│   ├── pair-confirm.tsx
│   ├── h/[hostId].tsx         # sessions for a host
│   └── session/[id].tsx       # chat + agent status
└── src/
    ├── transport/             # ← port from orca/mobile/src/transport/
    │   ├── stable-logical-rpc-client.ts   # migrateTo(), subscription replay
    │   ├── direct-endpoint-probe.ts       # parallel race
    │   ├── endpoint-supervisor.ts         # hysteresis + foreground/background
    │   ├── reconnect-controller.ts        # recovery-gated backoff
    │   ├── e2ee-session.ts                # Noise_XX client (lib TBD)
    │   └── connection-health.ts           # verdicts + "check Tailscale" hint
    └── keychain/              # device token + host profile (expo-secure-store)
```

---

## 5. Key Code Anchors (read these before Phase 2)

### 5.1 `src/remote/mod.rs` — the surface + handler seam
`RemoteRpcSurface::start()` (gated on `config.remote.enabled`) loads identity+registry from `<workspace>/state`, binds `127.0.0.1:<remote.port>`, and spawns `transport::run_listener` with `build_rpc_handler(Arc<RpcCtx>)`. **Phase 2 widens the bind** (LAN/Tailscale) — see §8.

### 5.2 `src/remote/transport.rs::run_listener`
Signature: `run_listener(listener: TcpListener, server_static: Vec<u8>, shutdown: CancellationToken, handler: H)` where `H: Fn(Vec<u8>) -> Fut<Vec<u8>>` (decrypted App-frame bytes in → reply bytes out). Refuses non-Noise first frame (Close 1008). Tolerates transient accept errors. **Phase 2 subscription methods need this loop to push server-initiated frames** (extend beyond request/reply).

### 5.3 `src/remote/rpc.rs`
`dispatch(req: Value, ctx: &RpcCtx) -> Result<RpcOutcome, RpcError>`. `RpcCtx { registry, device_id, kernel: Option<Arc<KernelHandle>> }`. **Phase 2 adds session/chat/persona methods here** + wires `kernel: Some(...)` (Phase 1 leaves it `None` in tests).

### 5.4 `src/remote/noise.rs`
`Responder` (built from server static secret, completes XX) → `Transport { encrypt, decrypt }`. The Phase 2 Noise **client** mirrors this as an initiator (see Task 6's test + `examples/remote_probe.rs` for the initiator pattern over WS frames).

### 5.5 Orca porting source
`orca/mobile/src/transport/` — port `stable-logical-rpc-client.ts`, `mobile-direct-endpoint-probe.ts`, `mobile-endpoint-hysteresis.ts`, `mobile-relay-reconnect-controller.ts` nearly verbatim (drop the relay path — oxios has no cloud relay).

---

## 6. Companion ↔ Daemon Wire Contract (Phase 2)

Pair → the daemon prints `oxios://pair?code=...` (from `oxios serve --remote`). Companion decodes the offer, dials `offer.endpoints[]` in parallel, first Noise-XX-authenticated wins (pin against `offer.public_key_b64`). Connection path labels: `Direct · LAN` / `Direct · Tailscale` (no relay). Hysteresis: 3 successes/30s, 60s dwell, 60s cooldown. Unreachable `100.x`/`*.ts.net` → "Can't connect — check Tailscale".

---

## 7. Validation Plan (Phase 2.D exit)

### 7.1 In-process (extend the Phase 1 E2E)
Add RPC-method tests in `src/remote/mod.rs` for each new method (`session.list`, `chat.send` echo, etc.), reusing `build_rpc_handler` + a temp `RpcCtx` (kernel `None` for unit tests).

### 7.2 End-to-end smoke
1. `cargo run -p oxios --features remote -- serve --remote` (capture the printed `oxios://pair?code=` URL).
2. Companion dev build scans the QR (or paste the code), pairs.
3. Send "hello" from the phone → assert streamed transcript renders.
4. Switch network (LAN → Tailscale) → assert the logical client migrates without dropping the subscription.

### 7.3 Regression checklist
- [ ] Phase 1 `paired_client_round_trip_status_get` + `plaintext_is_refused_with_policy_close` still pass
- [ ] Default build (no `remote` feature) unaffected — `cargo test -p oxios` green
- [ ] Loopback HTTP API / WebSurface unchanged
- [ ] `remote_probe` example still compiles

---

## 8. Risks & Gotchas

| Risk | Status | Follow-up |
|------|--------|-----------|
| **No device-token verification on the wire** (Phase 1) | accepted (loopback-only) | **MUST wire before widening bind** — Phase 2.A prerequisite |
| Bind is loopback only (Phase 1) | by design | Phase 2 widens to LAN/Tailscale interfaces (config-gated) |
| `OutboundQueue` live-path backpressure is vestigial (drains each iteration) | deferred | Real backpressure matters for Phase 2 streaming (terminal/chat bursts) — accumulate before drain / gate on socket `bufferedAmount` |
| AuditTrail not wired (connect/disconnect/RPC) | deferred | Phase 2 wires it (RFC §6.7/§10) |
| `--remote` doesn't propagate to daemonized child | accepted | Daemon mode requires `[remote]` in config.toml; `--remote` implies foreground |
| Subagent `pub mod` clobber pattern (Phase 1 SDD) | resolved | When adding modules to `src/remote/mod.rs`, write the FULL block + read back |
| Live `oxios serve --remote` exited before readiness (during Phase 11 manual test) | env artifact | Likely instance-lock conflict with an already-running daemon; `oxios stop` first. Clearer error is a nice-to-have. |

---

## 9. Out of Scope for Phase 2

- **Cloud relay** (rejected — RFC §2; `orca`'s director+cells not adopted). Tailscale+LAN direct only.
- **Coding UI capability packs / terminal** — Phase 3 (`Persona.capabilities` field + coding pack).
- **Worktree fan-out** — Phase 4.
- **SSH-worktree remote-box execution** — future RFC (orca `relay.js` daemon pattern).
- **Tauri/Electron desktop shell** — browser is the desktop client; daemon is the host.

---

## 10. Suggested First Commit (Sub-session 2.A)

1. `git worktree add ../oxios-companion -b feat/remote-companion` (or work in-tree under `companion/`).
2. `cd companion && pnpm create expo-app .` (Expo Router); add `expo-secure-store`, the chosen Noise lib, `react-native-webview` (later), websocket client.
3. Port `stable-logical-rpc-client.ts` + `direct-endpoint-probe.ts` + `endpoint-supervisor.ts` + `reconnect-controller.ts` from `orca/mobile/src/transport/` (drop relay).
4. Implement the Noise_XX client session against the Phase 1 wire format (`noise.rs` framing). Verify it completes XX against `examples/remote_probe.rs`'s pattern.
5. **Daemon side (parallel, small):** wire `DeviceRegistry::verify` into the connection path (§8 risk #1) — this unblocks safe bind-widening.
6. Commit: `feat(companion): scaffold Expo app + ported transport (RFC-044 Phase 2.A)`.

---

## 11. Quick Reference: Phase Roadmap (RFC-044 §9)

| Phase | Scope | Status |
|---|---|---|
| 1 | RemoteRpcSurface (daemon, Rust) | ✅ merged `5ccfef4da` |
| **2** | **Native companion MVP (RN/Expo) — pair → chat** | **next** |
| 3 | Persona `capabilities` + UI capability packs + coding UX | pending |
| 4 | Worktree fan-out (parallel agents → compare → merge) | pending |

Each phase gets its own spec → plan → SDD execution cycle. Phases 3–4 specs should be written (brainstorming) before implementation.

---

## 12. Final Checklist Before "Phase 2 Complete"

- [ ] Companion pairs via QR and reaches the daemon over E2EE from the tailnet/LAN
- [ ] `chat.send` + `chat.subscribe` streaming transcript renders on mobile
- [ ] **Device-token verification wired on the connection path** (bind no longer loopback-only)
- [ ] Connection-health verdicts + Tailscale hint render
- [ ] Foreground/background reconnect works (Android Doze / iOS TCP-kill recovery)
- [ ] `cargo test -p oxios --features remote` green (incl. new RPC-method tests)
- [ ] Default build unaffected
- [ ] Phase 1 E2E + plaintext-refusal tests still pass
- [ ] This handoff updated / Phase 2 result doc written

---

End of handoff. Read this + `docs/rfc-044-remote-access-mobile-multiagent.md` §5–§6/§9/§10/§12 + the orca transport source (`orca/mobile/src/transport/`) and you're ready to start Phase 2.
