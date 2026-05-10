# Loop 14 Plan — REVISED

## Problem
Moving Kernel to oxios-kernel creates circular dependency (kernel → gateway → kernel).
Kernel stays in binary crate.

## Solution: KernelHandle in oxios-kernel

```
oxios-kernel/
  └── src/kernel_handle.rs  ← NEW: facade struct
        holds Arc<StateStore>, Arc<GitLayer>, Arc<AuditTrail>, ...
        exposes only System Call methods (pub fn)
        
oxios-web/
  └── server.rs
        AppState holds Arc<KernelHandle>  (not 25 individual fields)

binary (src/)
  └── kernel.rs  (Kernel + KernelBuilder stays here)
        build() creates KernelHandle from subsystems
        passes Arc<KernelHandle> to WebServer
```

## Tasks (REVISED)

### T1: Create KernelHandle in oxios-kernel
- Create `crates/oxios-kernel/src/kernel_handle.rs`
- Struct holding all subsystem Arcs (same as Kernel fields)
- All System Call methods from Kernel impl
- Export from lib.rs

### T2: Kernel::handle() → KernelHandle
- Add method to Kernel that creates KernelHandle from its fields
- Binary calls `kernel.handle()` and passes to WebServer

### T3: Refactor AppState to Arc<KernelHandle>
- Replace 24 fields with Arc<KernelHandle>
- Keep web-specific fields only

### T4: Migrate routes to state.kernel.xxx()
- Replace 85 direct subsystem accesses with kernel.xxx() calls

### T5: Update main.rs wiring
- Pass kernel.handle() to WebServer

## Batches

```
Batch 1 (sequential): [T1]           — KernelHandle
Batch 2 (parallel):   [T2, T3]       — handle() + AppState
Batch 3 (sequential): [T4, T5]       — Routes + main.rs wiring
Batch 4 (sequential): [cleanup]      — Remove dead code
```
