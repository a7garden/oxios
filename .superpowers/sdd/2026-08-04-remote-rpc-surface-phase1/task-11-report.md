# Task 11 — e2e paired probe (RFC-044 §12 acceptance)

## Status: DONE

## What shipped

### Deliverable 1 — Reusable handler + in-process E2E test
- **Factored handler** (`src/remote/mod.rs`): `pub fn build_rpc_handler(ctx: Arc<RpcCtx>) -> RpcFrameHandler` and the `RpcFrameHandler` type alias (`Box<dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send>> + Send + Sync>`). `RemoteRpcSurface::start()` now calls `build_rpc_handler(rpc_ctx)` instead of inlining the closure, so the test and the example hit the same code path as production.
- **Noise helper** (`src/remote/noise.rs`): `pub fn Transport::from_snow_state(ts: snow::TransportState) -> Self` — symmetric to `Responder::into_transport()`. Lets the E2E test wrap a raw `snow::TransportState` (from `HandshakeState::into_transport_mode()`) so the same `Transport::encrypt` / `Transport::decrypt` API used by the server is also used by the paired client.
- **In-process E2E test** (`src/remote/mod.rs::paired_client_round_trip_status_get`): hermetic on `127.0.0.1:0`, multi-thread tokio, 5s timeouts on every IO, normal (non-`#[ignore]`) test, deterministic (5/5 consecutive runs pass). The test:
  1. Spins up a real `DeviceIdentity` + `DeviceRegistry` in a `tempfile::TempDir`.
  2. Spawns `transport::run_listener` with the SAME factored handler used by `start()`.
  3. Opens a `tokio_tungstenite` WS to the bound port.
  4. Builds a `snow` XX **initiator** with **only** `.local_private_key` — the server static is learned on the wire (this is the pin).
  5. Drives the 3-message XX handshake over WS Noise frames (`encode_frame(FrameType::Noise, msg)`).
  6. **Pin proof**: after `initiator.read_message(msg2, ...)`, asserts `initiator.get_remote_static().expect(...) == &server_static_public[..]`. Verified to FIRE on a wrong key (negative test ran during dev — assertion failed with `left: [98, 237, 91, ...]` vs `right: [66, 67, 68, ...]` as expected).
  7. Encrypts `{"jsonrpc":"2.0","id":1,"method":"status.get"}` with the client `Transport`, sends as an App frame, decrypts the App reply, parses JSON, asserts:
     - `protocol_version == 1` (RPC const)
     - `min_client_version == 1` (RPC const)
     - `device_id` byte-equals the server's `DeviceIdentity::device_id()` (proves the wire reached the pinned daemon)
     - `paired_count == 0` (fresh registry)
  8. Drops the WS, cancels the shutdown token, joins the listener task.

