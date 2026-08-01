//! Unified asset store routes.
//!
//! - `GET  /api/assets/{name}` — **public** path-based binary serving
//!   (same rationale as `image_routes.rs`: `<img>` tags cannot send auth headers).
//! - `POST /api/assets` — multipart upload (protected).
//! - `GET  /api/assets` — list with filters (protected).
//! - `GET  /api/assets/{name}/meta` — metadata (protected).
//! - `PUT  /api/assets/{name}/meta` — update title/tags (protected).
//! - `DELETE /api/assets/{name}` — delete asset + file (protected).

use std::sync::Arc;

use axum::Json;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use oxios_kernel::asset_store::{AssetFilter, AssetSource, AssetStore};
use serde::Deserialize;

use crate::api::error::AppError;
use crate::api::server::AppState;

/// Helper: get the AssetStore from AppState, or return 503.
fn require_store(state: &AppState) -> Result<Arc<AssetStore>, AppError> {
    state
        .kernel
        .asset_store
        .as_ref()
        .cloned()
        .ok_or_else(|| AppError::ServiceUnavailable("asset store not available".into()))
}

// ── GET /api/assets/{name} — public binary serving ──────────────────

/// Serve an asset file directly from disk. Public route — no auth.
///
/// Path-based: the `{name}` (`{uuid}.{ext}`) maps directly to a file in
/// the assets root. Syntactic + canonicalization traversal guards, same
/// pattern as `image_routes.rs::handle_image_get`.
pub(crate) async fn handle_asset_get(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if name.is_empty() || name.contains('/') || name.contains("..") {
        return Err(AppError::BadRequest("invalid asset name".into()));
    }

    let store = require_store(&state)?;
    let base = store.root().to_path_buf();
    let canonical_base = base.canonicalize().unwrap_or_else(|_| base.clone());
    let full_path = base.join(&name);
    let canonical_file = match full_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return Err(AppError::NotFound("asset not found".into())),
    };
    if !canonical_file.starts_with(&canonical_base) {
        return Err(AppError::Forbidden("path traversal denied".into()));
    }

    let bytes = tokio::fs::read(&canonical_file)
        .await
        .map_err(|_| AppError::NotFound("asset not found".into()))?;

    let mime = asset_mime(&name).to_string();
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "public, max-age=86400".to_string()),
        ],
        bytes,
    ))
}

// ── POST /api/assets — multipart upload ──────────────────────────────

/// Upload a new asset via multipart form.
///
/// Fields:
/// - `file` (required) — the binary file.
/// - `source` (optional) — `upload` | `editor-paste` | `chat-attach` | `generated` | `web-search`.
///   Defaults to `upload`.
/// - `title` (optional) — title/description.
/// - `source_ref` (optional) — context reference (session_id, URL, etc.).
pub(crate) async fn handle_asset_upload(
    state: State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, AppError> {
    let store = require_store(&state)?;

    let mut file_bytes: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut source_str: Option<String> = None;
    let mut title: Option<String> = None;
    let mut source_ref: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("malformed multipart upload: {e}")))?
    {
        match field.name() {
            Some("file") => {
                filename = field.file_name().map(|s| s.to_string());
                let b = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("reading file field failed: {e}")))?;
                file_bytes = Some(b.to_vec());
            }
            Some("source") => {
                source_str = Some(field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("reading source field failed: {e}"))
                })?);
            }
            Some("title") => {
                title = Some(field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("reading title field failed: {e}"))
                })?);
            }
            Some("source_ref") => {
                source_ref = Some(field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("reading source_ref field failed: {e}"))
                })?);
            }
            _ => {}
        }
    }

    let bytes = file_bytes
        .ok_or_else(|| AppError::BadRequest("no 'file' field in multipart upload".into()))?;

    const MAX_UPLOAD: usize = 32 * 1024 * 1024;
    if bytes.len() > MAX_UPLOAD {
        return Err(AppError::PayloadTooLarge {
            size: bytes.len(),
            limit: MAX_UPLOAD,
        });
    }

    let fname = filename.unwrap_or_else(|| "upload.bin".to_string());
    let source = source_str
        .as_deref()
        .and_then(|s| s.parse::<AssetSource>().ok())
        .unwrap_or(AssetSource::Upload);

    let asset = store
        .store(bytes, &fname, source, source_ref)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Apply title if provided (store() doesn't take title).
    if let Some(ref t) = title {
        let _ = store.update_meta(&asset.storage_name, Some(t.clone()), asset.tags.clone());
    }

    let meta = store.get_meta(&asset.storage_name).unwrap_or(asset);

    tracing::info!(name = %meta.storage_name, filename = %fname, "Asset uploaded");
    Ok(Json(asset_to_json(&meta)))
}

