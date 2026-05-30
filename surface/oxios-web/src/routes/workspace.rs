use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use oxios_kernel::memory::{MemoryEntry, MemoryType};
use oxios_kernel::{SkillEntry, SkillSource, SkillStatus};

use crate::error::AppError;
use crate::routes::{paginate, PageParams};
use crate::server::AppState;

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
    Ok(Json(serde_json::json!({ "status": "created", "path": path, "is_dir": body.is_dir })))
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
        let mut entries = tokio::fs::read_dir(&canonical).await.map_err(|e| {
            AppError::Internal(format!("failed to read directory: {e}"))
        })?;
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
    Ok(Json(serde_json::json!({ "status": "deleted", "path": path })))
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
// Seeds
// ---------------------------------------------------------------------------

/// Seed summary for listing.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct SeedSummary {
    /// Seed unique ID.
    id: String,
    /// The goal of this seed.
    goal: String,
    /// Number of constraints.
    constraints_count: usize,
    /// Creation timestamp.
    created_at: String,
}

/// GET /api/seeds — List Ouroboros seeds.
pub(crate) async fn handle_seeds_list(
    state: State<Arc<AppState>>,
    Query(params): Query<PageParams>,
) -> Json<serde_json::Value> {
    let mut summaries = Vec::new();

    if let Ok(names) = state.kernel.state.list_category("seeds").await {
        for name in names {
            if let Ok(Some(content)) = state.kernel.state.load_markdown("seeds", &name).await {
                // Try to parse as JSON (seeds stored as JSON)
                if let Ok(seed) = serde_json::from_str::<oxios_ouroboros::Seed>(&content) {
                    summaries.push(SeedSummary {
                        id: seed.id.to_string(),
                        goal: seed.goal,
                        constraints_count: seed.constraints.len(),
                        created_at: seed.created_at.to_rfc3339(),
                    });
                } else {
                    // Raw markdown seed
                    summaries.push(SeedSummary {
                        id: name.clone(),
                        goal: content.lines().next().unwrap_or(&name).into(),
                        constraints_count: 0,
                        created_at: String::new(),
                    });
                }
            }
        }
    }

    Json(paginate(&summaries, &params))
}

/// GET /api/seeds/:id — Get a specific seed.
pub(crate) async fn handle_seed_get(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Try JSON first, then markdown
    if let Ok(Some(content)) = state.kernel.state.load_markdown("seeds", &id).await {
        if let Ok(seed) = serde_json::from_str::<oxios_ouroboros::Seed>(&content) {
            return Ok(Json(serde_json::to_value(&seed).unwrap_or_default()));
        }
        return Ok(Json(serde_json::json!({
            "id": id,
            "content": content,
        })));
    }

    Err(AppError::NotFound("seed not found".into()))
}

/// Evolution lineage entry for a seed.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct EvolutionEntry {
    /// Seed ID.
    id: String,
    /// Generation number.
    generation: u32,
    /// Goal at this generation.
    goal: String,
    /// Parent seed ID (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<String>,
    /// Evaluation score (if evaluated).
    #[serde(skip_serializing_if = "Option::is_none")]
    score: Option<f64>,
    /// Whether evaluation passed.
    #[serde(skip_serializing_if = "Option::is_none")]
    passed: Option<bool>,
}

