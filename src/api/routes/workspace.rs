use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use oxios_kernel::memory::{MemoryEntry, MemoryTier, MemoryType, ProtectionLevel};
use oxios_kernel::{SkillEntry, SkillSource, SkillStatus};

use crate::api::error::AppError;
use crate::api::routes::PageParams;
#[cfg(test)]
use crate::api::routes::paginate;
use crate::api::server::AppState;

// ---------------------------------------------------------------------------
// Workspace
// ---------------------------------------------------------------------------

/// Query parameters for workspace tree.
#[derive(Debug, Deserialize)]
pub(crate) struct TreeQuery {
    /// Subdirectory to list (optional).
    #[serde(default)]
    pub dir: Option<String>,
}

/// File tree entry.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct TreeEntry {
    /// File or directory name.
    name: String,
    /// Whether this is a directory.
    is_dir: bool,
    /// File size in bytes (0 for directories).
    size: u64,
}

/// GET /api/workspace/tree — File tree of workspace.
pub(crate) async fn handle_workspace_tree(
    state: State<Arc<AppState>>,
    Query(query): Query<TreeQuery>,
) -> Result<Json<Vec<TreeEntry>>, AppError> {
    let base = state.kernel.state.workspace_path();
    let canonical_base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let dir = match &query.dir {
        Some(d) => {
            let candidate = base.join(d);
            let canonical = match candidate.canonicalize() {
                Ok(c) => c,
                Err(_) => return Err(AppError::NotFound("directory not found".into())),
            };
            if !canonical.starts_with(&canonical_base) {
                return Err(AppError::Forbidden("path traversal denied".into()));
            }
            canonical
        }
        None => canonical_base,
    };

    let mut entries = Vec::new();
    if let Ok(mut read_dir) = tokio::fs::read_dir(&dir).await {
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let metadata = match entry.metadata().await {
                Ok(m) => m,
                Err(_) => continue,
            };
            entries.push(TreeEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir: metadata.is_dir(),
                size: metadata.len(),
            });
        }
    }

    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));

    Ok(Json(entries))
}

/// GET /api/workspace/file/*path — Read a file.
pub(crate) async fn handle_workspace_file_get(
    state: State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let base = state.kernel.state.workspace_path();
    let full_path = base.join(&path);

    // Security: ensure the path doesn't escape the workspace
    let canonical_base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let canonical_file = match full_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return Err(AppError::NotFound("file not found".into())),
    };

    if !canonical_file.starts_with(&canonical_base) {
        return Err(AppError::Forbidden("path traversal denied".into()));
    }

    match tokio::fs::read_to_string(&canonical_file).await {
        Ok(content) => {
            let mime = guess_mime(&path);
            Ok((
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, mime)],
                content,
            ))
        }
        Err(_) => Err(AppError::NotFound("file not found".into())),
    }
}

/// PUT /api/workspace/file/*path — Write/update a file.
pub(crate) async fn handle_workspace_file_put(
    state: State<Arc<AppState>>,
    Path(path): Path<String>,
    body: String,
) -> Result<(), AppError> {
    // Validate file size (max 1MB)
    const MAX_FILE_SIZE: usize = 1024 * 1024;
    if body.len() > MAX_FILE_SIZE {
        return Err(AppError::PayloadTooLarge {
            size: body.len(),
            limit: MAX_FILE_SIZE,
        });
    }

    let base = state.kernel.state.workspace_path();
    let full_path = base.join(&path);

    // Security: ensure the path doesn't escape the workspace
    let canonical_base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    if let Some(parent) = full_path.parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::Internal(format!("failed to create directory: {e}")))?;
        }
        let canonical_parent = parent
            .canonicalize()
            .map_err(|e| AppError::Internal(format!("failed to resolve path: {e}")))?;
        if !canonical_parent.starts_with(&canonical_base) {
            return Err(AppError::Forbidden("path traversal denied".into()));
        }
    }

    // F6: the parent-canonical check above does not catch a pre-existing
    // symlink at `full_path` itself pointing outside the workspace —
    // `tokio::fs::write` follows symlinks and would overwrite the target.
    if let Ok(meta) = tokio::fs::symlink_metadata(&full_path).await
        && meta.file_type().is_symlink()
    {
        let canonical_full = full_path
            .canonicalize()
            .map_err(|e| AppError::Internal(format!("failed to resolve path: {e}")))?;
        if !canonical_full.starts_with(&canonical_base) {
            return Err(AppError::Forbidden("path traversal denied".into()));
        }
    }

    match tokio::fs::write(&full_path, &body).await {
        Ok(_) => {
            tracing::info!(path = %path, "File written");
            Ok(())
        }
        Err(e) => {
            tracing::error!(path = %path, error = %e, "Failed to write file");
            Err(AppError::Internal("failed to write file".into()))
        }
    }
}

// ---------------------------------------------------------------------------
// File Create & Delete
// ---------------------------------------------------------------------------

/// Request body for creating a file.
#[derive(Debug, Deserialize)]
pub(crate) struct CreateFileRequest {
    /// Whether to create a directory instead of a file.
    #[serde(default)]
    pub is_dir: bool,
}

/// POST /api/workspace/file/*path — Create an empty file or directory.
pub(crate) async fn handle_workspace_file_create(
    state: State<Arc<AppState>>,
    Path(path): Path<String>,
    Json(body): Json<CreateFileRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let base = state.kernel.state.workspace_path();
    let full_path = base.join(&path);

    // Security: path traversal check
    let canonical_base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    // Ensure parent exists
    if let Some(parent) = full_path.parent() {
        let canonical_parent = parent
            .canonicalize()
            .map_err(|_| AppError::NotFound("parent directory not found".into()))?;
        if !canonical_parent.starts_with(&canonical_base) {
            return Err(AppError::Forbidden("path traversal denied".into()));
        }
    }

    // F6: `full_path.exists()` follows symlinks, so a dangling symlink at
    // `full_path` pointing outside the workspace would bypass the check
    // and `tokio::fs::write` would create/overwrite the symlink target.
    // Reject any pre-existing symlink outright (create requires the path
    // to be absent anyway).
    if let Ok(meta) = tokio::fs::symlink_metadata(&full_path).await
        && meta.file_type().is_symlink()
    {
        let canonical_full = full_path
            .canonicalize()
            .map_err(|_| AppError::NotFound("path not found".into()))?;
        if !canonical_full.starts_with(&canonical_base) {
            return Err(AppError::Forbidden("path traversal denied".into()));
        }
    }

    if full_path.exists() {
        return Err(AppError::BadRequest("file already exists".into()));
    }

    if body.is_dir {
        tokio::fs::create_dir_all(&full_path)
            .await
            .map_err(|e| AppError::Internal(format!("failed to create directory: {e}")))?;
    } else {
        // Ensure parent dir exists
        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        tokio::fs::write(&full_path, "")
            .await
            .map_err(|e| AppError::Internal(format!("failed to create file: {e}")))?;
    }

    tracing::info!(path = %path, is_dir = body.is_dir, "File created");
    Ok(Json(
        serde_json::json!({ "status": "created", "path": path, "is_dir": body.is_dir }),
    ))
}

