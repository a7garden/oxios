# oxi-sdk Re-export Audit

> Date: 2026-05-16  
> File: `crates/oxios-kernel/src/lib.rs` (bottom of file)

## Summary

The kernel's `lib.rs` re-exports 25 types from `oxi_sdk` with the intent of making them available to downstream consumers via `oxios_kernel::`. The audit finds that **zero** of these re-exports are used outside the kernel through the `oxios_kernel::` path. Internal kernel modules import directly from `oxi_sdk::`, `oxi_agent::`, or `oxi_ai::` instead.

---

## Full Re-export List

| # | Re-exported as | Original (`oxi_sdk::`) | Used internally via `crate::`? | Used by downstream via `oxios_kernel::`? | Used internally via `oxi_sdk::`/`oxi_ai::`/`oxi_agent::`? |
|---|----------------|------------------------|-------------------------------|------------------------------------------|----------------------------------------------------------|
| 1 | `MessageBus` | `MessageBus` | ❌ | ❌ | ✅ via `oxi_sdk::MessageBus` in `kernel_handle/a2a_api.rs` |
| 2 | `InterAgentMessage` | `InterAgentMessage` | ❌ | ❌ | ✅ via `oxi_sdk::InterAgentMessage` in `kernel_handle/a2a_api.rs` |
| 3 | `SdkAgentMetrics` | `AgentMetrics` | ❌ | ❌ | ❌ Not used anywhere |
| 4 | `SdkMetricsSnapshot` | `MetricsSnapshot` | ❌ | ❌ | ❌ Not used anywhere |
| 5 | `ProviderPool` | `ProviderPool` | ❌ | ❌ | ❌ Not used anywhere |
| 6 | `RateLimitPolicy` | `RateLimitPolicy` | ❌ | ❌ | ❌ Not used anywhere |
| 7 | `Provider` | `Provider` | ❌ | ❌ | ✅ via `oxi_ai::Provider` in `agent_runtime.rs`, `engine.rs` |
| 8 | `ProviderRegistry` | `ProviderRegistry` | ❌ | ❌ | ❌ Not used anywhere |
| 9 | `Model` | `Model` | ❌ | ❌ | ✅ via `oxi_ai::Model` in `engine.rs`, `supervisor.rs` |
| 10 | `StreamOptions` | `StreamOptions` | ❌ | ❌ | ✅ via `oxi_ai::StreamOptions` in `supervisor.rs` |
| 11 | `Agent` | `Agent` | ❌ | ❌ | ❌ Not used anywhere |
| 12 | `AgentLoop` | `AgentLoop` | ❌ | ❌ | ✅ via `oxi_agent::AgentLoop` in `agent_runtime.rs` |
| 13 | `AgentConfig` | `AgentConfig` | ❌ | ❌ | ❌ Not used anywhere |
| 14 | `AgentEvent` | `AgentEvent` | ❌ | ❌ | ✅ via `oxi_agent::AgentEvent` in `agent_runtime.rs` |
| 15 | `StructuredOutput` | `StructuredOutput` | ❌ | ❌ | ❌ Not used anywhere |
| 16 | `OutputMode` | `OutputMode` | ❌ | ❌ | ❌ Not used anywhere |
| 17 | `KernelToolProvider` | `KernelToolProvider` | ❌ | ❌ | ✅ via `oxi_sdk::KernelToolProvider` in `tools/kernel_bridge.rs` |
| 18 | `KernelToolContext` | `KernelToolContext` | ❌ | ❌ | ✅ via `oxi_sdk::KernelToolContext` in `tools/kernel_bridge.rs` |
| 19 | `Oxi` | `Oxi` | ❌ | ❌ | ✅ via `oxi_sdk::Oxi` in `engine.rs` |
| 20 | `OxiBuilder` | `OxiBuilder` | ❌ | ❌ | ✅ via `oxi_sdk::OxiBuilder` in `engine.rs` |
| 21 | `AgentBuilder` | `AgentBuilder` | ❌ | ❌ | ❌ Not used anywhere |
| 22 | `SdkAgentGroup` | `AgentGroup` | ❌ | ❌ | ❌ Not used anywhere (only in a comment in `agent_group.rs`) |
| 23 | `SdkGroupResult` | `GroupResult` | ❌ | ❌ | ❌ Not used anywhere |
| 24 | `SdkGroupStrategy` | `GroupStrategy` | ❌ | ❌ | ❌ Not used anywhere |
| 25 | `SdkAgentGroupOutput` | `AgentGroupOutput` | ❌ | ❌ | ❌ Not used anywhere |

