# Phase 3 Result: Web UI Integration — Knowledge API Routes

## Summary

Successfully implemented the knowledge web editor integration and API routes for Oxios. All changes compile cleanly (`cargo check --workspace` passes).

## What Was Done

### 1. Copied files.md Web Assets
- Copied `/Volumes/MERCURY/PROJECTS/files.md/web/` → `channels/oxios-web/static/knowledge/`
- Includes: `index.html`, `app.js`, `editor.js`, `files.js`, `chat.js`, `app.css`, `chat.css`, `modals.js`, `welcome.js`, `lib/` (CodeMirror/HyperMD), `img/`, etc.

### 2. Modified JS for Oxios API Endpoints

**`files.js`:**
- Changed `API_URL` from `'https://api.files.md'` to `''` (same-origin)
- Replaced `syncTextsWithServer()` with no-op (Oxios uses direct REST CRUD)
- Replaced `syncLocalFileWithServer()` with no-op
- Replaced `syncMediaFiles()` with no-op
- Added `OXIOS_KNOWLEDGE_BASE = '/api/knowledge'` constant

**`app.js`:**
- Replaced token-based auth flow with `markServerOk()` call (Oxios handles auth via middleware)
- Removed `issuePermanentToken` fetch

**`index.html`:**
- Updated title to "Knowledge — Oxios"
- Added `oxios-adapter.js` script before `files.js`

**`oxios-adapter.js` (new):**
- Provides `oxiosReadFile()`, `oxiosWriteFile()`, `oxiosDeleteFile()`, `oxiosGetTree()`, `oxiosSearch()`, `oxiosGetBacklinks()`, `oxiosGetGraph()`, `oxiosCopilotChat()`
- Sets `OXIOS_MODE = true` and auto-marks server as ready

### 3. Created Knowledge API Route Handlers

**`channels/oxios-web/src/routes/knowledge_routes.rs`** — 8 handlers:

| Endpoint | Handler | Description |
|----------|---------|-------------|
| `GET /api/knowledge/tree` | `handle_knowledge_tree` | File tree of `knowledge/` directory |
| `GET /api/knowledge/file/{*path}` | `handle_knowledge_file_get` | Read a knowledge file |
| `PUT /api/knowledge/file/{*path}` | `handle_knowledge_file_put` | Write/update a knowledge file (max 5MB) |
| `DELETE /api/knowledge/file/{*path}` | `handle_knowledge_file_delete` | Delete a knowledge file |
| `POST /api/knowledge/search` | `handle_knowledge_search` | Text search across `.md` files |
| `GET /api/knowledge/backlinks` | `handle_knowledge_backlinks` | Get backlinks for a file (scans for `[text](path)` links) |
| `GET /api/knowledge/graph` | `handle_knowledge_graph` | Link graph for visualization |
| `POST /api/knowledge/copilot` | `handle_knowledge_copilot` | AI copilot (placeholder until Phase 4) |

All handlers use filesystem operations on `{workspace}/knowledge/` as fallback (since Phase 2's KnowledgeApi isn't wired yet). Security: path traversal protection on all file operations.

**Unit tests:** 7 tests covering MIME guessing, link extraction, wikilinks, URL filtering, tree entry serialization, and search hit serialization.

### 4. Registered Routes in mod.rs

- Added `mod knowledge_routes;`
- Added `pub(crate) use knowledge_routes::{...}` re-exports
- Added 8 routes in `build_routes()` function under the protected API route group (auth middleware applied)

### 5. Static Knowledge Assets

- Updated `middleware.rs` to add `"/knowledge/"` to the `static_prefixes` whitelist — no auth required for static knowledge assets
- Static files are served by the existing `fallback_service(ServeDir::new("static"))` in `plugin.rs`
- `/knowledge/` → `static/knowledge/index.html` (via `append_index_html_on_directories(true)`)

### 6. Verification
- ✅ `cargo check -p oxios-web` — passes
- ✅ `cargo check --workspace` — passes (0 errors)

## Architecture Decisions

1. **Simplified sync protocol**: The original files.md mtime-based 3-way merge sync was replaced with simple REST CRUD. This removes complexity while keeping the editor functional. The sync engine can be re-enabled later when `oxios-markdown` crate is integrated.

2. **Filesystem fallback**: All handlers use direct filesystem I/O on `{workspace}/knowledge/` instead of the planned KnowledgeApi. This allows Phase 3 to work independently of Phase 2. When KnowledgeApi is wired, the handlers can be updated to use it.

3. **Search = simple text scan**: The search handler scans `.md` files line-by-line for substring matches. This is functional but not optimal. Will be replaced with HNSW semantic search via KnowledgeApi.

4. **Backlinks = file scanning**: Backlinks are computed by scanning all `.md` files for markdown link patterns. Will be replaced with BacklinkIndex when available.

5. **Copilot = placeholder**: Returns a helpful message indicating the AI engine is not yet wired. Ready for Phase 4 integration.

## Next Steps

- **Phase 2 completion**: Wire KnowledgeApi into KernelHandle
- **Phase 4**: Implement copilot using oxi engine for real AI-powered Q&A
- **Phase 5**: Add HNSW semantic search, BacklinkIndex, graph visualization with PageRank