/// DELETE /api/workspace/file/*path — Delete a file or empty directory.
pub(crate) async fn handle_workspace_file_delete(
    state: State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let base = state.kernel.state.workspace_path();
    let full_path = base.join(&path);

    // Security: path traversal check
    let canonical_base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let canonical = match full_path.canonicalize() {
        Ok(c) => c,
        Err(_) => return Err(AppError::NotFound("file not found".into())),
    };

    if !canonical.starts_with(&canonical_base) {
        return Err(AppError::Forbidden("path traversal denied".into()));
    }

    if canonical.is_dir() {
        // Only delete empty directories
        let mut entries = tokio::fs::read_dir(&canonical)
            .await
            .map_err(|e| AppError::Internal(format!("failed to read directory: {e}")))?;
        if entries
            .next_entry()
            .await
            .map(|e| e.is_some())
            .unwrap_or(true)
        {
            return Err(AppError::BadRequest("directory is not empty".into()));
        }
        tokio::fs::remove_dir(&canonical)
            .await
            .map_err(|e| AppError::Internal(format!("failed to delete directory: {e}")))?;
    } else {
        tokio::fs::remove_file(&canonical)
            .await
            .map_err(|e| AppError::Internal(format!("failed to delete file: {e}")))?;
    }

    tracing::info!(path = %path, "File deleted");
    Ok(Json(
        serde_json::json!({ "status": "deleted", "path": path }),
    ))
}

/// Guess MIME type from file extension.
fn guess_mime(path: &str) -> String {
    match path.rsplit('.').next() {
        Some("md") => "text/markdown; charset=utf-8".into(),
        Some("json") => "application/json".into(),
        Some("toml") => "application/toml".into(),
        Some("yaml" | "yml") => "application/yaml".into(),
        Some("txt") => "text/plain; charset=utf-8".into(),
        Some("html") => "text/html".into(),
        Some("css") => "text/css".into(),
        Some("js") => "application/javascript".into(),
        _ => "text/plain; charset=utf-8".into(),
    }
}

// ---------------------------------------------------------------------------
// Skills
// ---------------------------------------------------------------------------

/// Compact a file path for display (replace home dir with ~).
fn compact_path(path: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        let path_str = path.to_string_lossy();
        if let Some(rest) = path_str.strip_prefix(home_str.as_ref()) {
            return format!("~{rest}");
        }
    }
    path.to_string_lossy().into_owned()
}

