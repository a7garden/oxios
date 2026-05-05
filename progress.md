# Phase 2-B Progress: .programs/*/program.toml 재작성

## Status: ✅ COMPLETE

### Files Changed
- `.programs/code-review/program.toml` — 새 스키마로 재작성 (지시형, requires_tools: read/container_exec/grep/find)
- `.programs/debug/program.toml` — 새 스키마로 재작성 (지시형, requires_tools: read/container_exec/grep/find)
- `.programs/deploy/program.toml` — 새 스키마로 재작성 (지시형, requires_tools: read/container_exec/grep)
- `.programs/refactor/program.toml` — 새 스키마로 재작성 (지시형, requires_tools: read/container_exec/grep/find/edit)
- `crates/oxios-kernel/src/mcp.rs` — ToolDef에 command 필드 누락 수정 (MCP tools → 빈 문자열)

### SKILL.md files
- 모두 변경 없음 (그대로 유지)

### Verification
- `cargo test -p oxios-kernel`: 222 passed, 0 failed
- `cargo test -p oxios-kernel -- program`: 2 passed (program install + tool schemas)