---

## Category Analysis

### Category A: Dead re-exports (never used anywhere)
These are re-exported but referenced by **no** code in the entire workspace:

| Type | Recommendation |
|------|---------------|
| `SdkAgentMetrics` | **Remove** |
| `SdkMetricsSnapshot` | **Remove** |
| `ProviderPool` | **Remove** |
| `RateLimitPolicy` | **Remove** |
| `ProviderRegistry` | **Remove** |
| `Agent` | **Remove** |
| `AgentConfig` | **Remove** |
| `StructuredOutput` | **Remove** |
| `OutputMode` | **Remove** |
| `AgentBuilder` | **Remove** |
| `SdkAgentGroup` | **Remove** |
| `SdkGroupResult` | **Remove** |
| `SdkGroupStrategy` | **Remove** |
| `SdkAgentGroupOutput` | **Remove** |

**14 items — over half the re-exports — are completely unused.**

### Category B: Used internally but via `oxi_sdk::`/`oxi_agent::`/`oxi_ai::` directly
These are imported by kernel modules through the original crate path, not through the re-export:

| Type | Where used | Recommendation |
|------|-----------|---------------|
| `MessageBus` | `kernel_handle/a2a_api.rs` via `oxi_sdk::MessageBus` | Keep re-export (used by A2A subsystem) but fix import to use `crate::` |
| `InterAgentMessage` | `kernel_handle/a2a_api.rs` via `oxi_sdk::InterAgentMessage` | Keep re-export, fix import |
| `Provider` | `agent_runtime.rs` via `oxi_ai::Provider`, `engine.rs` via `oxi_sdk` | Keep re-export, fix import |
| `Model` | `engine.rs` via `oxi_sdk::Model`, `supervisor.rs` via `oxi_ai::Model` | Keep re-export, fix import |
| `StreamOptions` | `supervisor.rs` via `oxi_ai::StreamOptions` | Keep re-export, fix import |
| `AgentLoop` | `agent_runtime.rs` via `oxi_agent::AgentLoop` | Keep re-export, fix import |
| `AgentEvent` | `agent_runtime.rs` via `oxi_agent::AgentEvent` | Keep re-export, fix import |
| `KernelToolProvider` | `tools/kernel_bridge.rs` via `oxi_sdk::` | Keep re-export, fix import |
| `KernelToolContext` | `tools/kernel_bridge.rs` via `oxi_sdk::` | Keep re-export, fix import |
| `Oxi` | `engine.rs` via `oxi_sdk::Oxi` | Keep re-export, fix import |
| `OxiBuilder` | `engine.rs` via `oxi_sdk::OxiBuilder` | Keep re-export, fix import |

### Category C: Not referenced at all
See Category A. These should be removed.

---

## Downstream Consumer Analysis

