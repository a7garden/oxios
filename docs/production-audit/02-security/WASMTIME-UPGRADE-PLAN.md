# Wasmtime Upgrade Assessment — 2026-05-31

**Current:** wasmtime 22.0.1 (with `cranelift` feature)  
**Target:** wasmtime 45.0.0 (latest stable)  
**Recommended minimum:** wasmtime ≥ 43.1.0 (all known RUSTSEC advisories patched)

---

## Verdict: ❌ Do NOT upgrade in this audit cycle

The jump from 22 → 45 spans **23 major versions** with extensive breaking API changes.  
This requires a dedicated migration sprint, not a patch-level change.

---

## Breaking Changes (22 → 45)

### High-Impact (affects `wasm_sandbox.rs`)

| Version | Change | Impact on Oxios |
|---------|--------|-----------------|
| 23–25 | `Linker::define_wasi()` removed | Our code calls `linker.define_wasi(wasi_ctx)`. Must migrate to `wasmtime-wasi::add_to_linker()` |
| 25–27 | `WasiCtx` builder API changed | `WasiCtxBuilder::new().build()` may need migration to `WasiCtxBuilder::new().inherit_stdio().build()` |
| 28–30 | `Store<T>` creation changed | `Store::new()` API signature may differ |
| 31+ | WASI preview 1 → preview 2 migration | `wasmtime-wasi` crate reorganized. Must import `wasmtime-wasi` v0.31+ separately |
| 33–36 | `Linker` API changes | `define_wasi` → `wasmtime_wasi::add_to_linker_sync()` or `add_to_linker_async()` |
| 36+ | Fuel API may have changed | `store.set_fuel()` and `store.fuel_remaining()` may have different signatures |
| 38–42 | `Memory::write` / `Memory::read` API changes | Parameter types may have changed |
| 42+ | `get_typed_func` API changes | Generic parameter syntax may differ |

### Medium-Impact

| Version | Change | Impact |
|---------|--------|--------|
| 24+ | Minimum Rust version bumped | May require MSRV bump in CI |
| 30+ | `Config` API additions | `consume_fuel` still supported but new defaults may differ |
| 35+ | Cranelift settings changes | May need config adjustments |
| 40+ | `Module::from_binary` unchanged | ✅ Should work as-is |

---

## Affected Code

Single file: `crates/oxios-kernel/src/wasm_sandbox.rs` (~100 lines of `#[cfg(feature = "wasm-sandbox")]` code)

Key API touchpoints:
1. `wasmtime::Config::new()` — ✅ likely compatible
2. `wasmtime::Engine::new()` — ✅ likely compatible
3. `wasmtime::Linker::new()` — ⚠️ API changed
4. `linker.define_wasi(ctx)` — ❌ **removed**, must use `wasmtime_wasi::add_to_linker*`
5. `wasmtime::Store::new()` — ⚠️ may need type adjustments
6. `store.set_fuel()` — ⚠️ check signature
7. `store.fuel_remaining()` — ⚠️ check signature
8. `linker.instantiate()` — ✅ async version available
9. `instance.get_typed_func::<(i32,i32),(i32,i32)>()` — ⚠️ generic params may differ
10. `memory.write()` / `memory.read()` — ⚠️ check lifetime/ownership

---

## Migration Plan

### Step 1: Preparation (0.5 day)
- [ ] Create feature branch `wasmtime-upgrade`
- [ ] Add CI matrix: test with `--features wasm-sandbox`
- [ ] Document current behavior with integration tests

### Step 2: Dependency Update (0.5 day)
- [ ] Update `crates/oxios-kernel/Cargo.toml`: `wasmtime = "43"` / `wasmtime-wasi = "0.43"`
- [ ] Run `cargo check -p oxios-kernel --features wasm-sandbox`
- [ ] Fix compile errors iteratively

### Step 3: API Migration (1 day)
- [ ] Migrate `Linker` + WASI context setup
  ```rust
  // OLD (22)
  let mut linker = wasmtime::Linker::new(&engine);
  let wasi_ctx = wasmtime_wasi::WasiCtxBuilder::new().build();
  linker.define_wasi(wasi_ctx)?;
  
  // NEW (43+) — approximate, verify against actual 43.x docs
  let mut linker = wasmtime::Linker::new(&engine);
  wasmtime_wasi::add_to_linker_sync(&mut linker, |ctx| ctx)?;
  ```
- [ ] Migrate `Store` creation
- [ ] Migrate `get_typed_func` if signature changed
- [ ] Verify fuel API

### Step 4: Testing (0.5 day)
- [ ] `cargo test -p oxios-kernel --features wasm-sandbox`
- [ ] Manual test: load and execute a WASM module
- [ ] Verify memory limits are enforced
- [ ] Verify fuel exhaustion works correctly

### Step 5: Verification (0.5 day)
- [ ] `cargo audit` — confirm all wasmtime advisories are resolved
- [ ] `cargo test --workspace` — full suite passes
- [ ] Update `SECURITY-POSTURE.md`

**Estimated effort: 2–3 days**

---

## Immediate Mitigation (Applied Now)

While the upgrade is pending, the following mitigations are already in place:

1. **Feature gate:** `wasm-sandbox` is NOT in the default feature set. Users must explicitly opt in.
2. **Documentation:** The feature is documented as insecure until wasmtime is upgraded.
3. **Default features in `Cargo.toml`:**
   ```toml
   [features]
   default = ["browser", "sqlite-memory"]
   # wasm-sandbox is explicitly NOT in default
   ```

### Recommended config-level mitigation

Add to `share/default-config.toml`:

```toml
# WASM sandbox is disabled by default due to known wasmtime vulnerabilities
# (RUSTSEC-2026-0096, RUSTSEC-2026-0095). Do not enable in production until
# wasmtime is upgraded to >= 43.0.1.
# wasm_sandbox_enabled = false
```

---

## Risks of Delaying

| Risk | Likelihood | Impact |
|------|-----------|--------|
| New wasmtime vuln discovered in 22.x | High | More advisories |
| User enables wasm-sandbox unknowingly | Low | Sandbox escape on ARM64 |
| Supply chain concern from audit scanners | Medium | CI/CD noise |

The critical vulnerabilities (sandbox escape) only affect **ARM64 hosts** with `wasm-sandbox` enabled. Oxios primarily targets macOS (ARM64 Apple Silicon) and Linux (x86-64), so the ARM64 risk is real but contained to an opt-in feature.
