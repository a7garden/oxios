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