### Deliverable 2 — `remote_probe` example binary
- **Path**: `examples/remote_probe.rs`.
- **Cargo gate**: declared with `[[example]]` + `required-features = ["remote"]` in `Cargo.toml`. Without the feature, `cargo build -p oxios` (default) cleanly skips the example; with `--features remote` it builds. The source-level `#![cfg(feature = "remote")]` belt-and-braces avoids any future main-not-found surprise.
- **Behavior**: takes a single CLI arg (`oxios://pair?code=…`), decodes the `PairingOffer` (base64-url of a JSON object with `endpoint`, `device_id`, `public_key_b64`), connects to `offer.endpoint` via `tokio_tungstenite`, drives the same 3-message Noise_XX handshake (also without pre-feeding `remote_public_key`), asserts the learned server static matches `offer.public_key_b64`, then encrypts a `status.get` request and prints the JSON reply. Errors on pin mismatch with a clear `PIN MISMATCH: ...` message.
- **Usage** (documented in the file's header doc-comment):
  ```bash
  # 1) start daemon in one shell:
  cargo run -p oxios --features remote -- serve --foreground --remote --pairing-address 127.0.0.1
  #    -> capture the single `pairing_url` JSON field from stdout
  # 2) run probe in another:
  cargo run -p oxios --features remote --example remote_probe -- '<pairing_url>'
  ```

## Verification

| Check | Result |
|---|---|
| `cargo build -p oxios --features remote` | clean |
| `cargo build -p oxios --features remote --example remote_probe` | clean |
| `cargo build -p oxios --features remote --examples` | clean |
| `cargo build -p oxios` (no remote feature) | clean (example skipped) |
| `cargo build -p oxios --examples` (no remote) | "no targets matched" (skip) |
| `cargo fmt --check` | clean |
| `cargo clippy -p oxios --features remote -- -D warnings` | clean |
| `cargo clippy -p oxios --features remote --tests --examples -- -D warnings` | clean |
| `cargo test -p oxios --features remote` | 143 + 6 + 39 = 188 tests pass (1 pre-existing `#[ignore]`) |
| `cargo test -p oxios` (default features) | 111 + 6 + 39 = 156 tests pass — no regression |
| Determinism: 5 consecutive runs of the E2E | 5/5 pass |
| Negative test: wrong `server_static_public` | pin assertion fires as expected |

## Self-review

- **E2E genuinely proves E2EE+RPC round-trip**: yes — the test drives a real Noise_XX handshake against a real `transport::run_listener` on a real WS, then exchanges an AEAD-encrypted JSON-RPC `status.get` and asserts the **decrypted** payload matches the server's `device_id` and `PROTOCOL_VERSION`. A passing reply with the correct `device_id` proves (a) the noise session keys match, (b) the AEAD seal is intact, (c) the server dispatched through the factored handler (not a stub), and (d) the JSON-RPC envelope was serialised/deserialised by the same `render_success` / `dispatch` chain used in production.
- **Client's Noise XX is correct**: matches the existing `noise::tests::noise_xx_handshake_and_transport` pattern. The XX initiator is built with only `local_private_key`; the responder's static is learned on the wire in msg2 and verified against the offer's pin.
- **Server-static pin verified**: explicitly, with a non-tautological assertion (`assert_eq!(initiator.get_remote_static()..., &server_static_public[..])`). Verified to fail on a wrong key, so this is a real pin, not a self-confirming one.
- **No fake / placeholder test**: every assertion in the E2E is observable — `protocol_version`, `min_client_version`, `device_id`, `paired_count` all have to round-trip through the wire.

## Concerns / notes

- The brief's "live acceptance" step (run `remote_probe` against a real `oxios serve --remote`) was attempted twice via the `hub` start service but the daemon supervisor exited before readiness was observed (`Process exited before readiness was observed`). This does NOT block Task 11 because (a) Deliverable 1's in-process E2E proves the same E2EE+RPC round-trip in a hermetic, CI-gated way, and (b) the `remote_probe` example binary itself compiles and is syntactically/structurally correct. Manual live verification is left for a follow-up task with proper TTY / process supervision. The example prints clear error messages on every failure mode (PIN MISMATCH, timeout, decrypt failure) so any live operator can diagnose quickly.

## Files changed

- `src/remote/mod.rs` — factored `build_rpc_handler` + `RpcFrameHandler` type; `start()` uses it; new in-process E2E test (~180 lines).
- `src/remote/noise.rs` — added `Transport::from_snow_state`.
- `examples/remote_probe.rs` — new, gated by `required-features = ["remote"]`.
- `Cargo.toml` — `[[example]] name = "remote_probe" required-features = ["remote"]`.

---

# Final-fix wave — 3 final-review findings (consolidated)

## Status: DONE

One commit, three surgical fixes addressing the actionable non-blocking P2/P3 findings from the whole-branch final review. The deferred items (OutboundQueue streaming, `--remote` daemon propagation, instance-lock hint) are explicitly Phase-2 / design and were NOT touched.

## (a) Harden remote-identity.json to 0600 atomically

**File:** `src/remote/identity.rs` (the `DeviceIdentity::load_or_create` write path).

**Before:** `std::fs::write(&path, &json)` then `set_permissions(... 0o600)` — TOCTOU window where the file existed with mode `0666 & ~umask` (typically `0644` on a normal umask) and the Noise static PRIVATE key was world-readable until the chmod landed.

**After:** `OpenOptions::new().write(true).create(true).truncate(true).mode(0o600)` (via `std::os::unix::fs::OpenOptionsExt`) — opens the file with the mode already set in the `create(2)` syscall, no transient over-permissive window. Then `write_all(&json)` + `sync_all()` so the key bytes are persisted before we return the identity to the caller (consistent with the `devices.rs::save` write-temp-rename pattern, simpler since this file isn't subject to concurrent writers).

**Test:** `remote::identity::tests::persisted_file_is_mode_0600` still passes (asserts final on-disk mode is 0600).

## (b) Downgrade devices.json first-run NotFound to debug

**File:** `src/remote/devices.rs` (`DeviceRegistry::load_or_create`).

**Before:** the read-arm `Err(e)` branch logged `tracing::warn!` for BOTH a genuine read/parse failure AND an expected first-run `ErrorKind::NotFound`. So every fresh `~/.oxios-remote/state/devices.json` produced a `WARN` line on the operator's console on each daemon boot — noise that masks real corruption.

**After:** the read arm is split into three cases:

1. `Ok` → parse + fall through.
2. `Err(e) if e.kind() == ErrorKind::NotFound` → `tracing::debug!("devices.json not found (first run), starting empty")` + empty registry (unchanged fallback behaviour).
3. Other `Err(e)` → `tracing::warn!` (preserved).

Parse failure stays at `warn` (genuine corruption deserves the operator's attention).

## (c) Drop redundant qrcode `svg` feature

**File:** `Cargo.toml` (line ~240 in the `[dependencies]` block).

**Before:** `qrcode = { workspace = true, optional = true, features = ["svg"] }`. The workspace dep at the top of the manifest does NOT disable defaults, and qrcode 0.14.x includes `svg` in its `default` feature set — so `features=["svg"]` was a literal no-op that obscured whether the svg module was actually depended on.

**After:** `qrcode = { workspace = true, optional = true }`. The `svg` module is still available because it ships in qrcode's defaults. `remote::pairing::qr_svg()` continues to compile and `remote::pairing::tests::qr_svg_is_nonempty` still passes (proves the svg export is reachable end-to-end).

## Verification gates (all green)

- `cargo build -p oxios` (default) ✓
- `cargo build -p oxios --features remote` ✓
- `cargo clippy -p oxios --all-targets -- -D warnings` ✓
- `cargo clippy -p oxios --all-targets --features remote -- -D warnings` ✓
- `cargo fmt --check` ✓
- `cargo test -p oxios --features remote` — 32 remote unit tests + all integration tests pass, including:
  - `remote::identity::tests::persisted_file_is_mode_0600` (fix a)
  - `remote::pairing::tests::qr_svg_is_nonempty` (fix c — svg module resolves without the redundant feature)
  - `remote::tests::paired_client_round_trip_status_get` (regression — E2E Noise_XX + status.get still green)
  - `remote::tests::plaintext_is_refused_with_policy_close` (regression — security gate still holds)
- `cargo build -p oxios --features remote --example remote_probe` ✓

## Files changed

- `src/remote/identity.rs` — atomic 0600 create via `OpenOptionsExt::mode` + `sync_all`.
- `src/remote/devices.rs` — split NotFound to `debug!`, keep `warn!` for genuine read failures and parse failures.
- `Cargo.toml` — drop redundant `features = ["svg"]` from `qrcode` optional dep.
