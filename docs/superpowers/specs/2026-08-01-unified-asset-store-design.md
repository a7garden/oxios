# Unified Asset Store — Design Spec

**Date:** 2026-08-01
**Status:** Approved → Implementation
**Scope:** Full-stack (backend API, storage, frontend gallery, editor integration, chat integration, generated-image migration)

## 1. Problem

Oxios currently has three fragmented image/asset systems:

| System | Location | Route | Issue |
|--------|----------|-------|-------|
| Generated images | `<workspace>/images/<uuid>.<ext>` | `GET /api/images/{name}` (public) | Outside KB, no metadata, not browsable |
| Knowledge assets | `~/.oxios/knowledge/...` | `GET /api/knowledge/asset/{*path}` (auth) | Read-only — no upload endpoint |
| Chat attachments | base64 inline | none | Ephemeral, bloats message payloads |

No central store. No browse/manage UI. Assets can't be reused across contexts.

## 2. Solution

A single central asset store at `~/.oxios/assets/`, served via `/api/assets/{name}` (public, path-based), with a JSON metadata index for browsing/filtering.

## 3. Storage

```
~/.oxios/assets/
├── index.json                # metadata manifest (JSON array)
├── {uuid}.png                # flat layout — UUID filename prevents collisions
├── {uuid}.webp
└── {uuid}.mp3
```

**Why flat, not date-partitioned:** UUID filenames eliminate collisions. Date-based subdirectories add complexity for no benefit in a local-first app. Path-based serving becomes trivial: `GET /api/assets/{uuid}.ext` maps directly to a file on disk.

**Why JSON, not SQLite:** The project uses SQLite for high-volume query-heavy data (memory, agent logs, tasks). Asset metadata is low-volume (hundreds of entries), low-write, and needs only simple filtering. JSON is simpler to implement, inspect, and migrate. If the store ever needs FTS or thousands of entries, migration to SQLite is mechanical.

## 4. Asset Metadata Schema (`index.json`)

```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "filename": "screenshot.png",
    "title": null,
    "mime_type": "image/png",
    "size_bytes": 12345,
    "source": "upload",
    "source_ref": null,
    "tags": [],
    "width": 1024,
    "height": 768,
    "duration_secs": null,
    "sha256": "abc123def456...",
    "created_at": 1753990800,
    "storage_name": "550e8400-e29b-41d4-a716-446655440000.png"
  }
]
```

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `id` | string (UUID) | Stable identifier, same as UUID in `storage_name` |
| `filename` | string | Original uploaded filename |
| `title` | string? | User-editable title/description |
| `mime_type` | string | Guessed from filename extension + magic bytes |
| `size_bytes` | u64 | File size |
| `source` | enum | `upload` \| `editor-paste` \| `chat-attach` \| `generated` \| `web-search` \| `imported` |
| `source_ref` | string? | Context: session_id, URL, agent_id, etc. |
| `tags` | string[] | User-assigned tags for organization |
| `width` | u32? | Image width (if image) |
| `height` | u32? | Image height (if image) |
| `duration_secs` | f64? | Audio/video duration (if applicable) |
| `sha256` | string | Content hash for dedup |
| `created_at` | i64 | Unix timestamp |
| `storage_name` | string | Filename on disk: `{uuid}.{ext}` |

**Dedup:** On upload, compute SHA256. If an asset with the same hash exists, return the existing asset instead of duplicating.

## 5. Backend API

All routes in `src/api/routes/asset_routes.rs`, following the existing axum patterns.

### 5.1 Routes

| Method | Route | Auth | Purpose |
|--------|-------|------|---------|
| `POST` | `/api/assets` | protected | Multipart upload (`file` + optional `source`, `title`, `tags`) |
| `GET` | `/api/assets` | protected | List with filters (JSON response) |
| `GET` | `/api/assets/{name}` | **public** | Serve binary directly from disk |
| `GET` | `/api/assets/{name}/meta` | protected | Get metadata for a specific asset |
| `PUT` | `/api/assets/{name}/meta` | protected | Update title/tags |
| `DELETE` | `/api/assets/{name}` | protected | Delete asset + file + index entry |

