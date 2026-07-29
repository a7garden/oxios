# Managed Relay Remote Access — Architecture

> **Status:** Architecture (level-0). Decomposed into 5 sub-specs.
> **Date:** 2026-07-29
> **Scope:** How an Oxios host (user's Mac) is securely reached from any device (iPhone, browser on another machine) without the user installing Tailscale or operating reverse proxies.
> **Supersedes:** [`2026-07-29-remote-access-architecture-design.md.superseded`](2026-07-29-remote-access-architecture-design.md.superseded) (Tailscale-Serve-as-default → managed relay).
> **Decomposes into:** 5 sub-specs (see §9). This document is the contract between them; each sub-spec owns one subsystem.

---

## 1. Problem

A user wants to reach their local Oxios daemon from an iPhone on LTE — at a café, on a trip, from a different WiFi. Today:

- The daemon binds `127.0.0.1:4200`; only the host machine's browsers can reach it.
- The user runs Tailscale on both Mac and iPhone to bridge them. This works, but requires every user to install and join a tailnet before using Oxios.
- The current remote-access design doc (now superseded) proposed Tailscale Serve as the recommended path. We are pivoting: Tailscale is a power-user fallback, not a default.

The product question: **can a non-technical user install Oxios and use their phone to reach it, with zero VPN/client setup?** The answer this doc commits to is **yes**, via a small managed relay that Oxios operates.

## 2. Goals and non-goals

### Goals

- **G1.** A user with a freshly installed `oxios` binary can run `oxios serve --tunnel` and reach the daemon from any browser (iPhone Safari, laptop on another network) within ~30 seconds.
- **G2.** Zero inbound port exposure on the host. Only outbound 443 is opened. NAT, CGNAT, and ISP firewalling are irrelevant.
- **G3.** All payload that flows through the relay is end-to-end encrypted. The relay operator (Oxios) cannot read chat content, file paths, shell commands, tool inputs/outputs, or API keys.
- **G4.** The user's state (sessions, memory, knowledge, config) remains 100% local on the host. The relay stores only metadata (which user owns which device, last-seen, audit events).
- **G5.** The host binary keeps working offline (Tier 0). The tunnel is additive, not required.

### Non-goals

- **N1.** Cloud-hosted Oxios. State does not move. (LobeHub-style SaaS is not in scope — see supersede note in §11.)
- **N2.** Multi-user tenancy on a single Oxios instance. One user, many devices.
- **N3.** Sharing the dashboard with people who are not the owner. (Tailscale Funnel as a temporary share path is still valid; see supersede doc §4 Tier 2.)
- **N4.** Self-hosting the relay on a personal VPS in this iteration. The relay lives at `relay.oxios.com`. The relay code may be open-sourced later; that is a separate decision.
- **N5.** Replacing the existing loopback auth (`/api/auth/issue`, Bearer token). Tunnel auth is a separate, additive layer.

## 3. Trust model

```
┌────────────────────────────────────────────────────────────────┐
│  untrusted                                                    │
│    Internet, ISP, transit networks, Cloudflare edge           │
│      ↓ HTTPS (TLS terminated at Cloudflare edge)              │
│  semi-trusted: relay.oxios.com                                │
│    Sees:  ciphertext bytes, user_id, device_id, sizes,         │
│           timing, source IP                                    │
│    Cannot: decrypt payload, forge messages, replay (AEAD),     │
│            see user content                                    │
│      ↓ authenticated routing, no decryption                   │
│  trusted: User's Mac (oxios binary)                           │
│    Holds: device static key (Ed25519+X25519),                 │
│           device_token (encrypted with platform keychain),     │
│           source of truth for ~/.oxios/*                      │
│      ↔ E2E encrypted tunnel (Noise_XX_25519_ChaChaPoly_SHA256) │
│  trusted: User's Browser (app.oxios.com)                      │
│    Holds: ephemeral X25519 key, OAuth access_token,           │
│           decrypts payload to display                          │
└────────────────────────────────────────────────────────────────┘
```

**Three security boundaries matter:**

1. **Network → relay:** TLS by Cloudflare. We trust Cloudflare to terminate TLS correctly. The relay payload is then *re-encrypted* with our own Noise session on top — defense in depth.
2. **Relay → host / browser:** the Noise session. The relay is an opaque pipe. It cannot read or modify the inner stream.
3. **OAuth identity vs. device identity:** OAuth (GitHub/Google) proves *who* the human is. Device keypair proves *which* machine is the user's. Both required: a stolen GitHub token alone cannot connect without a device token, and a leaked device token alone has no user attached.

## 4. Architecture overview

```
┌────────────────────────────────────────────────────────────────────────┐
│  oxios.com  (Cloudflare account: oxios-ops)                            │
│                                                                        │
│  ┌─────────────────────────┐  ┌──────────────────────────────────────┐  │
│  │ app.oxios.com           │  │ auth.oxios.com                       │  │
│  │ SPA (Cloudflare Pages)  │  │ OAuth broker (Cloudflare Worker)    │  │
│  │ - static React build    │  │ - GitHub / Google OAuth              │  │
│  │ - same-origin as relay  │  │ - Device Code Flow (RFC 8628)        │  │
│  │   via /api/* reverse    │  │ - token rotation                     │  │
│  │   proxy or CORS         │  └──────────────────────────────────────┘  │
│  └─────────────────────────┘                                            │
│                              ↕  D1 (account/device metadata)            │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ relay.oxios.com  (Cloudflare Worker + Durable Object)           │  │
│  │  - TLS terminate                                                 │  │
│  │  - OAuth + device_token verify                                    │  │
│  │  - Per-user Durable Object holds device↔connection routing table │  │
│  │  - Forwards opaque (post-Noise) bytes; cannot decrypt            │  │
│  │  - Rate limit, audit log                                         │  │
│  └──────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
                                ▲ outbound 443 only (WebSocket)
                                │
┌────────────────────────────────────────────────────────────────────────┐
│  User's Mac  (oxios binary, target: aarch64-apple-darwin)              │
│  ┌──────────────────────────────────────────────────────────────────┐ │
│  │ tunnel module  (new)                                             │ │
│  │   - device keypair  (Ed25519+X25519, keychain-wrapped)            │ │
│  │   - device_token    (encrypted at rest)                          │ │
│  │   - WebSocket outbound client (tokio-tungstenite)                │ │
│  │   - Noise_XX handshake                                            │ │
│  │   - Reconnect with backoff + jitter                              │ │
│  └──────────────────────────────────────────────────────────────────┘ │
│  ┌──────────────────────────────────────────────────────────────────┐ │
│  │ existing daemon (unchanged)                                      │ │
│  │   - bind 127.0.0.1:4200                                          │ │
│  │   - AccessManager / orchestrator / memory / skills / ...         │ │
│  │   - new: WebSurface accepts inbound proxied requests via a       │ │
│  │     tunnelled bridge (same handler chain as local)               │ │
│  └──────────────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────┘
```

The host binary's tunnel module is a *new* module that wraps (does not replace) the existing axum server. The local listener stays on loopback. The tunnel module owns the outbound connection and demultiplexes inbound tunneled traffic to the same `Router` instance the local server uses.

## 5. User flows

### 5.1 First-time setup on a new Mac

```
$ oxios serve --tunnel

  Oxios first run.
  Choose a mode:
    [1] Local only        (this Mac only)
    [2] Anywhere via oxios.com  (recommended)
  Select [2]: _

  ↳ Generating device keypair... done
  ↳ Registering with auth.oxios.com...

  ┌────────────────────────────────────────────────┐
  │  Open this URL on any browser to link device:  │
  │                                                │
  │    https://auth.oxios.com/device/ABCD-1234     │
  │                                                │
  │  or scan:                                      │
  │  ┌──────────────┐                              │
  │  │  █▀▀▀▀▀█▀▀█ │                              │
  │  │  █ ▀▀▀ ▀█▀▀█ │                              │
  │  │  ▀▀▀▀▀▀ ▀  ▀ │                              │
  │  └──────────────┘                              │
  └────────────────────────────────────────────────┘

  ↳ Waiting for confirmation (timeout 10 min)...
  ↳ Linked to user alice@github.com
  ↳ Device: alice-macbook
  ↳ Tunnel: outbound 443 to relay.oxios.com ✓
  ↳ Reachable at: https://app.oxios.com/devices/alice-macbook
```

The user never touches a config file, never pastes a token, never sees a YAML/JSON.

### 5.2 Reaching the daemon from an iPhone

```
1. iPhone Safari → https://app.oxios.com
2. "Continue with GitHub" → OAuth → back to app
3. Devices list (https://app.oxios.com/devices) shows: 🍎 alice-macbook (online ●)
4. Tap → SPA opens WebSocket to relay, performs `browser_hello` (Contract 7.6).
   The relay's per-user Durable Object (Contract 7.7) routes the browser to the
   host and emits a frame type 0x11 (browser-attached) so the host knows a
   browser is waiting.
5. The host initiates a Noise_XX handshake (Contract 7.4: host = initiator,
   browser = responder; in Noise_XX the initiator always writes message 1).
   Handshake bytes (frame type 0x01) are relayed opaquely.
6. With the Noise transport established, the SPA proxies API calls to the host
   through the same `Router` the local listener uses (Contract 7.5). Chat,
   knowledge, skills, tools — all functional.
```

### 5.3 Offline / reconnect

If the Mac sleeps, the WebSocket drops. On wake, the binary reconnects with exponential backoff (1s, 2s, 4s, …, capped at 5 min, ±20% jitter). The browser side shows "offline" until the device is back. No data is lost (no payload was stored).

## 6. Components (high-level)

The system has five subsystems. Each becomes a sub-spec (§9).

| # | Subsystem | Domain | Sub-spec |
|---|---|---|---|
| 1 | OAuth broker | `auth.oxios.com` | A |
| 2 | Host tunnel | inside `oxios` binary | B |
| 3 | Relay server | `relay.oxios.com` | C |
| 4 | SPA relay integration | `app.oxios.com` (the existing React build) | D |
| 5 | E2E crypto layer | shared by B, C, D | E |

## 7. Sub-system contracts (the interfaces between sub-specs)

The sub-specs are independently buildable because they agree on the following contracts. **This section is normative: any sub-spec that violates a contract here is wrong.**

### Contract 7.1 — Device identity

A device is identified by an Ed25519 public key fingerprint (32 bytes hex). The same key also serves as the X25519 static key for Noise. The host binary generates this at first run; the public half is sent to the OAuth broker during registration. The private half never leaves the host and never leaves the platform keychain (see §7.2).

### Contract 7.2 — Key storage on the host

- `~/.oxios/device.key`: keychain-wrapped private key material. Plaintext bytes only ever in memory.
- `~/.oxios/device.token`: long-lived JWT issued by the OAuth broker after successful device code confirmation. Encrypted at rest using a key derived from a platform keychain entry (macOS Keychain, Linux libsecret).
- File modes: 0600. Process drops privilege to user-only on read.

**Keychain unlock at boot.** Sub-spec B must handle the case where the platform keychain is locked (e.g., macOS launchd starts the daemon before user login). Strategy: on first `oxios serve --tunnel` invocation under a logged-in user, the binary prompts the user once to authorize keychain access (one-time Touch ID / password). The unwrapped key is then cached in process memory only; subsequent restarts within the same user session use the cached value. On user logout or daemon restart under a different session, re-prompt. A non-keychain fallback (file-based key derived from a passphrase the user enters at first run) is permitted but keychain is preferred.

### Contract 7.3 — WebSocket frame format (between host/broker and relay)

A binary WebSocket message, single frame per logical message (no fragmentation at the application layer):

```
┌──────────┬─────────────┬─────────────────┐
│  type    │  size (BE)  │  payload        │
│ 1 byte   │  4 bytes    │  size bytes     │
└──────────┴─────────────┴─────────────────┘
```

`size` is the payload length in bytes, big-endian u32. **Maximum payload size is 64 KiB** (frames larger than this MUST be rejected by the relay with a close code 4xxx). This cap exists because the relay is a thin forwarder; oversized frames indicate a bug or attack. Application payloads larger than 64 KiB (rare) are split at the app-protocol layer (Contract 7.5) and reassembled by the Noise transport on the other side.

`type` discriminates:
- `0x01` Noise handshake (`payload` = raw Noise message bytes)
- `0x02` Encrypted app frame (`payload` = Noise transport message)
- `0x03` Ping (heartbeat, every 30s; payload empty)
- `0x04` Pong
- `0x05` Close reason (`payload` = UTF-8 string, ≤ 256 bytes)
- `0x10` Control: routing hint from relay (e.g., target device_id; payload is JSON)
- `0x11` Control: browser-attached notification (relay → host; tells the host a browser wants to start a Noise session)
- `0x12` Control: browser-detached notification (relay → host; browser gone)
- `0x20` Control: device-revoked (relay → host; tunnel must stop trying and require re-login)

The relay must NOT inspect or interpret the **payload** of types `0x01` or `0x02` (it forwards them opaquely). Types `0x10`, `0x11`, `0x12`, `0x20` carry routing or control metadata that the relay itself produces or interprets; the host and browser treat them as relay-defined control, not user content. Heartbeat (`0x03`/`0x04`) is relay-internal and the relay may short-circuit them.

### Contract 7.4 — Noise session

`Noise_XX_25519_ChaChaPoly_SHA256`. Initiator = host binary. Responder = browser SPA. **In Noise_XX, the initiator always writes the first handshake message; the responder replies.** Therefore the host (initiator) writes `-> e, es` first, the browser (responder) writes `<- e, ee, se` next, and so on through the three-message XX pattern. The relay sees handshake bytes only; the static keys are never revealed to the relay.

After handshake: both sides have a transport cipher state. Re-key every 2^16 messages or 1 hour, whichever first. Re-handshake on transport error.

### Contract 7.5 — App-level payload (after Noise decrypt)

JSON envelope (this is what the host's existing `WebSurface` already speaks; relay integration does not invent a new protocol):

```json
{
  "req_id": "uuid",
  "kind": "request" | "response" | "chunk" | "error" | "close",
  "method": "POST",
  "path": "/api/chat/stream",
  "headers": { "...": "..." },
  "status": 200,
  "chunk_index": 0,
  "final": false,
  "body_b64": "..."
}
```

`req_id` is stable across the request and all its response chunks; `chunk_index` is a monotonic counter starting at 0; `final: true` marks the last chunk. The same JSON envelope works in both directions (browser → host and host → browser). For SSE / streaming endpoints, the host emits one envelope per chunk; for unary endpoints, exactly one response envelope with `final: true`. The envelope itself is always JSON and is wrapped in a Noise transport message; the Noise layer further enforces the 64 KiB per-frame cap (Contract 7.3) by rejecting envelopes whose serialized form exceeds it. **The host reuses its existing axum `Router`** by adapting each decrypted envelope to a `Request<axum::body::Body>` and forwarding to a private `tower::Service` handle; the response stream is read chunk-by-chunk and re-enveloped. No second router.

### Contract 7.6 — Auth handshake on the WebSocket

When the host opens the WebSocket to `relay.oxios.com`, the first frame is a control message (relay-defined, NOT Noise — this is transport-level auth before the Noise session begins):

```json
{
  "type": "device_hello",
  "user_id": "github:12345",
  "device_id": "<hex of device public key>",
  "device_token": "<JWT>",
  "client_version": "oxios 0.x.y"
}
```

The relay validates `device_token` (signature, exp, user_id match). On success, replies with `device_welcome` carrying the per-user Durable Object's instance id (an opaque string the relay uses internally to route the host's frames to the right DO). The host treats the `device_welcome` payload as opaque — it does not parse the routing id. On failure, the relay closes the WS with a 4xxx code.

The browser side performs an analogous `browser_hello` carrying an OAuth access token. The relay validates and assigns the same per-user DO.

**`device_hello` and `browser_hello` are NOT frames of type 0x01/0x02.** They are relay-defined control messages at the WebSocket protocol layer, sent in cleartext (the underlying TLS is still provided by Cloudflare). The Noise session begins only after both sides have authenticated and the host receives the browser-attached notification (frame type 0x11). Sub-spec C owns the exact wire format of these control messages.

### Contract 7.7 — Per-user routing

The relay's per-user Durable Object is the only place that knows which WebSocket handles belong to a given user. It uses **frame type 0x10 (control)** to pass routing metadata, NOT to wrap the encrypted app payload. The host and browser treat type 0x02 frames as opaque Noise transport messages with no relay-visible header. The DO's routing table maps `user_id → { device_ws, [browser_ws, ...] }` and is the single source of truth for fan-out.

The DO emits type `0x10` to the host when a browser connects, to coordinate the Noise handshake. Type `0x11` (browser-attached) tells the host "initiate Noise with browser X"; type `0x12` (browser-detached) tells the host "browser X is gone, tear down its Noise session". Type `0x20` (device-revoked) is a kill switch.

## 8. Security and privacy

- **E2E mandatory.** No sub-spec may ship a build flag that disables Noise. Tests must assert post-handshake that relay-seen bytes are indistinguishable from random.
- **Replay protection.** Noise transport counters + a relay-level nonce check. Replays dropped at the relay.
- **Token storage.** `device_token` is encrypted with a key derived from a platform keychain entry. Reading the token requires unlocking the keychain (Touch ID on macOS, libsecret unlock on Linux).
- **OAuth provider scope.** We request only the minimum scopes: `read:user` for GitHub, `openid email profile` for Google. We never request write scopes.
- **Audit log (relay side).** Every `device_hello` / `browser_hello`, every 5xx, every rate-limit hit, every device revoke. Audit row: `{ ts, user_id, device_id, event, source_ip_hash }`. Audit is metadata only — never payload.
- **Audit log (host side).** Every proxied request that hits `WebSurface` is logged through the existing `AuditTrail` (AccessManager). This is the user-visible "what is my Mac doing" log.
- **Kill switch.** `/api/devices/revoke` in the OAuth broker (REST) revokes a `device_token` immediately. The relay's DO drops the WS handle. The host's next reconnect fails auth and the binary stops trying after 3 attempts (forces explicit re-`oxios tunnel login`).
- **Rate limits.** Per-user: 100 req/min, 1000 req/hour. Per-IP: 600 req/min (matches the existing `security.rate_limit_per_minute` default). 429 with Retry-After on breach.
- **What the relay does NOT have.** No LLM keys, no `~/.oxios/` access, no agent tool execution, no shell, no file system on the host. The relay is a thin router.

## 9. Decomposition & build order

This doc is the level-0 architecture. The five sub-specs are independently buildable. **Each sub-spec goes through the normal `spec → plan → implementation` cycle on its own.**

| Order | Sub-spec | Why this order | Approx. size |
|---|---|---|---|
| 1 | **A — OAuth broker** | Standalone Worker + D1. Testable with curl. No Mac binary coupling. Unblocks all later sub-specs (they all need an auth surface). | S |
| 2 | **B — Host tunnel (skeleton)** | Mac binary adds the tunnel module, keypair generation + keychain wrapping, keychain-unlock prompt (Contract 7.2), CLI surface (`--tunnel` flag, first-run wizard), outbound WebSocket client, reconnect with backoff, and an adapter that feeds the existing axum `Router`. Talks to A. SPA not yet integrated — host and browser both stubbed by A's "echo" mode. **L** — large because it touches CLI, keychain APIs, the WS client, and the adapter into WebSurface; not just a feature flag. | L |
| 3 | **C — Relay server** | Cloudflare Worker + Durable Object. Forwards opaque bytes. Tests with B + a fake browser. E2E wraps at sub-spec E. | M |
| 4 | **D — SPA relay integration** | Existing React SPA adds a relay client + device picker UI. Can run against B+C using a temporary in-browser X25519 key (E sub-spec adds real E2E). | M |
| 5 | **E — E2E crypto layer** | Wires Noise_XX into B and D. Final swap from "echo" to encrypted transport. | S |

After A–E: integration test, observability (logs/metrics/traces), kill-switch UX, docs (`docs/USER-GUIDE.md` and a new `docs/remote-access-guide.md`), and `oxios.com` domain registration (legal/ops, separate workstream).

Each sub-spec writes its own `docs/designs/2026-MM-DD-managed-relay-<letter>-<topic>-design.md` and is reviewed independently.

## 10. What this doc is NOT

- **Not a spec for any single subsystem.** A sub-spec owns the details of its subsystem. This doc only describes the seams.
- **Not a UX spec.** Sub-spec D owns the device picker, error states, offline indicator.
- **Not a deployment / infra runbook.** Sub-spec C owns `wrangler.toml`, secrets, region selection, multi-region failover.
- **Not a legal / privacy policy.** The relay's privacy posture is described here; the actual `oxios.com/privacy` page and OAuth consent strings are downstream.

## 11. Relationship to prior work

- **Supersedes** [`2026-07-29-remote-access-architecture-design.md.superseded`](2026-07-29-remote-access-architecture-design.md.superseded). That doc's §2.6 (Tailscale environment analysis) and §3 (gap analysis — CORS lockdown, missing zero-click token path, no TLS) remain valid and inform the priorities of sub-specs A, B, D. Tailscale Serve is preserved as a Tier-3 fallback path for users who prefer to operate their own VPN; the new design does not depend on it.
- **Unrelated to** `docs/designs/webui-tiers/` (a2a topology, live ops dashboard, memory map, settings UI). Those are local-UI richness work targeting `127.0.0.1:4200`. The managed relay system does not change what the UI shows; it changes where it is served from. They can ship in parallel.
- **Compatible with** `docker-compose/production/`. The tunnel module on the host is independent of how the daemon was launched. A Dockerized Oxios can opt into `--tunnel` the same way.
- **Builds on** the existing `WebSurface` / axum server. The tunnel module reuses the same `Router` (Contract 7.5). The existing local-Bearer auth remains unchanged for loopback callers.

## 12. Open questions

1. **Where to host the SPA during the transition?** Sub-spec D needs to decide: deploy `web/dist/` to Cloudflare Pages early (incremental) or only after relay is stable (atomic). *Recommendation: incremental — Pages is cheap, rollback is trivial.*
2. **Do we need region pinning?** Initial rollout is single-region. Add a second region (e.g., EU) when latency from a user's Mac to `relay.oxios.com` exceeds 200ms p50. *Defer to a separate sub-spec when measured.*
3. **What is the OAuth refresh policy?** Access tokens: 1 hour. Refresh tokens: 30 days, rotated on use. `device_token`: 90 days, rotated on use. *Final values picked in sub-spec A.*
4. **Browser session persistence.** If a user closes the iPhone Safari tab and reopens, do they reconnect automatically? Yes, via the existing `auth.ts` sessionStorage. *Owned by sub-spec D.*
5. **Headless daemon usage.** If the host is on a server with no GUI, can the user still run `oxios serve --tunnel`? Yes — the verification URL is printed to stdout, the user opens it on any browser. *Sub-spec B owns the CLI surface.*
6. **Cloudflare Workers plan and cost ceiling.** Cloudflare Workers Free plan: 100k req/day, 10ms CPU per invocation. Workers Paid: $5/month for 10M requests + 30s CPU. Durable Objects: per-GB-second, per-request. We expect < 10k active users for the first year, so Workers Paid is the floor. Sub-spec C must define a monthly cost ceiling (suggest: alert at $200, hard cap at $1000/month until we have revenue). *Final policy in sub-spec C.*
7. **Cloudflare WebSocket constraints.** Cloudflare Workers WebSocket API has a 100s default idle timeout and per-isolate connection limits. Sub-spec C must verify the per-user Durable Object can hold the host WS plus N browser WSs simultaneously and decide on heartbeat (currently 30s) to avoid idle disconnects.

## 13. Acceptance criteria (for the whole program, not any sub-spec)

- A user with a fresh Mac, a `brew install oxios`, and an iPhone reaches the daemon from the phone in under 60 seconds total.
- No file in `~/.oxios/` ever transits the relay in plaintext (verified by an E2E test that asserts the relay-seen bytes are indistinguishable from random across 10k requests).
- `oxios serve` (no `--tunnel`) still works exactly as before. The tunnel is additive.
- A revoked `device_token` is rejected within 5 seconds of revocation by every relay region.
- **Cost ceiling.** Cloudflare spend attributable to the relay system (sub-spec C) stays under the budget set in Open Question #6. Alert at 80%, hard cap at 100%, no new regions deployed beyond cap until policy is revised.
- **Privacy ceiling.** A subpoena / legal-request disclosure from Oxios (the operator) reveals only metadata as listed in §3 (user_id, device_id, timestamps, source IP hash). No payload, no content, no LLM keys. Verified by a self-audit run quarterly.
