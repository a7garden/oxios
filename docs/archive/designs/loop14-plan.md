# Loop 14 Implementation Plan

## Tasks

### T1: Move Kernel to oxios-kernel crate
- **What:** Move `src/kernel.rs` (Kernel struct + KernelBuilder + System Call methods) into `crates/oxios-kernel/src/kernel.rs`
- **Why:** oxios-web can't import Arc<Kernel> when Kernel is in the binary crate
- **Files:** 
  - Create `crates/oxios-kernel/src/kernel.rs` (move from src/kernel.rs)
  - Update `crates/oxios-kernel/src/lib.rs` (pub mod kernel, pub use)
  - Update `src/main.rs` (use oxios_kernel::Kernel)
  - Delete old `src/kernel.rs` (replaced)
- **Verify:** `cargo check --workspace`
- **dependsOn:** []
- **touchesFiles:** kernel.rs (move), lib.rs, main.rs

### T2: Refactor AppState to Arc<Kernel>
- **What:** Replace 24 individual AppState fields with Arc<Kernel>. Keep web-specific fields.
- **Files:**
  - `channels/oxios-web/src/server.rs` (AppState, WebServer::new)
- **Verify:** Will break routes — that's expected, fixed in T3
- **dependsOn:** [T1]
- **touchesFiles:** server.rs

### T3: Migrate all routes to kernel.xxx() calls
- **What:** Replace 85 `state.state_store.xxx()` / `state.supervisor.xxx()` with `state.kernel.xxx()`
- **Files:**
  - `channels/oxios-web/src/routes/*.rs` (all route files)
- **Verify:** `cargo check --workspace`
- **dependsOn:** [T2]
- **touchesFiles:** routes/*.rs

### T4: Change Kernel fields pub → pub(crate)
- **What:** All 22 Kernel fields change from `pub` to `pub(crate)`
- **Files:**
  - `crates/oxios-kernel/src/kernel.rs`
  - `src/main.rs` (if it accesses fields directly)
- **Verify:** `cargo check --workspace`
- **dependsOn:** [T3]
- **touchesFiles:** kernel.rs, main.rs

### T5: Update main.rs wiring
- **What:** Update main.rs to pass Arc<Kernel> to WebServer instead of individual subsystems
- **Files:**
  - `src/main.rs`
- **Verify:** `cargo check --workspace`
- **dependsOn:** [T2]
- **touchesFiles:** main.rs

## Execution Batches

```
Batch 1 (sequential): [T1]           — Move Kernel to oxios-kernel
Batch 2 (parallel):   [T2, T5]       — AppState refactoring + main.rs wiring
Batch 3 (sequential): [T3]           — Migrate routes to kernel.xxx()
Batch 4 (sequential): [T4]           — pub → pub(crate)
```

## Acceptance Criteria

- [ ] `cargo check --workspace` passes
- [ ] AppState has <10 fields (kernel + web-specific)
- [ ] All routes use `state.kernel.xxx()` pattern
- [ ] Kernel fields are pub(crate), only System Calls are pub
- [ ] No direct subsystem access from outside kernel.rs