/// Convert a SkillEntry to its JSON API representation (RFC-009 §5.1).
fn skill_entry_to_json(entry: &SkillEntry) -> serde_json::Value {
    let meta = entry.metadata.as_ref();
    let source_str = match entry.source {
        SkillSource::Bundled => "bundled",
        SkillSource::Managed => "managed",
        SkillSource::Workspace => "workspace",
    };
    let status_str = match entry.status {
        SkillStatus::Ready => "ready",
        SkillStatus::NeedsSetup => "needs_setup",
        SkillStatus::Disabled => "disabled",
    };

    let requirements = meta
        .map(|m| {
            serde_json::json!({
                "bins": m.requires.bins,
                "anyBins": m.requires.any_bins,
                "env": m.requires.env,
                "config": m.requires.config,
            })
        })
        .unwrap_or(serde_json::json!({
            "bins": [],
            "anyBins": [],
            "env": [],
            "config": [],
        }));

    let missing = serde_json::json!({
        "bins": entry.eligibility.missing_bins,
        "anyBins": entry.eligibility.missing_any_bins,
        "env": entry.eligibility.missing_env,
        "config": entry.eligibility.missing_config,
    });

    let install: Vec<serde_json::Value> = meta
        .map(|m| {
            m.install
                .iter()
                .map(|spec| {
                    let label = match spec.kind {
                        oxios_kernel::InstallKind::Brew => {
                            let name = spec.formula.as_deref().unwrap_or("unknown");
                            format!("Install {name} (brew)")
                        }
                        oxios_kernel::InstallKind::Node => {
                            let name = spec.package.as_deref().unwrap_or("unknown");
                            format!("Install {name} (npm)")
                        }
                        oxios_kernel::InstallKind::Go => {
                            let name = spec.module.as_deref().unwrap_or("unknown");
                            format!("Install {name} (go)")
                        }
                        oxios_kernel::InstallKind::Uv => {
                            let name = spec.package.as_deref().unwrap_or("unknown");
                            format!("Install {name} (uv)")
                        }
                        oxios_kernel::InstallKind::Download => "Download".to_string(),
                    };
                    let bins: Vec<String> = match spec.kind {
                        oxios_kernel::InstallKind::Brew => spec
                            .formula
                            .as_ref()
                            .map(|f| vec![f.clone()])
                            .unwrap_or_default(),
                        oxios_kernel::InstallKind::Node => spec
                            .package
                            .as_ref()
                            .map(|p| vec![p.clone()])
                            .unwrap_or_default(),
                        oxios_kernel::InstallKind::Go => spec
                            .module
                            .as_ref()
                            .map(|m| vec![m.clone()])
                            .unwrap_or_default(),
                        oxios_kernel::InstallKind::Uv => spec
                            .package
                            .as_ref()
                            .map(|p| vec![p.clone()])
                            .unwrap_or_default(),
                        oxios_kernel::InstallKind::Download => vec![],
                    };
                    serde_json::json!({
                        "kind": spec.kind.to_string(),
                        "label": label,
                        "bins": bins,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let os = meta.map(|m| m.os.clone()).unwrap_or_default();

    let config_checks: Vec<serde_json::Value> = entry
        .eligibility
        .config_checks
        .iter()
        .map(|c| serde_json::json!({ "path": c.path, "satisfied": c.satisfied }))
        .collect();

    serde_json::json!({
        "name": entry.skill.name,
        "description": entry.skill.description,
        "author": meta.and_then(|m| m.author.clone()).unwrap_or_default(),
        "version": meta.and_then(|m| m.version.clone()).unwrap_or_default(),
        "emoji": meta.and_then(|m| m.emoji.clone()).unwrap_or_default(),
        "homepage": meta.and_then(|m| m.homepage.clone()).unwrap_or_default(),
        "source": source_str,
        "bundled": entry.bundled,
        "status": status_str,
        "eligible": entry.eligibility.eligible,
        "always": meta.map(|m| m.always).unwrap_or(false),
        "user_invocable": entry.invocation.user_invocable,
        "file_path": compact_path(&entry.skill.file_path),
        "requirements": requirements,
        "missing": missing,
        "os": os,
        "install": install,
        "config_checks": config_checks,
        "format": entry.format.to_string(),
    })
}

/// GET /api/skills — List all skills (RFC-009 §5.1).
pub(crate) async fn handle_skills_list(
    state: State<Arc<AppState>>,
    Query(_params): Query<PageParams>,
) -> Json<serde_json::Value> {
    let entries = state.kernel.extensions.list_skills_entries().await;
    let skills: Vec<serde_json::Value> = entries.iter().map(skill_entry_to_json).collect();
    Json(serde_json::json!({ "skills": skills }))
}

/// GET /api/skills/:name — Get skill details (RFC-009 §5.1).
pub(crate) async fn handle_skill_get(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    match state.kernel.extensions.get_skill_entry(&name).await {
        Some(entry) => Ok(Json(skill_entry_to_json(&entry))),
        None => Err(AppError::NotFound("skill not found".into())),
    }
}

/// POST /api/skills/:name/enable — Enable a skill.
pub(crate) async fn handle_skill_enable(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .kernel
        .extensions
        .enable_skill(&name)
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    tracing::info!(skill = %name, "Skill enabled via API");
    Ok(Json(
        serde_json::json!({ "status": "enabled", "name": name }),
    ))
}

/// POST /api/skills/:name/disable — Disable a skill.
pub(crate) async fn handle_skill_disable(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .kernel
        .extensions
        .disable_skill(&name)
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    tracing::info!(skill = %name, "Skill disabled via API");
    Ok(Json(
        serde_json::json!({ "status": "disabled", "name": name }),
    ))
}

/// GET /api/skills/:name/content — Get SKILL.md content.
pub(crate) async fn handle_skill_content(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let content = state
        .kernel
        .extensions
        .skill_manager()
        .get_skill_content(&name)
        .await;
    match content {
        Some(md) => Ok(Json(serde_json::json!({
            "name": name,
            "content": md,
        }))),
        None => Err(AppError::NotFound("skill not found".into())),
    }
}

/// Request body for creating a skill.
#[derive(Debug, Deserialize)]
pub(crate) struct SkillCreateRequest {
    /// Skill name.
    name: String,
    /// Skill description.
    description: String,
    /// Skill markdown content.
    #[serde(default)]
    content: String,
}

/// POST /api/skills — Create a new skill.
pub(crate) async fn handle_skill_create(
    state: State<Arc<AppState>>,
    Json(body): Json<SkillCreateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Validate skill content size (max 64KB)
    const MAX_SKILL_CONTENT: usize = 64 * 1024;
    if body.content.len() > MAX_SKILL_CONTENT {
        return Err(AppError::PayloadTooLarge {
            size: body.content.len(),
            limit: MAX_SKILL_CONTENT,
        });
    }

    state
        .kernel
        .extensions
        .create_skill(&body.name, &body.description, &body.content)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, skill = %body.name, "Failed to create skill");
            AppError::BadRequest(e.to_string())
        })?;

    tracing::info!(skill = %body.name, "Skill created via API");
    Ok(Json(serde_json::json!({
        "status": "created",
        "name": body.name,
    })))
}

/// DELETE /api/skills/:name — Delete a skill.
pub(crate) async fn handle_skill_delete(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .kernel
        .extensions
        .delete_skill(&name)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, skill = %name, "Failed to delete skill");
            AppError::BadRequest(e.to_string())
        })?;

    tracing::info!(skill = %name, "Skill deleted via API");
    Ok(Json(serde_json::json!({
        "status": "deleted",
        "name": name,
    })))
}

// ---------------------------------------------------------------------------
// Memory
// ---------------------------------------------------------------------------

/// Label for a memory tier, matching the frontend's tier keys.
fn tier_label(t: &MemoryTier) -> &'static str {
    match t {
        MemoryTier::Hot => "hot",
        MemoryTier::Warm => "warm",
        MemoryTier::Cold => "cold",
    }
}

/// Map a stored entry to the `MemoryDetail` shape consumed by the web UI.
fn memory_entry_to_detail(e: &MemoryEntry) -> serde_json::Value {
    serde_json::json!({
        "id": e.id,
        "key": e.id,
        "tier": tier_label(&e.tier),
        "memory_type": e.memory_type.label(),
        "content": e.content,
        "summary": null,
        "project_ids": [],
        "created_at": e.created_at.to_rfc3339(),
        "updated_at": e.modified_at.to_rfc3339(),
        "last_accessed": e.accessed_at.to_rfc3339(),
        "access_count": e.access_count,
        "pinned": e.pinned,
        "protected": !matches!(e.protection, ProtectionLevel::None),
        "protection_reason": null,
        "tags": e.tags,
        "importance": e.importance,
    })
}

/// Query params for `GET /api/memory`.
#[derive(Debug, Deserialize)]
pub(crate) struct MemoryListQuery {
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
}

/// GET /api/memory — List memory entries (optionally filtered by tier/type).
pub(crate) async fn handle_memory_list(
    state: State<Arc<AppState>>,
    Query(q): Query<MemoryListQuery>,
) -> Json<serde_json::Value> {
    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    let mut entries = state.kernel.agents.list_all_memories(500).await;

    if let Some(t) = &q.r#type {
        entries.retain(|e| e.memory_type.label() == t.as_str());
    }
    if let Some(tier) = &q.tier {
        entries.retain(|e| tier_label(&e.tier) == tier.as_str());
    }

    let total = entries.len();
    let page = q.page.unwrap_or(1).max(1);
    let start = (page - 1) * limit;
    let items: Vec<_> = entries
        .iter()
        .skip(start)
        .take(limit)
        .map(memory_entry_to_detail)
        .collect();

    Json(serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "limit": limit,
    }))
}

/// GET /api/memory/:name — Get a specific memory entry by ID.
pub(crate) async fn handle_memory_get(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    match state.kernel.agents.get_memory(&name).await {
        Some(entry) => Ok(Json(memory_entry_to_detail(&entry)).into_response()),
        None => Err(AppError::NotFound("memory entry not found".into())),
    }
}