### 5.2 Path-Based Serving (Critical Design Decision)

`GET /api/assets/{name}` is **path-based**, not index-based. It serves the file `{name}` directly from `~/.oxios/assets/` with:

- Syntactic guard: `{name}` must be a single segment, no `/`, no `..`
- Canonicalization guard: resolved path must be under the assets root
- MIME guessing from extension (same as `image_routes.rs`)

**Why path-based:** Generated images are written to disk by `FsImageStore` before the chat handler registers them in the index. If serving were index-based, a streaming/cron/A2A race would 404. Path-based serving works the moment the file hits disk — the index is metadata-only for browsing/filtering, not a prerequisite for retrieval.

This is the same pattern as the existing `GET /api/images/{name}` route.

### 5.3 List Endpoint

`GET /api/assets?type=image&source=generated&search=screenshot&page=1&limit=24`

Returns:
```json
{
  "items": [/* AssetMeta[] */],
  "total": 42,
  "page": 1,
  "limit": 24
}
```

Filters:
- `type` — `image` | `audio` | `video` | `document` (derived from `mime_type`)
- `source` — filter by source enum
- `search` — substring match on filename + title + tags
- `page` / `limit` — pagination (default 24)

### 5.4 AssetStore Struct

In the binary crate, initialized in `AppState`:

```rust
pub struct AssetStore {
    root: PathBuf,               // ~/.oxios/assets/
    index: RwLock<Vec<Asset>>,
}
```

- `new(root)` — creates dir, loads `index.json` (or starts empty)
- `save(&self, bytes, filename, source, ...) -> Asset` — writes file, computes SHA256, dedup check, appends to index, persists atomically
- `list(&self, filter) -> (Vec<&Asset>, total)` — filter + paginate
- `get_meta(&self, name) -> Option<&Asset>`
- `update_meta(&self, name, title, tags) -> Result`
- `delete(&self, name) -> Result` — removes file + index entry
- `register(&self, name, filename, source, source_ref) -> Asset` — registers an existing file (for generated images, migration)

Index persistence: `index.json` written atomically (temp file + rename) on every mutation. Loaded once at startup.

## 6. Generated Images Integration

### 6.1 FsImageStore Redirect

`oxios-kernel/src/image_gen/store.rs` — `FsImageStore` currently:
- dir: `<workspace>/images/`
- serve_prefix: `/api/images/`

Changes:
- dir: `~/.oxios/assets/` (resolved from oxios home, not workspace)
- serve_prefix: `/api/assets/`

The `image_generation_tool.rs` sets `images_dir: kernel.state.workspace_path().join("images")` — change to resolve the oxios home assets directory.

### 6.2 Registration Flow

When the chat handler processes a completed `image_generation` tool result, it extracts image URLs (`/api/assets/{uuid}.ext`) and calls `AssetStore::register()` with `source: "generated"` and `source_ref: session_id`. This adds the metadata entry.

If registration is skipped (cron, A2A), the image still serves correctly via path-based serving — it just won't appear in the gallery until a lazy scan registers it.

### 6.3 Backward Compatibility

- `GET /api/images/{name}` stays registered as a fallback — serves from `<workspace>/images/` if the file exists there (for notes that already reference old URLs).
- On startup, `AssetStore` scans `~/.oxios/assets/` for files not in the index and registers them (handles the lazy registration case).

## 7. Frontend — Asset Gallery

New route `/assets` in the web UI, accessible from the sidebar.

### 7.1 Layout

- **Header** — title, upload button, type filter tabs (All / Images / Audio / Video / Documents)
- **Toolbar** — search input, source filter dropdown, sort dropdown
- **Grid** — responsive thumbnail grid (images show preview, others show icon + filename)
- **Detail drawer** — click an asset to open: full preview (images), metadata (title, tags, source, size, dimensions), copy URL button, edit metadata, delete button

### 7.2 Upload

- Drag-and-drop anywhere on the gallery page
- Click upload button → file picker
- Multi-file upload supported
- Shows progress state

## 8. Frontend — Knowledge Editor Integration

