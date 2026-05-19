# Progress

## Status
In Progress — Phase 2 complete, Phase 3 partially complete

## Tasks

### Phase 2: KnowledgeApi + Knowledge Tools + KernelHandle Integration
- [x] Add `oxios-markdown` as dependency to `oxios-kernel/Cargo.toml`
- [x] Create `kernel_handle/knowledge_api.rs` — 12th KernelHandle API facade
- [x] Add `knowledge` module to `kernel_handle/mod.rs` (field + constructor)
- [x] Create `tools/kernel/knowledge_tool.rs` — action-based agent tool
- [x] Register KnowledgeTool in `tools/kernel/mod.rs`
- [x] Add `"knowledge"` to `kernel_bridge.rs::tool_names()`
- [x] Add `KernelDomain { "knowledge" }` case in `registration.rs`
- [x] Update `src/kernel.rs` — both KernelHandle::new() calls pass KnowledgeApi
- [x] Update `supervisor.rs` test helper — pass KnowledgeApi
- [x] `cargo check -p oxios-kernel` passes (0 errors, 0 warnings)
- [x] `cargo test -p oxios-kernel` passes (541 tests)
- [x] `cargo check --workspace` passes

### Phase 3: Web UI Integration — Knowledge API Routes
- [x] Copy files.md web assets to `channels/oxios-web/static/knowledge/`
- [x] Modify JS to use Oxios API endpoints (disabled sync protocol, set API_URL to same-origin)
- [x] Create `oxios-adapter.js` bridge for Oxios REST API
- [x] Create `routes/knowledge_routes.rs` with all 8 handlers
- [x] Register knowledge module and routes in `routes/mod.rs`
- [x] Update `middleware.rs` to allow `/knowledge/` static assets without auth
- [x] `cargo check -p oxios-web` passes
- [x] `cargo check --workspace` passes
- [ ] Verify static knowledge assets are served correctly (needs running server)
- [ ] Create default knowledge files (Chat.md, Later.md, etc.)

## Files Changed (Phase 2)

### New Files
- `crates/oxios-kernel/src/kernel_handle/knowledge_api.rs` — KnowledgeApi facade (12th API domain)
- `crates/oxios-kernel/src/tools/kernel/knowledge_tool.rs` — KnowledgeTool agent tool

### Modified Files
- `crates/oxios-kernel/Cargo.toml` — added `oxios-markdown` dependency
- `crates/oxios-kernel/src/kernel_handle/mod.rs` — added knowledge module, KnowledgeApi field, updated constructors
- `crates/oxios-kernel/src/tools/kernel/mod.rs` — added knowledge_tool module + registration
- `crates/oxios-kernel/src/tools/kernel_bridge.rs` — added "knowledge" to tool_names, updated test
- `crates/oxios-kernel/src/tools/registration.rs` — added knowledge domain CSpace case
- `crates/oxios-kernel/src/tools/mod.rs` — exported KnowledgeTool
- `crates/oxios-kernel/src/lib.rs` — exported KnowledgeApi + KnowledgeTool
- `crates/oxios-kernel/src/supervisor.rs` — updated test KernelHandle construction
- `src/kernel.rs` — updated both KernelHandle::new() calls (12 params now)

## Notes (Phase 2)
- KnowledgeApi does NOT depend on EngineProvider — copilot_chat is Phase 4
- VirtualFs methods are synchronous; note_write fires memory indexing async (graceful when no tokio rt)
- BacklinkIndex is wrapped in `RwLock` for thread-safe interior mutability
- KnowledgeTool creates a fresh KnowledgeApi per invocation (lightweight — just wraps paths)
- One pre-existing test failure in `oxios-web` (`test_extract_links_wikilink`) — not introduced by this phase
