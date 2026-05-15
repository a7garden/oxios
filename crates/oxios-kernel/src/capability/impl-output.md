# Capability System Implementation — Output

## Files Created

### 1. `crates/oxios-kernel/src/capability/mod.rs`
Module root that declares submodules (`types`, `template`, `resolve`) and re-exports core types at the module level.

### 2. `crates/oxios-kernel/src/capability/types.rs`
Core capability types:
- **`CapabilityId`** — UUID newtype, random at creation (unforgeable)
- **`Rights`** — u8 bitmask with NONE(0x00), READ(0x01), WRITE(0x02), EXECUTE(0x04), DELEGATE(0x08), ALL(0x0F). Implements `BitOr`, `BitAnd`, `Display`
- **`ResourceRef`** — Tagged enum: KernelDomain, Program, Space, Agent, Exec, Browser, A2a, Mcp
- **`Issuer`** — Kernel or Agent(id) tagged enum
- **`Capability`** — Binds Rights + ResourceRef + Issuer. `kernel()` and `delegated()` constructors
- **`CSpace`** — HashMap<CapabilityId, Capability> wrapper with `can(resource, rights)` check

Tests: 5 unit tests covering rights bitops, display, CSpace checks, delegation.

### 3. `crates/oxios-kernel/src/capability/template.rs`
`CapabilityTemplate` builder with presets:
- `worker()` = Exec(shell) + Browser
- `standard()` = worker + Memory(READ)
- `operator()` = standard + Space + Agent + A2a + Persona + Program + MCP + Memory(WRITE)
- `supervisor()` = operator + Security + Budget + Resource + Cron
- `with_programs(names)` = worker + named programs
- `.with(resource, rights)` for chaining
- `.build()` / `.build_for(agent_id)` produce CSpace

Tests: 7 unit tests covering all templates and builder chaining.

### 4. `crates/oxios-kernel/src/capability/resolve.rs`
`resolve_cspace(cspace_hint, persona_role, default_template, agent_id)`:
1. If `cspace_hint` is present → use it
2. Else if `persona_role` matches a known role → use it
3. Else → fall back to `default_template` or "worker"

JSON cspace_hint is detected but not yet parsed (logged as warning, falls back to worker).

Tests: 7 unit tests covering priority chain, empty hints, unknown names, JSON fallback.

## Files Modified

### `crates/oxios-kernel/src/lib.rs`
- Added `pub mod capability;` (alphabetically ordered)
- Added re-exports: `Capability, CapabilityId, CSpace, Issuer, ResourceRef, Rights, CapabilityTemplate, resolve_cspace`

### `crates/oxios-ouroboros/src/seed.rs`
- Added `cspace_hint: Option<String>` field with `#[serde(default, skip_serializing_if)]`
- Updated `Seed::new()` to initialize `cspace_hint: None`
- Updated `Seed::evolved_from()` to propagate `cspace_hint`

### `crates/oxios-ouroboros/src/ouroboros_engine.rs`
- Added `cspace_hint: None` to the two Seed struct literals (generate and evolve)

## Compilation Status

- `cargo check -p oxios-kernel` — ✅ passes (errors only from upstream `oxi-agent`)
- `cargo check -p oxios-ouroboros` — ✅ passes clean
- `cargo test -p oxios-ouroboros` — ✅ all 10 unit tests + 2 doc tests pass
- `cargo test -p oxios-kernel` — ❌ blocked by pre-existing `oxi-agent` compile errors (not related to this change)