// ── GET /api/assets — list with filters ──────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct AssetListQuery {
    #[serde(rename = "type")]
    type_filter: Option<String>,
    source: Option<String>,
    search: Option<String>,
    page: Option<usize>,
    limit: Option<usize>,
}

/// List assets with optional filtering and pagination.
pub(crate) async fn handle_asset_list(
    state: State<Arc<AppState>>,
    Query(q): Query<AssetListQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let store = require_store(&state)?;
    let filter = AssetFilter {
        type_filter: q.type_filter,
        source: q.source,
        search: q.search,
        page: q.page.unwrap_or(1),
        limit: q.limit.unwrap_or(24),
    };

    let (items, total) = store.list(&filter);
    let page = filter.page.max(1);
    let limit = if filter.limit == 0 { 24 } else { filter.limit };

    Ok(Json(serde_json::json!({
        "items": items.iter().map(asset_to_json).collect::<Vec<_>>(),
        "total": total,
        "page": page,
        "limit": limit,
    })))
}

// ── GET /api/assets/{name}/meta — metadata ───────────────────────────

/// Get metadata for a single asset.
pub(crate) async fn handle_asset_meta_get(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let store = require_store(&state)?;
    store
        .get_meta(&name)
        .map(|a| Json(asset_to_json(&a)))
        .ok_or_else(|| AppError::NotFound(format!("asset not found: {name}")))
}

// ── PUT /api/assets/{name}/meta — update metadata ────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct AssetMetaUpdate {
    title: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

/// Update asset metadata (title, tags).
pub(crate) async fn handle_asset_meta_update(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<AssetMetaUpdate>,
) -> Result<Json<serde_json::Value>, AppError> {
    let store = require_store(&state)?;
    let updated = store
        .update_meta(&name, body.title, body.tags)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("asset not found: {name}")))?;
    Ok(Json(asset_to_json(&updated)))
}

// ── DELETE /api/assets/{name} — delete ───────────────────────────────

/// Delete an asset (file + index entry).
pub(crate) async fn handle_asset_delete(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<StatusCode, AppError> {
    let store = require_store(&state)?;
    let deleted = store
        .delete(&name)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    if deleted {
        tracing::info!(name = %name, "Asset deleted");
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(format!("asset not found: {name}")))
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Serialize an [`Asset`](oxios_kernel::asset_store::Asset) to JSON.
fn asset_to_json(a: &oxios_kernel::asset_store::Asset) -> serde_json::Value {
    serde_json::json!({
        "id": a.id,
        "filename": a.filename,
        "title": a.title,
        "mime_type": a.mime_type,
        "size_bytes": a.size_bytes,
        "source": a.source,
        "source_ref": a.source_ref,
        "tags": a.tags,
        "width": a.width,
        "height": a.height,
        "duration_secs": a.duration_secs,
        "sha256": a.sha256,
        "created_at": a.created_at,
        "storage_name": a.storage_name,
        "url": format!("/api/assets/{}", a.storage_name),
    })
}

/// Best-effort MIME from filename extension (broader than `image_routes.rs`).
fn asset_mime(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("avif") => "image/avif",
        Some("bmp") => "image/bmp",
        Some("ico") => "image/x-icon",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("ogg") => "audio/ogg",
        Some("flac") => "audio/flac",
        Some("m4a") => "audio/mp4",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        Some("pdf") => "application/pdf",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}
