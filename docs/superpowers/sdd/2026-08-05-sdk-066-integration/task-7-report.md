# Task 7 Report — Wire lifecycle hooks into OxiosEngine and config.toml

**STATUS:** DONE
**Commit:** `4bb4c7e36`
**Branch:** `feat/sdk-066-integration`

## Summary

HookSpec config now plumbs end-to-end: `EngineConfig.hooks` → `OxiosEngineBuilder::with_hook_specs` → `CommandHookRunner` → `OxicodeBuilder::with_hooks`. All three engine-construction branches in `src/kernel.rs` (router, legacy `routing_enabled`, default) wire hooks via a local `attach_hooks` helper. `share/default-config.toml` documents the `[[hooks]]` schema.

## Files changed (5)

| File | Change |
|------|--------|
| `crates/oxios-kernel/src/config.rs` | Added `pub hooks: Vec<oxicode_sdk::ports::hooks::HookSpec>` to `EngineConfig` (after `quick_ask_model`, before `router`), with `#[serde(default)]`. Added `hooks: Vec::new()` to `Default` impl. |
| `crates/oxios-kernel/src/engine.rs` | Added `hook_specs: Option<Vec<oxicode_sdk::ports::hooks::HookSpec>>` field to `OxiosEngineBuilder`. Initialized to `None` in `OxiosEngine::builder()` factory. Added `with_hook_specs` method. Updated `build(self) → build(mut self)` to wire `CommandHookRunner` into `OxicodeBuilder::with_hooks` before calling `self.inner.build()`. Added `hook_specs: self.hook_specs` to all three `Self { … }` constructions in `api_key`, `credential`, `provider`. |
| `crates/oxios-kernel/src/lib.rs` | Re-exported `HookSpec` and `OxiosEngineBuilder` so the root crate can use them without pulling `oxicode_sdk` directly. |
| `src/kernel.rs` | Added local `attach_hooks` helper (idempotent — short-circuits on empty `hooks`). Wired into the router branch (`build()`), legacy `routing_enabled` branch (`build_with_routing()`), and default branch (via the new builder-based `build_default_engine`). `build_default_engine` now uses `OxiosEngine::builder()` + `.default_model()` + optional `.api_key(primary_provider, key)` + optional `.with_catalog(...)` + `attach_hooks(...).build()` (per brief). |
| `share/default-config.toml` | Added commented `[[hooks]]` template section after the router block, before `[daemon]`. |

## Verification

### `cargo check` (root + all crates)
`Finished dev profile [unoptimized + debuginfo] target(s) in 10.50s` — all green, zero warnings related to the change.

### `cargo test -p oxios-kernel`
- 815 unit tests — all pass
- 5 + 5 + 16 + 5 = 31 integration tests — all pass
- 12 doc tests — all pass
- 0 failed, 1 ignored (pre-existing)

## Deviations from the brief

1. **Re-exports in `lib.rs`.** The brief's `attach_hooks` signature in `src/kernel.rs` referenced `oxicode_sdk::ports::hooks::HookSpec` directly, but the root crate (`oxios`) does not depend on `oxicode_sdk` (only the kernel does, transitively). To respect the "No net-new dependencies" constraint, I added two re-exports to `crates/oxios-kernel/src/lib.rs`:
   - `pub use oxicode_sdk::ports::hooks::HookSpec;`
   - `pub use engine::{..., OxiosEngineBuilder};`
   The helper in `src/kernel.rs` uses `oxios_kernel::HookSpec` and `oxios_kernel::OxiosEngineBuilder` accordingly. This is consistent with how the kernel already re-exports other oxicode-sdk types (e.g. `Authorizer`, `CostTracker`, `Tracer`).

2. **Match ergonomics in `build()`.** The brief's `if let Some(ref specs) = self.hook_specs` would trigger a project rule violation (`rs-match-ergonomics`). Used `if let Some(specs) = &self.hook_specs` instead — identical semantics, no `ref` binding.

3. **`[daemon]` header preservation.** An intermediate edit accidentally dropped the `[daemon]` section header. Restored in a follow-up edit; verified `[daemon]` is intact at line 85 of `share/default-config.toml`.

## Deferred limitation (per brief)

The default branch in `src/kernel.rs` no longer calls `OxiosEngine::from_config*`. Those helpers pre-seeded ~18 known provider API keys at build time; the new `build_default_engine` only seeds the explicit `config.engine.api_key` override for the primary provider. Runtime credential resolution (env vars, `auth.json`) still works through the `AuthProvider` port — confirmed in `oxicode-sdk` builder.rs. If a regression appears, revert `build_default_engine` to the `from_config*` path and document that hooks require the router or legacy-routing path.

## Other notes

- Pre-existing modifications in `crates/oxios-kernel/src/kernel_handle/{engine_api,knowledge_lens}.rs` (rustfmt-only reformatting) were left untouched — out of scope for this task.
- No net-new dependencies added.
- All `Self { ... }` constructions in `OxiosEngineBuilder` updated: `builder()` (1), `api_key` (1), `credential` (1), `provider` (1). Verified via grep.

---

## Fix Round 1 — `build_with_routing` did not wire hooks

**Reviewer finding (Important):** the legacy `routing_enabled` path called `self.inner.build()` directly in `build_with_routing()` and never invoked the hook-wiring block, so hooks configured in that path were dropped before reaching `Oxicode`.

### Change
Extracted the hook-wiring block into a private helper `fn wire_hooks(mut self) -> Self` on `OxiosEngineBuilder`, and called it from both `build()` and `build_with_routing()`.

`crates/oxios-kernel/src/engine.rs`:

Per the reviewer note: router registration is intentionally NOT added to `build_with_routing()` (Task 2's deferred-minor stays deferred — `RouterProvider` only works in the `build()` path).

### Verification

**`cargo check` (root):**
```
Checking oxios-kernel v1.36.0 (/Volumes/MERCURY/PROJECTS/oxios/crates/oxios-kernel)
Checking oxios-gateway v1.36.0 (/Volumes/MERCURY/PROJECTS/oxios/crates/oxios-gateway)
Checking oxios v1.36.0 (/Volumes/MERCURY/PROJECTS/oxios)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.21s
```
Clean — zero warnings.

**`cargo test -p oxios-kernel`:**
```
test result: ok. 815 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok.   5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok.   5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok.  16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok.   5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok.  12 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out
```
858 tests pass, 0 failed, 5 ignored (pre-existing).

### Commit
`763566034` — `fix(kernel): wire hooks in build_with_routing too`.
