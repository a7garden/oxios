# Phase 2 Result: KnowledgeApi + Knowledge Tools + KernelHandle Integration

## Summary

Successfully implemented Phase 2 of the Knowledge System (RFC-004). All checks pass:
- `cargo check -p oxios-kernel` ✅ (0 errors, 0 warnings)
- `cargo test -p oxios-kernel` ✅ (541 tests pass, including 12 new knowledge_api tests + 1 new knowledge_tool test)
- `cargo check --workspace` ✅
- `cargo test --workspace` — 1 pre-existing failure in `oxios-web` unrelated to this phase

## What was implemented

### 1. Dependency addition
- Added `oxios-markdown = { path = "../oxios-markdown" }` to `oxios-kernel/Cargo.toml`

### 2. KnowledgeApi facade (`kernel_handle/knowledge_api.rs`)
12th KernelHandle API domain. Methods:
- `new(knowledge_dir, memory)` — create with directory + MemoryManager
- `for_space(space_dir, memory)` — create scoped to a Space
- `note_read(path)` — read .md file content
- `note_write(path, content)` — write .md + index backlinks + store in MemoryManager
- `note_delete(path)` — delete file + remove from backlink index
- `note_move(old_path, new_path)` — rename + re-index
- `note_tree(dir)` — list FileEntry items in directory
- `search(query, limit)` — name-based fuzzy search + semantic search via MemoryManager
- `backlinks_for(path)` — get backlinks for a note
- `link_graph()` — full link graph for visualization

Key design decisions:
- VirtualFs is wrapped in `Arc<RwLock<VirtualFs>>` for thread-safe interior mutability
- BacklinkIndex wrapped in `Arc<RwLock<BacklinkIndex>>` similarly
- `note_write` fires memory indexing async (graceful when no tokio runtime available)
- No EngineProvider dependency — copilot_chat deferred to Phase 4
- Uses `oxios_markdown::parser::extract_headings` for tags and `similar` for name similarity

### 3. KernelHandle integration (`kernel_handle/mod.rs`)
- Added `pub mod knowledge_api` and `pub use KnowledgeApi`
- Added `knowledge: KnowledgeApi` field to KernelHandle struct
- Updated `KernelHandle::new()` to accept 12 parameters (was 11)
- Updated `from_subsystems()` to create KnowledgeApi from `workspace/knowledge/`

### 4. KnowledgeTool (`tools/kernel/knowledge_tool.rs`)
Action-based agent tool following the memory_tools pattern:
- Actions: `read`, `write`, `delete`, `move`, `tree`, `search`, `backlinks`
- Uses `from_kernel(&KernelHandle)` pattern — extracts knowledge_dir + memory Arc
- Creates lightweight KnowledgeApi per invocation

### 5. Registration
- `tools/kernel/mod.rs` — added module + `register_all_kernel_tools()` call
- `tools/kernel_bridge.rs` — added `"knowledge"` to `tool_names()` (now 24 tools)
- `tools/registration.rs` — added `KernelDomain { "knowledge" }` CSpace case
- `tools/mod.rs` — exported `KnowledgeTool`
- `lib.rs` — exported `KnowledgeApi` + `KnowledgeTool`

### 6. Binary crate updates (`src/kernel.rs`)
- Both `KernelHandle::new()` calls updated to pass `KnowledgeApi`
- Knowledge directory: `workspace_path/knowledge/`

### 7. Test fixes
- `supervisor.rs` test helper updated for 12-param KernelHandle
- `kernel_bridge.rs` test updated for 12-param KernelHandle + 24 tool count

## Test coverage

New tests:
- `knowledge_api::tests::test_split_path_*` (4 tests) — path parsing
- `knowledge_api::tests::test_note_write_and_read` — roundtrip
- `knowledge_api::tests::test_note_read_missing` — missing file
- `knowledge_api::tests::test_note_delete` — deletion
- `knowledge_api::tests::test_note_move` — rename
- `knowledge_api::tests::test_backlinks` — backlink indexing
- `knowledge_api::tests::test_note_tree` — directory listing
- `knowledge_api::tests::test_search_by_name` — search
- `knowledge_api::tests::test_link_graph` — graph generation
- `knowledge_tool::tests::test_knowledge_tool_schema` — tool schema validation

## Files created
1. `crates/oxios-kernel/src/kernel_handle/knowledge_api.rs` (336 lines)
2. `crates/oxios-kernel/src/tools/kernel/knowledge_tool.rs` (290 lines)

## Files modified
1. `crates/oxios-kernel/Cargo.toml` (+2 lines)
2. `crates/oxios-kernel/src/kernel_handle/mod.rs` (12→12 APIs documented, +knowledge field/param)
3. `crates/oxios-kernel/src/tools/kernel/mod.rs` (+3 lines)
4. `crates/oxios-kernel/src/tools/kernel_bridge.rs` (+5 lines, test count 23→24)
5. `crates/oxios-kernel/src/tools/registration.rs` (+2 lines)
6. `crates/oxios-kernel/src/tools/mod.rs` (+1 line)
7. `crates/oxios-kernel/src/lib.rs` (+2 lines)
8. `crates/oxios-kernel/src/supervisor.rs` (+5 lines)
9. `src/kernel.rs` (+12 lines across 2 locations)
