# Tool Architecture Redesign — Progress

## Phase 0: ClawGarden 잔재 제거 ✅
- b72674d refactor(kernel): remove ClawGarden naming, adopt Oxios identity

## Phase 1: ToolRegistry 재구성
### Phase 1-A-2: container_exec.rs ✅
- File created: `crates/oxios-kernel/src/tools/container_exec.rs`
- ContainerExecTool implements AgentTool trait
- Delegates to BashTool for local fallback
- Uses ContainerManager.exec_in_container() for container path
- Compiles clean, 4 unit tests passing
- Build blocked by sibling files (host_exec_tool.rs, program_tool.rs) from parallel workers — not caused by this file
