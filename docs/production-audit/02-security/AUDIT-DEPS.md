# Dependency Vulnerability Audit — 2026-07-24

**Tool:** `cargo audit` (advisory database refreshed 2026-07-24)
**Scope:** 905 crate dependencies in `Cargo.lock`
**Result:** 0 unignored vulnerabilities; one documented RSA exception and two non-blocking warnings.

## Classification

| Finding | Status | Reason |
|---|---|---|
| `RUSTSEC-2023-0071` (`rsa` 0.9.10) | Accepted exception | Transitive via `oxi-ai`; no upstream fix is available. |
| Wasmtime 24.x advisories | Resolved | Lockfile now uses Wasmtime 36.0.12. |
| Unmaintained transitive crates | Tracked | No direct production dependency currently owns these versions. |
| Yanked `spin` 0.9.8 | Tracked | Transitive via `oxi-sdk`; no compatible direct replacement is available in this workspace. |

## Reachability

`wasm-sandbox` is optional and absent from default features. When enabled, the
patched Wasmtime 36.0.12 line is compiled and checked by the dedicated feature gate.
RSA remains transitive and is not directly invoked by Oxios application code.

## Commands

- `cargo audit` — pass with the documented RSA exception and non-blocking warnings.
- `cargo check -p oxios-kernel --features wasm-sandbox` — pass.

[docs/production-audit/02-security/SECURITY-POSTURE.md#5260]
SWAP.BLK 1:
# Oxios Security Posture — 2026-07-24

**Audit scope:** dependency vulnerabilities, command execution, optional Wasm sandbox, and local web API.
**Review date:** 2026-07-24

## Vulnerability Summary

| Finding | Status | Action |
|---|---|---|
| Wasmtime 36.0.12 | Resolved | Keep `wasm-sandbox` opt-in and rerun its feature gate after upgrades. |
| `rsa` RUSTSEC-2023-0071 | Accepted exception | No upstream fix; monitor `oxi-sdk`/`rsa`. |
| Unmaintained/yanked transitive crates | Tracked | Review when upstream dependency updates become available. |

## Security Controls

- Shell execution is disabled by default and gated by AccessManager.
- Structured execution uses an allowlist and metacharacter checks.
- Workspace and knowledge paths use canonicalization/sandbox checks.
- API defaults to loopback binding and restricted CORS.
- Wasm sandbox memory, fuel, module-size, and wall-clock limits are enforced.

## Operational Caveat

Binding the API to a non-loopback interface requires explicit authentication and
TLS termination. Authenticated URL skill import and query-string WebSocket fallback
remain review items because they can expose internal network details or credentials.

## Verification

- Rust formatting, clippy, workspace build, nextest, and doctests pass.
- Frontend typecheck, tests, Biome lint, and production build pass.
- `cargo audit` reports no unignored vulnerabilities.

[docs/production-audit/02-security/WASMTIME-UPGRADE-PLAN.md#C1A7]
SWAP.BLK 1:
# Wasmtime Security Maintenance — 2026-07-24

**Current:** `wasmtime`/`wasmtime-wasi` 36.0.12 with Cranelift.
**Scope:** Optional `wasm-sandbox` feature in `oxios-kernel`.
**Status:** Patched dependency line; no open Wasmtime advisories remain in the lockfile.

## Decision

Upgrade from the vulnerable 24.x line to the maintained 36.0.x line. The existing
preview1 integration remains compatible; the only source change required by the
newer `ResourceLimiter` trait is changing table sizes from `u32` to `usize`.

The feature remains opt-in and is not part of the default feature set. Opt-in users
should still run the Wasm-specific compile/test gate before enabling it in production.

## Verification

- `cargo check -p oxios-kernel --features wasm-sandbox`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo audit` (zero vulnerabilities; remaining accepted exception: transitive RSA)

## Maintenance

Keep `wasmtime` and `wasmtime-wasi` on the same 36.x patch line. Re-run the three
checks above whenever either dependency changes. A future move to 43+ will require
reviewing the preview1-to-p1 module rename.
