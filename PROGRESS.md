# Progress

## Status
Completed

## Tasks
- [x] Delete `crates/oxios-kernel/src/program/` directory (mod.rs, types.rs, parser.rs, installer.rs)
- [x] Delete `crates/oxios-kernel/src/host_tools.rs` — entire file
- [x] Delete `crates/oxios-kernel/src/tools/program_tool.rs` — entire file
- [x] Create `crates/oxios-kernel/src/tools/tool_types.rs` with `ToolDef` and `ArgumentDef`
- [x] Update `tools/mod.rs` — remove program_tool module, add tool_types module
- [x] Update `tools/kernel/mod.rs` — remove ProgramTool registration
- [x] Update `tools/kernel_bridge.rs` — remove ProgramManager/HostToolValidator from tests, remove "program" from tool_names
- [x] Update `tools/registration.rs` — update comments about ProgramTool
- [x] Rewrite `kernel_handle/extension_api.rs` — remove all legacy ProgramManager/HostToolValidator code
- [x] Update `kernel_handle/mcp_api.rs` — change import to tool_types::ToolDef
- [x] Update `kernel_handle/mod.rs` — update deprecated from_subsystems to use SkillManager instead of SkillStore
- [x] Update `mcp/mod.rs` — change import to tool_types::{ArgumentDef, ToolDef}
- [x] Update `lib.rs` — remove program/host_tools modules and re-exports, remove SkillStore/ProgramTool, add tool_types re-exports
- [x] Update `config.rs` — remove required_host_tools/optional_host_tools from ExecConfig
- [x] Update `supervisor.rs` — update test to use new ExtensionApi::new(skill_manager) signature
- [x] Update `agent_runtime.rs` — update program tools comment
- [x] Update `skill.rs` — remove SkillStore compatibility shim
- [x] Remove program-related integration tests from tests/integration_tests.rs
- [x] `cargo check -p oxios-kernel --tests` passes with 0 errors

## Files Changed
- `crates/oxios-kernel/src/program/` — **deleted entirely** (mod.rs, types.rs, parser.rs, installer.rs)
- `crates/oxios-kernel/src/host_tools.rs` — **deleted entirely**
- `crates/oxios-kernel/src/tools/program_tool.rs` — **deleted entirely**
- `crates/oxios-kernel/src/tools/tool_types.rs` — **new file** (ToolDef + ArgumentDef types)
- `crates/oxios-kernel/src/tools/mod.rs` — removed program_tool module/re-export, added tool_types
- `crates/oxios-kernel/src/tools/kernel/mod.rs` — removed ProgramTool registration
- `crates/oxios-kernel/src/tools/kernel_bridge.rs` — removed legacy args from ExtensionApi::new(), removed "program" from tool_names
- `crates/oxios-kernel/src/tools/registration.rs` — updated comments
- `crates/oxios-kernel/src/kernel_handle/extension_api.rs` — rewrote to only use SkillManager
- `crates/oxios-kernel/src/kernel_handle/mcp_api.rs` — updated import path
- `crates/oxios-kernel/src/kernel_handle/mod.rs` — updated deprecated from_subsystems
- `crates/oxios-kernel/src/mcp/mod.rs` — updated import path
- `crates/oxios-kernel/src/lib.rs` — removed program/host_tools modules and legacy re-exports
- `crates/oxios-kernel/src/config.rs` — removed required_host_tools/optional_host_tools from ExecConfig
- `crates/oxios-kernel/src/supervisor.rs` — updated test ExtensionApi construction
- `crates/oxios-kernel/src/agent_runtime.rs` — updated comment
- `crates/oxios-kernel/src/skill.rs` — removed SkillStore compatibility shim
- `crates/oxios-kernel/tests/integration_tests.rs` — removed 2 program-related tests

## Notes
- `cargo check -p oxios-kernel --tests` passes with 0 errors (84 pre-existing warnings)
- The `oxios` binary crate has pre-existing errors unrelated to this cleanup (`.meta` field access on `SkillEntry`, `SkillManager` clone, `dream` module privacy) — these are part of the broader Skill unification and not in scope
- All remaining references to ProgramManager/SkillStore/HostToolValidator are in comments only
- `ToolDef` and `ArgumentDef` now live in `tools/tool_types.rs` — used by MCP adapters

### Binary Crate Cleanup (this task)

- [x] Update `src/kernel.rs` — remove ProgramManager/HostToolValidator imports and usage, update ExtensionApi::new() signature
- [x] Update `src/main.rs` — remove Command::Program, Pkg methods now use SkillManager, remove host_tools config handling
- [x] `cargo check -p oxios-kernel` passes with 0 errors (the dream module error is pre-existing)