| Consumer crate | Uses `oxios_kernel::` for SDK types? | Depends on `oxi-sdk` directly? |
|----------------|-------------------------------------|-------------------------------|
| `oxios` (binary) | ❌ No | ✅ Yes (but doesn't use it) |
| `oxios-web` | ❌ No | ❌ No |
| `oxios-gateway` | ❌ No | ❌ No |
| `oxios-ouroboros` | ❌ No | ❌ (depends on `oxi-ai` directly) |
| `oxios-cli` | ❌ No | ❌ No |
| `oxios-telegram` | ❌ No | ❌ No |

**No downstream consumer uses the re-exports through `oxios_kernel::`.** The binary has `oxi-sdk` as a direct dependency but never uses it.

---

## Direct Crate Usage Within oxios-kernel

The kernel uses three different import paths for oxi types:

| Import path | Files | Types imported |
|-------------|-------|---------------|
| `oxi_sdk::{...}` | `engine.rs`, `kernel_bridge.rs`, `a2a_api.rs` | `Oxi`, `OxiBuilder`, `KernelToolProvider`, `KernelToolContext`, `MessageBus`, `InterAgentMessage` |
| `oxi_agent::{...}` | `agent_runtime.rs`, all `tools/*.rs`, `tools/registration.rs` | `AgentLoop`, `AgentEvent`, `AgentLoopConfig`, `AgentTool`, `ToolRegistry`, `SearchCache`, etc. |
| `oxi_ai::{...}` | `agent_runtime.rs`, `engine.rs`, `supervisor.rs`, `credential.rs`, `onboarding.rs` | `Provider`, `Model`, `StreamOptions`, `CompactionStrategy`, `Context`, oauth helpers |

---

## Recommendations

### Phase 1: Remove dead re-exports (low risk)

Remove the 14 unused re-exports from the `pub use oxi_sdk` block in `lib.rs`:

```rust
// REMOVE these:
AgentMetrics as SdkAgentMetrics, MetricsSnapshot as SdkMetricsSnapshot,
ProviderPool, RateLimitPolicy,
ProviderRegistry,
Agent, AgentConfig,
StructuredOutput, OutputMode,
AgentBuilder,
AgentGroup as SdkAgentGroup,
GroupResult as SdkGroupResult,
GroupStrategy as SdkGroupStrategy,
AgentGroupOutput as SdkAgentGroupOutput,
```

### Phase 2: Fix internal imports to use `crate::` instead of `oxi_sdk::`/`oxi_agent::`/`oxi_ai::`

For the 11 items that are both re-exported AND used internally, standardize imports:

| File | Current | Target |
|------|---------|--------|
| `kernel_handle/a2a_api.rs` | `oxi_sdk::MessageBus`, `oxi_sdk::InterAgentMessage` | `crate::MessageBus`, `crate::InterAgentMessage` |
| `engine.rs` | `oxi_sdk::{Oxi, OxiBuilder}`, `oxi_sdk::Model`, `oxi_sdk::Provider` | `crate::{Oxi, OxiBuilder, Model, Provider}` |
| `tools/kernel_bridge.rs` | `oxi_sdk::{KernelToolContext, KernelToolProvider}` | `crate::{...}` |
| `agent_runtime.rs` | `oxi_agent::{AgentEvent, AgentLoop, ...}`, `oxi_ai::Provider` | `crate::{AgentLoop, AgentEvent, Provider}` |
| `supervisor.rs` | `oxi_ai::{Model, StreamOptions, ...}` | `crate::{Model, StreamOptions}` |

This is optional stylistically — both paths resolve to the same types — but it ensures the re-exports are actually exercised.

### Phase 3: Prepare for oxi-sdk full re-exports

When `oxi-sdk` adds full re-exports of `oxi-ai` and `oxi-agent` types:

1. **Drop `oxi-ai` from `oxios-kernel/Cargo.toml`** — replace all `use oxi_ai::` with `use oxi_sdk::`
2. **Drop `oxi-agent` from `oxios-kernel/Cargo.toml`** — replace all `use oxi_agent::` with `use oxi_sdk::`
3. **Drop `oxi-sdk` from root `Cargo.toml` `[dependencies]`** — the binary doesn't use it
4. **Drop `oxi-ai` from root `Cargo.toml` `[workspace.dependencies]`** — only `oxios-kernel` and `oxios-ouroboros` use it; once they use `oxi-sdk`, it can be removed

### Phase 4: Decide on re-export policy

**Option A (recommended): Keep a minimal re-export surface**

Only re-export types that consumers actually need:

```rust
pub use oxi_sdk::{
    MessageBus, InterAgentMessage,
    Provider, Model, StreamOptions,
    AgentLoop, AgentEvent,
    KernelToolProvider, KernelToolContext,
    Oxi, OxiBuilder,
};
```

**Option B: Re-export everything for convenience**

If you want `oxios_kernel` to be the single import point, re-export the full `oxi_sdk`:

```rust
pub use oxi_sdk; // re-export the entire crate
// Consumers: oxios_kernel::oxi_sdk::Agent
```

---

## Additional Findings

### `oxi-sdk` unused in binary

The root `Cargo.toml` lists `oxi-sdk = { workspace = true }` but `src/main.rs` and `src/kernel.rs` never use it. The binary should drop this dependency.

### `oxi-ai` used directly in `oxios-ouroboros`

```
crates/oxios-ouroboros/Cargo.toml: oxi-ai = { workspace = true }
```

When oxi-sdk re-exports oxi-ai types, this can be switched to `oxi-sdk`.

### Inconsistent provider imports

`Provider` trait is imported as `oxi_ai::Provider` in `agent_runtime.rs` but `engine.rs` uses `oxi_sdk::Provider` (via `dyn oxi_sdk::Provider`). Since `oxi_sdk` likely re-exports `oxi_ai::Provider`, these are the same type — but the inconsistency is confusing. Phase 2 fixes this.
