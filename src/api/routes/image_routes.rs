//! Image serving route for generated images.
//!
//! Serves files materialized by the kernel's image-gen store (base64
//! responses) under `<workspace>/images/`. Registered as a **public** route
//! (no auth) so agent-emitted markdown `![](url)` renders in the browser —
//! `<img>` tags cannot send the `Authorization: Bearer` header. Safe because
//! Oxios is local-first (loopback bind by default) and filenames are
//! unguessable UUIDs, the same rationale as the public marketplace routes.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;

use crate::api::error::AppError;
use crate::api::server::AppState;

/// GET /api/images/{name} — serve a generated image by filename.
///
/// `{name}` is `<uuid>.<ext>` as produced by [`FsImageStore`]. Path
/// traversal is rejected both syntactically and via canonicalization.
///
/// [`FsImageStore`]: oxios_kernel::image_gen::FsImageStore
pub(crate) async fn handle_image_get(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    // Syntactic guard: a valid filename is a single segment, no traversal.
    if name.is_empty() || name.contains('/') || name.contains("..") {
        return Err(AppError::BadRequest("invalid image name".into()));
    }

    let base = state.kernel.state.workspace_path().join("images");
    let canonical_base = base.canonicalize().unwrap_or_else(|_| base.clone());
    let full_path = base.join(&name);
    let canonical_file = match full_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return Err(AppError::NotFound("image not found".into())),
    };
    if !canonical_file.starts_with(&canonical_base) {
        return Err(AppError::Forbidden("path traversal denied".into()));
    }

    let bytes = tokio::fs::read(&canonical_file)
        .await
        .map_err(|_| AppError::NotFound("image not found".into()))?;

    let mime = image_mime(&name).to_string();
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "public, max-age=86400".to_string()),
        ],
        bytes,
    ))
}

/// Best-effort MIME from the filename extension.
fn image_mime(name: &str) -> &'static str {
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
        _ => "application/octet-stream",
    }
}