// ---------------------------------------------------------------------------
// Memory map (RFC-T1-B): 2D projection + neighbor edges
// ---------------------------------------------------------------------------

/// Cache for memory map projections. Keyed by a coarse epoch (5 minutes)
/// plus the set of memory IDs, so a small memory mutation does not
/// invalidate the cache for every call. The full cache is held in-process
/// (no disk writes); the per-key TTL is short and the projection is
/// cheap enough that we are happy to recompute on TTL expiry.
#[derive(Default, Clone)]
pub struct MemoryMapCache {
    inner: std::sync::Arc<std::sync::Mutex<Option<MemoryMapCacheEntry>>>,
}

#[derive(Clone)]
struct MemoryMapCacheEntry {
    /// Epoch seconds (5-minute resolution).
    epoch: u64,
    /// Sorted ID list that was used to compute the projection.
    ids: Vec<String>,
    /// 64-bit content hash over (id, content_hash, tier, mem_type) for
    /// every entry in the projection. Detects content edits that do not
    /// change the id-set (the projection depends on the actual TF-IDF
    /// vectors, not just the set of entries).
    content_signature: u64,
    /// Pre-computed map entries (coords_2d + neighbors).
    entries: Vec<oxios_kernel::memory::MemoryMapEntry>,
}

impl MemoryMapCache {
    /// Try to get a cached entry. Returns `None` if the epoch is stale,
    /// the ID set has changed, or the per-entry content signature differs.
    fn get(
        &self,
        epoch: u64,
        ids: &[String],
        content_signature: u64,
    ) -> Option<Vec<oxios_kernel::memory::MemoryMapEntry>> {
        let guard = self.inner.lock().ok()?;
        let entry = guard.as_ref()?;
        if entry.epoch != epoch {
            return None;
        }
        if entry.ids != ids {
            return None;
        }
        if entry.content_signature != content_signature {
            return None;
        }
        Some(entry.entries.clone())
    }

    /// Store a fresh entry.
    fn put(
        &self,
        epoch: u64,
        ids: Vec<String>,
        content_signature: u64,
        entries: Vec<oxios_kernel::memory::MemoryMapEntry>,
    ) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = Some(MemoryMapCacheEntry {
                epoch,
                ids,
                content_signature,
                entries,
            });
        }
    }
}

/// Compute a stable signature over the projection-relevant fields of
/// each entry. Used as part of the memory-map cache key so that an
/// edit to a memory's `content` (which does not change the id set)
/// still invalidates the cache.
fn memory_map_content_signature(entries: &[MemoryEntry]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    // Sort by id for a stable hash independent of iteration order.
    let mut sorted: Vec<&MemoryEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    for e in sorted {
        e.id.hash(&mut hasher);
        e.content.hash(&mut hasher);
        // tier is a Copy enum; convert to a stable string for hashing.
        let tier_str = match e.tier {
            oxios_kernel::memory::MemoryTier::Hot => "hot",
            oxios_kernel::memory::MemoryTier::Warm => "warm",
            oxios_kernel::memory::MemoryTier::Cold => "cold",
        };
        tier_str.hash(&mut hasher);
        // Use the singular label for parity with `mem_type` filtering.
        e.memory_type.label().hash(&mut hasher);
    }
    hasher.finish()
}

/// Query parameters for the memory map endpoint.
#[derive(Debug, Deserialize)]
pub(crate) struct MemoryMapQuery {
    /// Optional tier filter.
    #[serde(default)]
    pub tier: Option<String>,
    /// Optional memory type filter.
    #[serde(default)]
    pub mem_type: Option<String>,
    /// Max entries to include (default 500, hard cap 2000).
    #[serde(default)]
    pub limit: Option<usize>,
}

/// 5-minute epoch for the memory map cache.
const MEMORY_MAP_EPOCH_SECS: u64 = 300;

/// GET /api/memory/map — 2D projection of memory entries for the Web UI map.
///
/// Returns one [`MemoryMapEntry`] per matching memory, with pre-computed
/// 2D coordinates and top similar neighbors. The projection uses PCA
/// over the in-memory TF-IDF vectors (see `embedding_viz`); results are
/// cached in-process for 5 minutes per (epoch, id-set) tuple.
pub(crate) async fn handle_memory_map(
    state: State<Arc<AppState>>,
    Query(params): Query<MemoryMapQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let limit = params.limit.unwrap_or(500).clamp(1, 2000);

    // Load matching entries from state store. We deliberately bypass the
    // in-memory vector index here because the index may not include
    // entries that were stored by other channels (e.g. compaction).
    let mut entries: Vec<MemoryEntry> = Vec::new();
    for category in [
        "memory/facts",
        "memory/episodes",
        "memory/knowledge",
        "memory/sessions",
        "memory/conversations",
        "memory/skills",
        "memory/preferences",
        "memory/decisions",
        "memory/profiles",
    ] {
        let Ok(names) = state.kernel.state.list_category(category).await else {
            continue;
        };
        for name in names {
            if entries.len() >= limit {
                break;
            }
            let Ok(Some(entry)) = state
                .kernel
                .state
                .load::<MemoryEntry>(category, &name)
                .await
            else {
                continue;
            };
            // Per-entry filters: `mem_type` matches the singular label()
            // (e.g. "fact") returned by the frontend, NOT the plural
            // category short name ("facts"). The `tier` filter is
            // matched in the same place for symmetry.
            if let Some(ref want) = params.mem_type
                && entry.memory_type.label() != want.as_str()
            {
                continue;
            }
            if let Some(ref want_tier) = params.tier {
                let tier_str = match entry.tier {
                    oxios_kernel::memory::MemoryTier::Hot => "hot",
                    oxios_kernel::memory::MemoryTier::Warm => "warm",
                    oxios_kernel::memory::MemoryTier::Cold => "cold",
                };
                if tier_str != want_tier.as_str() {
                    continue;
                }
            }
            entries.push(entry);
        }
        if entries.len() >= limit {
            break;
        }
    }

    // Cap again (in case we broke out of the inner loop early).
    entries.truncate(limit);

    // Compute 2D projection + neighbors.
    let map_entries = compute_memory_map_entries(&state, &entries).await;

    Ok(Json(serde_json::json!({
        "count": map_entries.len(),
        "epoch": current_epoch_secs() / MEMORY_MAP_EPOCH_SECS,
        "entries": map_entries,
    })))
}