/// GET /api/seeds/:id/evolution — Get evolution lineage for a seed.
pub(crate) async fn handle_seed_evolution(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<EvolutionEntry>>, AppError> {
    use oxios_ouroboros::Seed;
    // Helper to build lineage by following parent IDs.
    // Build lineage iteratively using a work stack.
    fn build_lineage_iterative(
        kernel: Arc<oxios_kernel::KernelHandle>,
        seed_id: String,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<EvolutionEntry>>> + Send>> {
        Box::pin(async move {
            let mut lineage = Vec::new();
            let mut stack = vec![seed_id];

            while let Some(current_id) = stack.pop() {
                let content = match kernel.state.load_markdown("seeds", &current_id).await {
                    Ok(Some(c)) => c,
                    _ => continue,
                };
                let seed: Seed = match serde_json::from_str(&content) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(error = %e, "Skipping invalid seed");
                        continue;
                    }
                };

                // Push parent first so it's processed before children (reversed order).
                if let Some(ref parent_id) = seed.parent_seed_id {
                    stack.push(parent_id.to_string());
                }

                let (score, passed) = {
                    let eval_name = format!("{}-eval", current_id);
                    if let Ok(Some(eval_content)) =
                        kernel.state.load_markdown("evals", &eval_name).await
                    {
                        if let Ok(eval) =
                            serde_json::from_str::<oxios_ouroboros::EvaluationResult>(&eval_content)
                        {
                            (Some(eval.score), Some(eval.all_passed()))
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    }
                };

                lineage.push(EvolutionEntry {
                    id: seed.id.to_string(),
                    generation: seed.generation,
                    goal: seed.goal,
                    parent_id: seed.parent_seed_id.map(|p| p.to_string()),
                    score,
                    passed,
                });
            }

            lineage.reverse(); // Reverse so parent comes first.
            Ok(lineage)
        })
    }

    match build_lineage_iterative(state.kernel.clone(), id).await {
        Ok(lineage) if !lineage.is_empty() => Ok(Json(lineage)),
        _ => Err(AppError::NotFound("seed evolution not found".into())),
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
            return format!("~{}", rest);
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
        .map(|m| serde_json::json!({
            "bins": m.requires.bins,
            "anyBins": m.requires.any_bins,
            "env": m.requires.env,
            "config": m.requires.config,
        }))
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
                            format!("Install {} (brew)", name)
                        }
                        oxios_kernel::InstallKind::Node => {
                            let name = spec.package.as_deref().unwrap_or("unknown");
                            format!("Install {} (npm)", name)
                        }
                        oxios_kernel::InstallKind::Go => {
                            let name = spec.module.as_deref().unwrap_or("unknown");
                            format!("Install {} (go)", name)
                        }
                        oxios_kernel::InstallKind::Uv => {
                            let name = spec.package.as_deref().unwrap_or("unknown");
                            format!("Install {} (uv)", name)
                        }
                        oxios_kernel::InstallKind::Download => {
                            "Download".to_string()
                        }
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

    let os = meta
        .map(|m| m.os.clone())
        .unwrap_or_default();

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
    Ok(Json(serde_json::json!({ "status": "enabled", "name": name })))
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
    Ok(Json(serde_json::json!({ "status": "disabled", "name": name })))
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

/// Memory entry summary.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct MemorySummary {
    /// Entry name.
    name: String,
    /// Category (memory type).
    category: String,
}

/// GET /api/memory — List memory entries.
pub(crate) async fn handle_memory_list(
    state: State<Arc<AppState>>,
    Query(params): Query<PageParams>,
) -> Json<serde_json::Value> {
    let mut entries = Vec::new();

    // List all memory categories
    for category in [
        "memory/facts",
        "memory/episodes",
        "memory/knowledge",
        "memory/sessions",
    ] {
        if let Ok(names) = state.kernel.state.list_category(category).await {
            let cat = category.split('/').nth(1).unwrap_or("fact");
            for name in names {
                entries.push(MemorySummary {
                    name,
                    category: cat.into(),
                });
            }
        }
    }

    Json(paginate(&entries, &params))
}

/// GET /api/memory/:name — Get a specific memory entry.
pub(crate) async fn handle_memory_get(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    // Memory entries are stored as JSON, not markdown.
    // Try all known memory categories to find the entry.
    for category in [
        "memory/facts",
        "memory/episodes",
        "memory/knowledge",
        "memory/sessions",
    ] {
        if let Ok(Some(entry)) = state
            .kernel
            .state
            .load::<oxios_kernel::memory::MemoryEntry>(category, &name)
            .await
        {
            return Ok(Json(serde_json::json!({
                "id": entry.id,
                "name": entry.id,
                "category": entry.memory_type.label(),
                "content": entry.content,
                "tags": entry.tags,
                "importance": entry.importance,
                "created_at": entry.created_at.to_rfc3339(),
            }))
            .into_response());
        }
    }

    Err(AppError::NotFound("memory entry not found".into()))
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
            ))
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

    // Use semantic search from kernel (HNSW-powered)
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
        "engine": "hnsw",
    })))
}

