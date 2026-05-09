use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use oxios_kernel::memory::{MemoryEntry, MemoryType};

use crate::error::AppError;
use crate::routes::{PageParams, paginate};
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
    let base = state.kernel.workspace_path();
    let canonical_base = base.canonicalize()
        .unwrap_or_else(|_| base.to_path_buf());
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

    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name))
    });

    Ok(Json(entries))
}

/// GET /api/workspace/file/*path — Read a file.
pub(crate) async fn handle_workspace_file_get(
    state: State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let base = state.kernel.workspace_path();
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

    let base = state.kernel.workspace_path();
    let full_path = base.join(&path);

    // Security: ensure the path doesn't escape the workspace
    let canonical_base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    if let Some(parent) = full_path.parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| AppError::Internal(format!("failed to create directory: {e}")))?;
        }
        let canonical_parent = parent.canonicalize()
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

    if let Ok(names) = state.kernel.list_category("seeds").await {
        for name in names {
            if let Ok(Some(content)) = state.kernel.load_markdown("seeds", &name).await {
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
    if let Ok(Some(content)) = state.kernel.load_markdown("seeds", &id).await {
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
                let content = match kernel.load_markdown("seeds", &current_id).await {
                    Ok(Some(c)) => c,
                    _ => continue,
                };
                let seed: Seed = match serde_json::from_str(&content) {
                    Ok(s) => s,
                    Err(e) => { tracing::warn!(error = %e, "Skipping invalid seed"); continue }
                };

                // Push parent first so it's processed before children (reversed order).
                if let Some(ref parent_id) = seed.parent_seed_id {
                    stack.push(parent_id.to_string());
                }

                let (score, passed) = {
                    let eval_name = format!("{}-eval", current_id);
                    if let Ok(Some(eval_content)) =
                        kernel.load_markdown("evals", &eval_name).await
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


/// Skill summary for listing.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct SkillSummary {
    /// Skill name.
    name: String,
    /// Skill description.
    description: String,
}

/// GET /api/skills — List all skills.
pub(crate) async fn handle_skills_list(
    state: State<Arc<AppState>>,
    Query(params): Query<PageParams>,
) -> Json<serde_json::Value> {
    match state.kernel.list_skills().await {
        Ok(skills) => {
            let summaries: Vec<SkillSummary> = skills
                .into_iter()
                .map(|s| SkillSummary {
                    name: s.name,
                    description: s.description,
                })
                .collect();
            Json(paginate(&summaries, &params))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list skills");
            Json(paginate(&Vec::<SkillSummary>::new(), &params))
        }
    }
}

/// GET /api/skills/:name — Get skill content.
pub(crate) async fn handle_skill_get(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    match state.kernel.load_skill(&name).await {
        Ok(Some(skill)) => Ok(Json(serde_json::json!({
            "name": skill.meta.name,
            "description": skill.meta.description,
            "content": skill.content,
            "path": skill.path.to_string_lossy(),
        }))),
        Ok(None) => Err(AppError::NotFound("skill not found".into())),
        Err(e) => {
            tracing::error!(error = %e, "Failed to load skill");
            Err(AppError::Internal("failed to load skill".into()))
        }
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

    state.kernel.create_skill(&body.name, &body.description, &body.content).await
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
    state.kernel.delete_skill(&name).await
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

    // List daily memory files
    if let Ok(names) = state.kernel.list_category("memory").await {
        for name in names {
            entries.push(MemorySummary {
                name,
                category: "daily".into(),
            });
        }
    }

    // List knowledge base entries
    if let Ok(names) = state.kernel.list_category("memory/knowledge").await {
        for name in names {
            entries.push(MemorySummary {
                name,
                category: "knowledge".into(),
            });
        }
    }

    Json(paginate(&entries, &params))
}

/// GET /api/memory/:name — Get a specific memory entry.
pub(crate) async fn handle_memory_get(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    // Try memory/ first, then memory/knowledge/
    if let Ok(Some(content)) = state.kernel.load_markdown("memory", &name).await {
        return Ok(Json(serde_json::json!({
            "name": name,
            "category": "daily",
            "content": content,
        }))
        .into_response());
    }

    if let Ok(Some(content)) = state.kernel.load_markdown("memory/knowledge", &name).await {
        return Ok(Json(serde_json::json!({
            "name": name,
            "category": "knowledge",
            "content": content,
        }))
        .into_response());
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
        content: body.content,
        source: "api".to_string(),
        session_id: None,
        tags: body.tags,
        importance: body.importance,
        created_at: chrono::Utc::now(),
        accessed_at: chrono::Utc::now(),
        access_count: 0,
    };
    
    // Use memory manager from kernel
    let id = state.kernel.memory_remember(entry).await
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
    let entries = state.kernel.memory_search(&body.query, type_filter, limit).await
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
    Ok(Json(serde_json::json!({ "count": results.len(), "entries": results })))
}