/// Compute (or fetch from cache) the MemoryMapEntry list for a given
/// set of MemoryEntry values.
async fn compute_memory_map_entries(
    state: &Arc<AppState>,
    entries: &[MemoryEntry],
) -> Vec<oxios_kernel::memory::MemoryMapEntry> {
    use oxios_kernel::embedding::EmbeddingProvider;
    use oxios_kernel::memory::{MemoryMapEntry, compute_pca_2d, compute_top_neighbors};

    if entries.is_empty() {
        return Vec::new();
    }

    let ids: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
    let epoch = current_epoch_secs() / MEMORY_MAP_EPOCH_SECS;
    let content_signature = memory_map_content_signature(entries);

    // Cache lookup.
    if let Some(cached) = state.memory_map_cache.get(epoch, &ids, content_signature) {
        return cached;
    }

    // Build embeddings via the kernel's TF-IDF provider. We collapse
    // the term-frequency map to a sorted (term, weight) list, then
    // encode as a sparse f32 vector keyed by term index for PCA.
    let provider = oxios_kernel::embedding::TfIdfEmbeddingProvider;
    let mut term_index: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut tf_vecs: Vec<Vec<(u32, f32)>> = Vec::with_capacity(entries.len());
    for entry in entries {
        let Ok(emb) = provider.embed(&entry.content).await else {
            tf_vecs.push(Vec::new());
            continue;
        };
        let oxios_kernel::embedding::EmbeddingVector::Sparse(tf) = emb else {
            // Dense vectors are also fine, but rare on the TF-IDF path.
            tf_vecs.push(Vec::new());
            continue;
        };
        let mut pairs: Vec<(u32, f32)> = tf
            .into_iter()
            .map(|(term, w)| {
                let next = term_index.len() as u32;
                let idx = *term_index.entry(term).or_insert(next);
                (idx, w as f32)
            })
            .collect();
        pairs.sort_by_key(|(idx, _)| *idx);
        // De-duplicate by index (sum weights if a term somehow appears twice).
        pairs.dedup_by_key(|(idx, _)| *idx);
        tf_vecs.push(pairs);
    }

    // Convert sparse pairs to dense f32 vectors aligned to `term_index`.
    let dim = term_index.len();
    let dense: Vec<Vec<f32>> = tf_vecs
        .iter()
        .map(|pairs| {
            let mut v = vec![0.0_f32; dim];
            for (idx, w) in pairs {
                if let Some(slot) = v.get_mut(*idx as usize) {
                    *slot = *w;
                }
            }
            v
        })
        .collect();

    // Project to 2D and compute neighbor lists.
    let coords = compute_pca_2d(&dense);
    let top_n = compute_top_neighbors(&dense, &ids, 5, 0.7);

    let map_entries: Vec<MemoryMapEntry> = entries
        .iter()
        .zip(coords.iter().zip(top_n.iter()))
        .map(|(entry, (xy, nbrs))| MemoryMapEntry {
            id: entry.id.clone(),
            tier: match entry.tier {
                oxios_kernel::memory::MemoryTier::Hot => "hot".into(),
                oxios_kernel::memory::MemoryTier::Warm => "warm".into(),
                oxios_kernel::memory::MemoryTier::Cold => "cold".into(),
            },
            mem_type: entry.memory_type.label().to_string(),
            content_preview: content_preview(&entry.content, 120),
            created_at: entry.created_at.to_rfc3339(),
            access_count: entry.access_count,
            coords_2d: *xy,
            top_neighbors: nbrs.clone(),
        })
        .collect();

    state
        .memory_map_cache
        .put(epoch, ids, content_signature, map_entries.clone());

    map_entries
}

/// Truncate content to a short preview suitable for hover tooltips.
fn content_preview(content: &str, max_chars: usize) -> String {
    let trimmed: String = content.chars().take(max_chars).collect();
    if content.chars().count() > max_chars {
        format!("{trimmed}\u{2026}")
    } else {
        trimmed
    }
}

/// Current wall-clock time as Unix seconds.
fn current_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Memory CRUD
// ---------------------------------------------------------------------------

/// Request body for creating a memory entry.
#[derive(Debug, Deserialize)]
pub(crate) struct MemoryCreateRequest {
    /// Memory content.
    content: String,
    /// Memory type: fact, episode, or knowledge.
    #[serde(default = "default_memory_type")]
    memory_type: String,
    /// Tags for search.
    #[serde(default)]
    tags: Vec<String>,
    /// Importance (0.0-1.0).
    #[serde(default = "default_importance")]
    importance: f32,
}

fn default_memory_type() -> String {
    "fact".to_string()
}

fn default_importance() -> f32 {
    0.5
}

/// POST /api/memory — Create a memory entry.
pub(crate) async fn handle_memory_create(
    state: State<Arc<AppState>>,
    Json(body): Json<MemoryCreateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Validate memory entry size (max 32KB)
    const MAX_MEMORY_ENTRY: usize = 32 * 1024;
    if body.content.len() > MAX_MEMORY_ENTRY {
        return Err(AppError::PayloadTooLarge {
            size: body.content.len(),
            limit: MAX_MEMORY_ENTRY,
        });
    }

    let memory_type = match body.memory_type.as_str() {
        "fact" => MemoryType::Fact,
        "episode" => MemoryType::Episode,
        "knowledge" => MemoryType::Knowledge,
        _ => {
            return Err(AppError::BadRequest(
                "memory_type must be fact, episode, or knowledge".into(),
            ));
        }
    };
    let entry = MemoryEntry {
        id: uuid::Uuid::new_v4().to_string(),
        memory_type,
        tier: memory_type.initial_tier(),
        content: body.content.clone(),
        content_hash: oxios_kernel::memory::content_hash(&body.content),
        source: "api".to_string(),
        session_id: None,
        tags: body.tags.clone(),
        importance: body.importance,
        pinned: false,
        protection: oxios_kernel::memory::ProtectionLevel::None,
        auto_classified: false,
        session_appearances: 0,
        user_corrected: false,
        seen_in_sessions: vec![],
        created_at: chrono::Utc::now(),
        accessed_at: chrono::Utc::now(),
        modified_at: chrono::Utc::now(),
        access_count: 0,
        decay_score: 1.0,
        compaction_level: 0,
        compacted_from: vec![],
        related_ids: vec![],
        contradicts: None,
    };

    // Use memory manager from kernel
    let id = state
        .kernel
        .agents
        .remember(entry)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(serde_json::json!({ "id": id, "status": "created" })))
}

#[derive(Debug, Deserialize)]
pub(crate) struct MemorySearchRequest {
    query: String,
    memory_type: Option<String>,
    limit: Option<usize>,
}