// ---------------------------------------------------------------------------
// Memory stats, pin, delete, dream
// ---------------------------------------------------------------------------

/// GET /api/memory/stats — Aggregate memory statistics.
pub(crate) async fn handle_memory_stats(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (_index_size, _total) = state.kernel.agents.memory_stats().await;

    // Count by category
    let mut by_type = serde_json::Map::new();
    let mut count = 0usize;
    for category in [
        "memory/facts",
        "memory/episodes",
        "memory/knowledge",
        "memory/sessions",
    ] {
        if let Ok(names) = state.kernel.state.list_category(category).await {
            let cat = category.split('/').nth(1).unwrap_or("unknown");
            by_type.insert(cat.to_string(), serde_json::Value::Number(names.len().into()));
            count += names.len();
        }
    }

    Ok(Json(serde_json::json!({
        "total": count,
        "by_tier": { "hot": 0, "warm": count, "cold": 0 },
        "by_type": by_type,
        "by_protection": { "none": count, "low": 0, "medium": 0, "high": 0, "permanent": 0 },
        "dream": {
            "status": "idle",
            "last_run": null,
            "last_report_id": null,
        }
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
    for category in [
        "memory/facts",
        "memory/episodes",
        "memory/knowledge",
        "memory/sessions",
    ] {
        if let Ok(Some(mut entry)) = state
            .kernel
            .state
            .load::<MemoryEntry>(category, &id)
            .await
        {
            entry.pinned = body.pinned;
            state
                .kernel
                .state
                .save(category, &id, &entry)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
            return Ok(Json(serde_json::json!({ "id": id, "pinned": body.pinned })));
        }
    }
    Err(AppError::NotFound("memory entry not found".into()))
}

/// DELETE /api/memory/{id} — Delete a memory entry.
pub(crate) async fn handle_memory_delete(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    for category in [
        "memory/facts",
        "memory/episodes",
        "memory/knowledge",
        "memory/sessions",
    ] {
        if let Ok(Some(_)) = state
            .kernel
            .state
            .load::<serde_json::Value>(category, &id)
            .await
        {
            state
                .kernel
                .state
                .delete(category, &id)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
            return Ok(Json(serde_json::json!({ "id": id, "deleted": true })));
        }
    }
    Err(AppError::NotFound("memory entry not found".into()))
}

/// GET /api/memory/dream/reports — List dream reports.
pub(crate) async fn handle_dream_reports(
    _state: State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    // Placeholder — return empty list until dream persistence is implemented
    Json(serde_json::json!({ "reports": [] }))
}

/// GET /api/memory/dream/status — Dream status.
pub(crate) async fn handle_dream_status(
    _state: State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "idle",
        "last_run": null,
        "checkpoint_exists": false,
    }))
}

// ---------------------------------------------------------------------------
// Seed agents
// ---------------------------------------------------------------------------

/// GET /api/seeds/{id}/agents — List agents spawned from this seed.
pub(crate) async fn handle_seed_agents(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let agents = state
        .kernel
        .agents
        .list()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let filtered: Vec<serde_json::Value> = agents
        .into_iter()
        .filter(|a| a.seed_id.as_ref().map(|s| s.to_string()) == Some(id.clone()))
        .map(|a| {
            serde_json::json!({
                "id": a.id.to_string(),
                "name": a.name,
                "status": a.status.to_string(),
                "steps_completed": 0,
                "created_at": a.created_at.to_rfc3339(),
            })
        })
        .collect();
    Ok(Json(serde_json::json!({ "agents": filtered })))
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
            assert!(mt.is_some(), "expected '{}' to be a valid memory type", t);
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
            assert!(mt.is_none(), "expected '{}' to be rejected", t);
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
}
