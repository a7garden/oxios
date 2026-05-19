# Progress

## Status
In Progress — Phase 3 partially complete

## Tasks

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

## Files Changed

### New Files
- `channels/oxios-web/static/knowledge/` — entire files.md web frontend copied
- `channels/oxios-web/static/knowledge/oxios-adapter.js` — Oxios API adapter for JS
- `channels/oxios-web/src/routes/knowledge_routes.rs` — Knowledge REST API handlers

### Modified Files
- `channels/oxios-web/static/knowledge/files.js` — disabled sync protocol, set API_URL to ''
- `channels/oxios-web/static/knowledge/app.js` — disabled token auth, marked server OK
- `channels/oxios-web/static/knowledge/index.html` — added oxios-adapter.js, updated title
- `channels/oxios-web/src/routes/mod.rs` — added knowledge_routes module + routes
- `channels/oxios-web/src/middleware.rs` — added /knowledge/ to static asset whitelist

## Notes
- The static knowledge assets are served via the existing fallback_service (ServeDir) in plugin.rs, which serves everything under `static/`. The `/knowledge/` prefix maps to `static/knowledge/index.html`.
- Sync protocol (mtime-based 3-way merge) is disabled; files are saved via simple REST CRUD.
- Copilot handler returns a placeholder response until the oxi engine is integrated (Phase 4).
- Search and backlinks use simple file scanning as fallback until KnowledgeApi is wired (Phase 2).