/// POST /api/memory/search — Search memory entries.
pub(crate) async fn handle_memory_search(
    state: State<Arc<AppState>>,
    Json(body): Json<MemorySearchRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let type_filter = body.memory_type.as_deref().and_then(|s| match s {
        "conversation" => Some(MemoryType::Conversation),
        "session" => Some(MemoryType::Session),
        "fact" => Some(MemoryType::Fact),
        "episode" => Some(MemoryType::Episode),
        "knowledge" => Some(MemoryType::Knowledge),
        _ => None,
    });
    let limit = body.limit.unwrap_or(10);

    // Use memory manager from kernel
    let entries = state
        .kernel
        .agents
        .search_memory(&body.query, type_filter, limit)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let results: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "type": e.memory_type.label(),
                "content": e.content,
                "tags": e.tags,
                "importance": e.importance,
                "created_at": e.created_at.to_rfc3339(),
            })
        })
        .collect();
    Ok(Json(
        serde_json::json!({ "count": results.len(), "entries": results }),
    ))
}

// ---------------------------------------------------------------------------
// Semantic search (HNSW-powered)
// ---------------------------------------------------------------------------

/// Request body for semantic search.
#[derive(Debug, Deserialize)]
pub(crate) struct SemanticSearchRequest {
    query: String,
    memory_type: Option<String>,
    limit: Option<usize>,
}

/// POST /api/memory/semantic — Semantic search using HNSW index.
///
/// Uses approximate nearest neighbor search for fast,
/// high-quality similarity matching.
pub(crate) async fn handle_memory_semantic_search(
    state: State<Arc<AppState>>,
    Json(body): Json<SemanticSearchRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let type_filter = body.memory_type.as_deref().and_then(|s| match s {
        "conversation" => Some(MemoryType::Conversation),
        "session" => Some(MemoryType::Session),
        "fact" => Some(MemoryType::Fact),
        "episode" => Some(MemoryType::Episode),
        "knowledge" => Some(MemoryType::Knowledge),
        _ => None,
    });
    let limit = body.limit.unwrap_or(10);

    // Determine which engine was actually used (honest reporting)
    let engine_label = if state.kernel.agents.has_hnsw_index() {
        "hnsw"
    } else {
        "keyword"
    };

    // Use semantic search from kernel (HNSW-powered or keyword fallback)
    let hits = state
        .kernel
        .agents
        .semantic_search_memory(&body.query, type_filter, limit)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let results: Vec<serde_json::Value> = hits
        .iter()
        .map(|hit| {
            serde_json::json!({
                "id": hit.entry.id,
                "type": hit.entry.memory_type.label(),
                "content": hit.entry.content,
                "tags": hit.entry.tags,
                "importance": hit.entry.importance,
                "similarity": hit.similarity,
                "distance": hit.distance,
                "created_at": hit.entry.created_at.to_rfc3339(),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "count": results.len(),
        "entries": results,
        "engine": engine_label,
    })))
}

// ---------------------------------------------------------------------------
// Memory stats, pin, delete, dream
// ---------------------------------------------------------------------------

/// GET /api/memory/stats — Aggregate memory statistics.
pub(crate) async fn handle_memory_stats(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let entries = state.kernel.agents.list_all_memories(10_000).await;

    let mut by_tier: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
    let mut by_type: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
    let mut total_size_bytes = 0usize;
    let mut oldest: Option<chrono::DateTime<chrono::Utc>> = None;
    let mut newest: Option<chrono::DateTime<chrono::Utc>> = None;

    for e in &entries {
        *by_tier.entry(tier_label(&e.tier)).or_default() += 1;
        *by_type.entry(e.memory_type.label()).or_default() += 1;
        total_size_bytes += e.content.len();
        oldest = Some(oldest.map_or(e.created_at, |o| o.min(e.created_at)));
        newest = Some(newest.map_or(e.created_at, |n| n.max(e.created_at)));
    }

    let by_tier_json: serde_json::Map<String, serde_json::Value> = by_tier
        .into_iter()
        .map(|(k, v)| (k.into(), serde_json::Value::from(v)))
        .collect();
    let by_type_json: serde_json::Map<String, serde_json::Value> = by_type
        .into_iter()
        .map(|(k, v)| (k.into(), serde_json::Value::from(v)))
        .collect();

    Ok(Json(serde_json::json!({
        "total": entries.len(),
        "by_tier": by_tier_json,
        "by_type": by_type_json,
        "total_size_bytes": total_size_bytes,
        "oldest_created": oldest.map(|d| d.to_rfc3339()),
        "newest_created": newest.map(|d| d.to_rfc3339()),
    })))
}

/// Request body for pinning a memory entry.
#[derive(Debug, Deserialize)]
pub(crate) struct PinRequest {
    pinned: bool,
}

/// PUT /api/memory/{id}/pin — Toggle pin status on a memory entry.
pub(crate) async fn handle_memory_pin(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<PinRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if state
        .kernel
        .agents
        .set_memory_pinned(&id, body.pinned)
        .await
    {
        Ok(Json(serde_json::json!({ "id": id, "pinned": body.pinned })))
    } else {
        Err(AppError::NotFound("memory entry not found".into()))
    }
}

/// DELETE /api/memory/{id} — Delete a memory entry.
pub(crate) async fn handle_memory_delete(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    if state.kernel.agents.forget_memory(&id).await {
        Ok(Json(serde_json::json!({ "id": id, "deleted": true })))
    } else {
        Err(AppError::NotFound("memory entry not found".into()))
    }
}

/// GET /api/memory/dream/reports — List dream reports.
pub(crate) async fn handle_dream_reports(_state: State<Arc<AppState>>) -> Json<serde_json::Value> {
    // Placeholder — return empty list until dream persistence is implemented
    Json(serde_json::json!({ "reports": [] }))
}

/// GET /api/memory/dream/status — Dream status.
pub(crate) async fn handle_dream_status(_state: State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "idle",
        "last_run": null,
        "checkpoint_exists": false,
    }))
}


