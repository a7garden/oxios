# Tool Architecture Redesign — Progress

## Phase 0: ClawGarden 잔재 제거 ✅
- Commit: `b72674d refactor(kernel): remove ClawGarden naming, adopt Oxios identity`

## Phase 1: ToolRegistry 재구성
### Batch 1-A: tools 모듈 생성
- [x] `tools/mod.rs` — 모듈 선언
- [x] `tools/container_exec.rs` — ContainerExecTool (BashTool 위임)
- [x] `tools/host_exec_tool.rs` — HostExecTool (HostExecBridge 래핑) — **10 tests pass**
- [x] `tools/program_tool.rs` — Phase 2용 placeholder
- [x] `lib.rs`에 `mod tools` + re-exports 추가

### Batch 1-B: agent_runtime.rs 수정
- [ ] `build_tool_registry()` 함수로 교체

### 상태
- Build: ✅ PASS
- Tests: ✅ PASS (200+ 통과)