### 8.1 Paste / Drag-Drop in Markdown Editor

When an image is pasted or dropped into the CodeMirror editor:
1. Upload to `/api/assets` with `source: "editor-paste"`
2. Insert `![](/api/assets/{uuid}.ext)` at cursor — **full absolute URL**, no relative-path resolution needed since assets are centrally stored
3. The existing `resolveRelativeImages`/`stripResolvedImages` functions are NOT involved for asset URLs — they only handle knowledge-relative paths

### 8.2 isAbsoluteUrl Update

`markdown-editor.tsx:80` — `isAbsoluteUrl()` must include `/api/assets/` in its pattern so these URLs are not mangled by `resolveRelativeImages`:

```typescript
function isAbsoluteUrl(url: string): boolean {
  return /^(https?:|data:|blob:|about:)/.test(url)
    || url.startsWith(ASSET_ROUTE)        // /api/knowledge/asset
    || url.startsWith('/api/assets/')     // NEW: unified asset store
}
```

## 9. Frontend — Chat Input Integration

Currently images are read as base64 data URLs and embedded inline. New behavior:

1. When attaching an image file, upload to `/api/assets` with `source: "chat-attach"`
2. The message carries the asset URL `/api/assets/{uuid}.ext` instead of the base64 data URL
3. Keeps message payloads lean; images become persistent and reusable

The `AttachedFile` type and `readFile` logic in `chat-input.tsx` are updated to support this upload-then-reference flow.

## 10. API Client (Frontend)

New hooks/utilities in `web/src/hooks/use-assets.ts`:

- `useAssets(filter)` — list assets with filters
- `useUploadAsset()` — upload mutation (multipart POST)
- `useDeleteAsset()` — delete mutation
- `useUpdateAssetMeta()` — update metadata mutation

Uses `api.upload()` (FormData helper) for uploads, `api.get/post/put/delete` for the rest.

## 11. File Inventory

### Backend (Rust)

| File | Action |
|------|--------|
| `src/api/routes/asset_routes.rs` | **New** — HTTP handlers (upload, list, serve, meta, delete) |
| `src/api/routes/mod.rs` | **Edit** — register asset routes (public + protected) |
| `src/api/server.rs` | **Edit** — add `AssetStore` to `AppState` |
| `src/api/plugin.rs` | **Edit** — initialize `AssetStore` on startup |
| `crates/oxios-kernel/src/image_gen/store.rs` | **Edit** — redirect dir to `~/.oxios/assets/`, serve prefix to `/api/assets/` |
| `crates/oxios-kernel/src/tools/builtin/image_generation_tool.rs` | **Edit** — update `images_dir` resolution |
| `src/main.rs` or `src/kernel.rs` | **Edit** — pass assets dir to FsImageStore |

### Frontend (TypeScript/React)

| File | Action |
|------|--------|
| `web/src/hooks/use-assets.ts` | **New** — API hooks for asset CRUD |
| `web/src/types/asset.ts` | **New** — TypeScript types for asset metadata |
| `web/src/routes/assets.tsx` | **New** — asset gallery page route |
| `web/src/components/assets/asset-gallery.tsx` | **New** — gallery grid + upload + filters |
| `web/src/components/assets/asset-detail.tsx` | **New** — detail drawer/preview |
| `web/src/components/assets/upload-zone.tsx` | **New** — drag-drop upload zone |
| `web/src/components/knowledge/markdown-editor.tsx` | **Edit** — paste/drop handlers, `isAbsoluteUrl` update |
| `web/src/components/chat/chat-input.tsx` | **Edit** — upload images to asset store instead of base64 |
| `web/src/components/layout/sidebar.tsx` | **Edit** — add Assets nav item |

## 12. Non-Goals (This Iteration)

- **Cloud sync** — no S3/remote storage. Local filesystem only.
- **Image editing** — no crop/resize/annotate.
- **Collections/albums** — tags are sufficient for now.
- **Version history** — no asset revision tracking.
- **Access control per asset** — all assets are accessible to all authenticated users (single-user local-first app).
- **Audio waveform preview** — play button only, no waveform visualization.