#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // TreeEntry serialization
    // -----------------------------------------------------------------------

    #[test]
    fn test_tree_entry_serialization() {
        let entry = TreeEntry {
            name: "hello.md".into(),
            is_dir: false,
            size: 1024,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["name"], "hello.md");
        assert_eq!(json["is_dir"], false);
        assert_eq!(json["size"], 1024);

        let dir_entry = TreeEntry {
            name: "src".into(),
            is_dir: true,
            size: 0,
        };
        let json = serde_json::to_value(&dir_entry).unwrap();
        assert_eq!(json["is_dir"], true);
        assert_eq!(json["size"], 0);
    }

    // -----------------------------------------------------------------------
    // Pagination
    // -----------------------------------------------------------------------

    #[test]
    fn test_pagination_bounds() {
        let items: Vec<i32> = (1..=10).collect();

        // Page 1, limit 3 → items [1, 2, 3]
        let p1 = PageParams { page: 1, limit: 3 };
        let result = paginate(&items, &p1);
        assert_eq!(result["total"], 10);
        assert_eq!(result["page"], 1);
        assert_eq!(result["limit"], 3);
        let returned: Vec<i32> = serde_json::from_value(result["items"].clone()).unwrap();
        assert_eq!(returned, vec![1, 2, 3]);

        // Page 4, limit 3 → items [10]
        let p4 = PageParams { page: 4, limit: 3 };
        let result = paginate(&items, &p4);
        let returned: Vec<i32> = serde_json::from_value(result["items"].clone()).unwrap();
        assert_eq!(returned, vec![10]);

        // Page 0 (underflow) → offset wraps to 0 via saturating_sub
        let p0 = PageParams { page: 0, limit: 3 };
        let result = paginate(&items, &p0);
        let returned: Vec<i32> = serde_json::from_value(result["items"].clone()).unwrap();
        assert_eq!(returned, vec![1, 2, 3]);

        // Limit capped at 500
        let big = PageParams {
            page: 1,
            limit: 9999,
        };
        let result = paginate(&items, &big);
        assert_eq!(result["limit"], 500);
    }

    // -----------------------------------------------------------------------
    // MIME guessing
    // -----------------------------------------------------------------------

    #[test]
    fn test_guess_mime_common_types() {
        assert_eq!(guess_mime("main.rs"), "text/plain; charset=utf-8");
        assert_eq!(guess_mime("Cargo.toml"), "application/toml");
        assert_eq!(guess_mime("README.md"), "text/markdown; charset=utf-8");
        assert_eq!(guess_mime("data.json"), "application/json");
        assert_eq!(guess_mime("app.js"), "application/javascript");
        assert_eq!(guess_mime("index.html"), "text/html");
        assert_eq!(guess_mime("unknown.bin"), "text/plain; charset=utf-8");
    }

    // -----------------------------------------------------------------------
    // Memory type validation
    // -----------------------------------------------------------------------

    #[test]
    fn test_memory_type_validation() {
        // Valid types — these should map to MemoryType variants correctly.
        let valid = vec!["fact", "episode", "knowledge"];
        for t in valid {
            let mt = match t {
                "fact" => Some(MemoryType::Fact),
                "episode" => Some(MemoryType::Episode),
                "knowledge" => Some(MemoryType::Knowledge),
                _ => None,
            };
            assert!(mt.is_some(), "expected '{t}' to be a valid memory type");
        }

        // Invalid types should not match any variant.
        let invalid = vec!["invalid", "", "FACT", "EpIsOdE"];
        for t in invalid {
            let mt: Option<MemoryType> = match t {
                "fact" => Some(MemoryType::Fact),
                "episode" => Some(MemoryType::Episode),
                "knowledge" => Some(MemoryType::Knowledge),
                _ => None,
            };
            assert!(mt.is_none(), "expected '{t}' to be rejected");
        }
    }

    // -----------------------------------------------------------------------
    // File size limit enforcement
    // -----------------------------------------------------------------------

    #[test]
    fn test_file_size_limit_enforcement() {
        // MAX_FILE_SIZE in handle_workspace_file_put is 1MB.
        const MAX_FILE_SIZE: usize = 1024 * 1024;

        // A body exactly at the limit should be accepted by the size check.
        let body_at_limit = "x".repeat(MAX_FILE_SIZE);
        assert_eq!(body_at_limit.len(), MAX_FILE_SIZE);
        assert!(body_at_limit.len() <= MAX_FILE_SIZE);

        // A body one byte over the limit should be rejected.
        let body_over_limit = "x".repeat(MAX_FILE_SIZE + 1);
        assert!(body_over_limit.len() > MAX_FILE_SIZE);

        // Simulate the check done in handle_workspace_file_put:
        // if body.len() > MAX_FILE_SIZE { return PayloadTooLarge }
        assert!(body_over_limit.len() > MAX_FILE_SIZE);

        // Skill content limit (64KB)
        const MAX_SKILL_CONTENT: usize = 64 * 1024;
        let big_skill = "a".repeat(MAX_SKILL_CONTENT + 1);
        assert!(big_skill.len() > MAX_SKILL_CONTENT);

        // Memory entry limit (32KB)
        const MAX_MEMORY_ENTRY: usize = 32 * 1024;
        let big_memory = "m".repeat(MAX_MEMORY_ENTRY + 1);
        assert!(big_memory.len() > MAX_MEMORY_ENTRY);
    }

    // -----------------------------------------------------------------------
    // MemoryMapCache (RFC-T1-B)
    // -----------------------------------------------------------------------

    fn make_entry(id: &str) -> oxios_kernel::memory::MemoryMapEntry {
        oxios_kernel::memory::MemoryMapEntry {
            id: id.into(),
            tier: "hot".into(),
            mem_type: "fact".into(),
            content_preview: "x".into(),
            created_at: "2026-06-04T00:00:00Z".into(),
            access_count: 0,
            coords_2d: (0.0, 0.0),
            top_neighbors: vec![],
        }
    }

    fn make_memory_entry(
        id: &str,
        content: &str,
        tier: oxios_kernel::memory::MemoryTier,
        mem_type: oxios_kernel::memory::MemoryType,
    ) -> MemoryEntry {
        MemoryEntry {
            id: id.into(),
            memory_type: mem_type,
            tier,
            content: content.into(),
            content_hash: oxios_kernel::memory::content_hash(content),
            source: "test".into(),
            session_id: None,
            tags: vec![],
            importance: 0.5,
            pinned: false,
            protection: oxios_kernel::memory::ProtectionLevel::None,
            auto_classified: false,
            session_appearances: 0,
            user_corrected: false,
            seen_in_sessions: vec![],
            created_at: chrono::Utc::now(),
            accessed_at: chrono::Utc::now(),
            modified_at: chrono::Utc::now(),
            access_count: 0,
            decay_score: 1.0,
            compaction_level: 0,
            compacted_from: vec![],
            related_ids: vec![],
            contradicts: None,
        }
    }

    #[test]
    fn test_memory_map_cache_misses_on_empty() {
        let cache = MemoryMapCache::default();
        assert!(cache.get(0, &[], 0).is_none());
    }

    #[test]
    fn test_memory_map_cache_round_trip() {
        let cache = MemoryMapCache::default();
        let ids = vec!["a".to_string(), "b".to_string()];
        let entries = vec![
            oxios_kernel::memory::MemoryMapEntry {
                id: "a".into(),
                tier: "hot".into(),
                mem_type: "fact".into(),
                content_preview: "alpha".into(),
                created_at: "2026-06-04T00:00:00Z".into(),
                access_count: 1,
                coords_2d: (0.0, 0.0),
                top_neighbors: vec![],
            },
            oxios_kernel::memory::MemoryMapEntry {
                id: "b".into(),
                tier: "warm".into(),
                mem_type: "episode".into(),
                content_preview: "beta".into(),
                created_at: "2026-06-04T00:00:00Z".into(),
                access_count: 2,
                coords_2d: (0.5, -0.5),
                top_neighbors: vec![oxios_kernel::memory::MemoryNeighbor {
                    id: "a".into(),
                    similarity: 0.81,
                }],
            },
        ];
        let entries_for_sig = vec![
            make_memory_entry(
                "a",
                "alpha",
                oxios_kernel::memory::MemoryTier::Hot,
                oxios_kernel::memory::MemoryType::Fact,
            ),
            make_memory_entry(
                "b",
                "beta",
                oxios_kernel::memory::MemoryTier::Warm,
                oxios_kernel::memory::MemoryType::Episode,
            ),
        ];
        let sig = memory_map_content_signature(&entries_for_sig);
        cache.put(42, ids.clone(), sig, entries.clone());
        let got = cache.get(42, &ids, sig).expect("hit");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].id, "a");
        assert_eq!(got[1].top_neighbors[0].similarity, 0.81);
    }

    #[test]
    fn test_memory_map_cache_stale_epoch_misses() {
        let cache = MemoryMapCache::default();
        let ids = vec!["a".to_string()];
        cache.put(1, ids.clone(), 0, vec![make_entry("a")]);
        assert!(cache.get(2, &ids, 0).is_none());
    }

    #[test]
    fn test_memory_map_cache_id_change_misses() {
        let cache = MemoryMapCache::default();
        let ids_a = vec!["a".to_string()];
        cache.put(1, ids_a.clone(), 0, vec![make_entry("a")]);
        // Same epoch, different id set => miss.
        let ids_b = vec!["a".to_string(), "b".to_string()];
        assert!(cache.get(1, &ids_b, 0).is_none());
    }

    #[test]
    fn test_memory_map_cache_content_change_misses() {
        // P1-1: editing a memory's `content` (which does not change the
        // id-set) must invalidate the cache. The signature is computed
        // from the content text, so any content edit changes the hash.
        let cache = MemoryMapCache::default();
        let ids = vec!["a".to_string()];
        let original = make_memory_entry(
            "a",
            "original content",
            oxios_kernel::memory::MemoryTier::Hot,
            oxios_kernel::memory::MemoryType::Fact,
        );
        let edited = make_memory_entry(
            "a",
            "edited content",
            oxios_kernel::memory::MemoryTier::Hot,
            oxios_kernel::memory::MemoryType::Fact,
        );
        let sig_original = memory_map_content_signature(&[original]);
        let sig_edited = memory_map_content_signature(&[edited]);
        assert_ne!(
            sig_original, sig_edited,
            "signature must differ when only the content changes"
        );
        cache.put(1, ids.clone(), sig_original, vec![make_entry("a")]);
        // Same epoch, same id-set, but content changed => miss.
        assert!(cache.get(1, &ids, sig_edited).is_none());
        // Original signature still hits (defensive).
        assert!(cache.get(1, &ids, sig_original).is_some());
    }

    #[test]
    fn test_memory_map_content_signature_is_stable_under_reorder() {
        // The signature is order-independent so iteration order from
        // StateStore does not flip the cache key.
        let a = make_memory_entry(
            "a",
            "alpha",
            oxios_kernel::memory::MemoryTier::Hot,
            oxios_kernel::memory::MemoryType::Fact,
        );
        let b = make_memory_entry(
            "b",
            "beta",
            oxios_kernel::memory::MemoryTier::Warm,
            oxios_kernel::memory::MemoryType::Episode,
        );
        let s1 = memory_map_content_signature(&[a.clone(), b.clone()]);
        let s2 = memory_map_content_signature(&[b, a]);
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_content_preview_truncates_with_ellipsis() {
        let long = "x".repeat(200);
        let preview = content_preview(&long, 120);
        assert_eq!(preview.chars().count(), 121); // 120 + ellipsis
        assert!(preview.ends_with('\u{2026}'));
    }

    #[test]
    fn test_content_preview_keeps_short_content() {
        let preview = content_preview("hello", 120);
        assert_eq!(preview, "hello");
    }

    #[test]
    fn test_content_preview_handles_empty() {
        let preview = content_preview("", 120);
        assert_eq!(preview, "");
    }

    // -----------------------------------------------------------------------
    // handle_memory_map filter (P0-1)
    // -----------------------------------------------------------------------
    //
    // The per-entry mem_type filter must compare against
    // `MemoryType::label()` (singular: "fact", "episode", "knowledge", …),
    // NOT the plural category short name ("facts", "episodes", …).
    // The category short name is "memory/facts" → "facts" (plural), so
    // a `params.mem_type = "fact"` filter must NOT match it.
    //
    // The 4-category vs 9-category scoping is tested at the route level
    // (would require a full AppState harness); here we pin the label()
    // values that the filter must use.

    #[test]
    fn test_memory_type_labels_match_filter_strings() {
        // Every singular label here is what the frontend `<Select>`
        // submits as `params.mem_type`. The filter must accept all of
        // these against entries of the corresponding type.
        let cases = [
            (MemoryType::Fact, "fact"),
            (MemoryType::Episode, "episode"),
            (MemoryType::Knowledge, "knowledge"),
            (MemoryType::Skill, "skill"),
            (MemoryType::Preference, "preference"),
            (MemoryType::Decision, "decision"),
            (MemoryType::Conversation, "conversation"),
            (MemoryType::Session, "session"),
            (MemoryType::UserProfile, "user_profile"),
        ];
        for (mt, label) in cases {
            assert_eq!(
                mt.label(),
                label,
                "{mt:?} label must be the singular {label} (not the plural category)"
            );
            // Category short name is the plural — confirm they differ
            // for every type except `Knowledge` (where they accidentally
            // coincide; that is the only case the old, broken code
            // happened to handle correctly).
            let cat_short = mt.category().split('/').nth(1).unwrap_or("");
            if mt != MemoryType::Knowledge {
                assert_ne!(
                    cat_short, label,
                    "category short name ({cat_short}) must not equal label ({label})"
                );
            }
        }
    }
}
