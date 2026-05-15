# Kernel Tool Implementation Output

## Summary

Implemented 7 new `AgentTool` wrappers that expose KernelHandle API domains to the agent tool-calling loop. All files follow the established action-based parameter pattern (same as `BrowserTool`).

## Files Created

| # | File | Lines | Description |
|---|------|-------|-------------|
| 1 | `crates/oxios-kernel/src/tools/kernel/mod.rs` | ~35 | Module file exporting all 7 tools |
| 2 | `crates/oxios-kernel/src/tools/kernel/space_tool.rs` | ~220 | SpaceTool — Space management |
| 3 | `crates/oxios-kernel/src/tools/kernel/agent_tool.rs` | ~195 | AgentTool (re-exported as `KernelAgentTool`) — Agent lifecycle |
| 4 | `crates/oxios-kernel/src/tools/kernel/persona_tool.rs` | ~165 | PersonaTool — Persona management |
| 5 | `crates/oxios-kernel/src/tools/kernel/cron_tool.rs` | ~210 | CronTool — Cron scheduling |
| 6 | `crates/ERCURY/PROJECTS/oxios/crates/oxios-kernel/src/tools/kernel/security_tool.rs` | ~170 | SecurityTool — Audit trail |
| 7 | `crates/oxios-kernel/src/tools/kernel/budget_tool.rs` | ~195 | BudgetTool — Budget management |
| 8 | `crates/oxios-kernel/src/tools/kernel/resource_tool.rs` | ~185 | ResourceTool — Resource monitoring |

## Files Modified

| File | Change |
|------|--------|
| `crates/oxios-kernel/src/tools/mod.rs` | Added `pub mod kernel;` and `pub use kernel::{...}` |

## Design Decisions

### Arc Extraction Pattern
Each tool extracts the needed `Arc<Inner>` from the KernelHandle's API struct fields (which are `pub(crate)`). This avoids cloning the API struct itself (which is not `Clone`) while sharing the underlying state via `Arc`.

Example:
```rust
pub fn from_kernel(kernel: &KernelHandle) -> Self {
    Self {
        space_manager: kernel.spaces.space_manager.clone(),
    }
}
```

### AgentTool Naming Conflict
The struct `AgentTool` in `agent_tool.rs` would conflict with the trait `oxi_agent::AgentTool` in the same module scope. Resolved by importing the trait with an alias:
```rust
use oxi_agent::AgentTool as OxiAgentTool;
```
The struct is re-exported from `mod.rs` as `KernelAgentTool` to avoid confusion at the crate level.

### SpaceTool Delegation
`SpaceTool` creates a temporary `SpaceApi` to delegate to, since `SpaceApi` methods operate on the `Arc<SpaceManager>` which is shared. A fresh `EventBus` is created for the temporary API (only needed for future event publishing, not used by the called methods).

### Action-Based Schemas
All tools follow the same parameter schema pattern:
- `action` (required, enum) — selects the operation
- Additional parameters vary by action
- Returns `AgentToolResult::success(json)` or `AgentToolResult::error(msg)`

## Tool Actions Summary

| Tool | Actions |
|------|---------|
| `SpaceTool` | `list`, `get`, `create`, `archive`, `merge`, `restore` |
| `KernelAgentTool` | `list`, `kill`, `budget` |
| `PersonaTool` | `list`, `get`, `set_active` |
| `CronTool` | `list`, `add`, `remove`, `trigger` |
| `SecurityTool` | `verify_chain`, `query_audit`, `audit_count` |
| `BudgetTool` | `check`, `set`, `reserve`, `reset` |
| `ResourceTool` | `snapshot`, `history`, `overloaded` |

## Compilation Status

✅ All new files compile without errors or warnings.

⚠️ The crate has 3 pre-existing errors in `orchestrator.rs` and `agent_group.rs` (missing `cspace_hint` field in `Seed` struct) that prevent a full build. These are unrelated to this change.

## Tests

Each tool file includes `#[cfg(test)] mod tests` with schema structure validation tests. Integration tests (exercising actual tool execution against live KernelHandle subsystems) require the pre-existing compilation errors to be resolved first.
