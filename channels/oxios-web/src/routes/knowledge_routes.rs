//! Knowledge API route handlers.
//!
//! Provides REST endpoints for the files.md-based knowledge editor.
//! All handlers interact with the workspace's `knowledge/` directory
//! via filesystem operations (fallback until KnowledgeApi is wired).
//!
//! Endpoints:
//! - GET  /api/knowledge/tree       — file tree listing
//! - GET  /api/knowledge/file/{*path} — read file
//! - PUT  /api/knowledge/file/{*path} — write file
//! - DELETE /api/knowledge/file/{*path} — delete file
//! - POST /api/knowledge/search     — search knowledge files
//! - GET  /api/knowledge/backlinks  — get backlinks for a file
//! - GET  /api/knowledge/graph      — link graph for visualization
//! - POST /api/knowledge/copilot    — AI copilot chat

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::server::AppState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

/// Query parameters for tree listing.
#[derive(Debug, Deserialize)]
pub(crate) struct KnowledgeTreeParams {
    /// Subdirectory within knowledge/ to list.
    #[serde(default)]
    pub dir: Option<String>,
}

/// File tree entry.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct KnowledgeTreeEntry {
    /// File or directory name.
    pub name: String,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// File size in bytes (0 for directories).
    pub size: u64,
}

/// Search request body.
#[derive(Debug, Deserialize)]
pub(crate) struct KnowledgeSearchBody {
    /// Search query string.
    pub query: String,
    /// Maximum number of results.
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

fn default_search_limit() -> usize {
    20
}

/// Search result entry.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct KnowledgeSearchHit {
    /// File path relative to knowledge/.
    pub path: String,
    /// File name.
    pub name: String,
    /// Content snippet around the match.
    pub snippet: String,
    /// Line number where match was found.
    pub line: usize,
}

/// Backlinks query parameters.
#[derive(Debug, Deserialize)]
pub(crate) struct KnowledgeBacklinksParams {
    /// File path to get backlinks for.
    pub path: String,
}

/// Backlink entry.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct KnowledgeBacklink {
    /// Source file path (the file containing the link).
    pub source_path: String,
    /// Link text.
    pub link_text: String,
    /// Context around the link.
    pub context: String,
}

/// Link graph node.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct KnowledgeGraphNode {
    /// Node ID (file path).
    pub id: String,
    /// Display label.
    pub label: String,
    /// Group (directory).
    pub group: String,
}

/// Link graph edge.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct KnowledgeGraphEdge {
    /// Source node ID.
    pub source: String,
    /// Target node ID.
    pub target: String,
    /// Link text (if available).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
}

/// Link graph response.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct KnowledgeGraph {
    /// Nodes.
    pub nodes: Vec<KnowledgeGraphNode>,
    /// Edges.
    pub edges: Vec<KnowledgeGraphEdge>,
}

/// Copilot request body.
#[derive(Debug, Deserialize)]
pub(crate) struct KnowledgeCopilotBody {
    /// User's question.
    pub question: String,
    /// Currently open file path for context.
    pub context_path: Option<String>,
}

/// Copilot response.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct KnowledgeCopilotResponse {
    /// Response text.
    pub content: String,
    /// Referenced note paths.
    pub referenced_notes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Helper: resolve knowledge directory path
// ---------------------------------------------------------------------------

/// Returns the knowledge base directory path: `{workspace}/knowledge/`.
fn knowledge_base_path(state: &AppState) -> PathBuf {
    state.kernel.state.workspace_path().join("knowledge")
}

/// Resolve a relative path within the knowledge directory, with path traversal protection.
fn resolve_knowledge_path(
    state: &AppState,
    relative: &str,
) -> Result<PathBuf, AppError> {
    let base = knowledge_base_path(state);
    // Ensure base exists
    let canonical_base = base.canonicalize().unwrap_or_else(|_| base.clone());

    let candidate = base.join(relative);

    // For non-existent files, check parent path
    let resolved = match candidate.canonicalize() {
        Ok(c) => c,
        Err(_) => {
            // File doesn't exist yet — verify parent is safe
            if let Some(parent) = candidate.parent() {
                // Create parent dirs if needed
                if !parent.exists() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| AppError::Internal(format!("failed to create directory: {e}")))?;
                }
                let canonical_parent = parent.canonicalize().map_err(|e| {
                    AppError::Internal(format!("failed to resolve path: {e}"))
                })?;
                if !canonical_parent.starts_with(&canonical_base) {
                    return Err(AppError::Forbidden("path traversal denied".into()));
                }
            }
            candidate
        }
    };

    // Verify the resolved path is within the knowledge directory
    if resolved.starts_with(&canonical_base) {
        Ok(resolved)
    } else {
        Err(AppError::Forbidden("path traversal denied".into()))
    }
}

/// Guess MIME type from file extension.
fn guess_knowledge_mime(path: &str) -> String {
    match path.rsplit('.').next() {
        Some("md") => "text/markdown; charset=utf-8".into(),
        Some("json") => "application/json".into(),
        Some("toml") => "application/toml".into(),
        Some("yaml" | "yml") => "application/yaml".into(),
        Some("txt") => "text/plain; charset=utf-8".into(),
        Some("html") => "text/html".into(),
        Some("css") => "text/css".into(),
        Some("js") => "application/javascript".into(),
        Some("png") => "image/png".into(),
        Some("jpg" | "jpeg") => "image/jpeg".into(),
        Some("gif") => "image/gif".into(),
        Some("webp") => "image/webp".into(),
        _ => "text/plain; charset=utf-8".into(),
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/knowledge/tree — File tree of the knowledge directory.
pub(crate) async fn handle_knowledge_tree(
    state: State<Arc<AppState>>,
    Query(params): Query<KnowledgeTreeParams>,
) -> Result<Json<Vec<KnowledgeTreeEntry>>, AppError> {
    let base = knowledge_base_path(&state);

    // Ensure knowledge directory exists
    if !base.exists() {
        tokio::fs::create_dir_all(&base)
            .await
            .map_err(|e| AppError::Internal(format!("failed to create knowledge dir: {e}")))?;
    }

    let canonical_base = base.canonicalize().unwrap_or_else(|_| base.clone());
    let dir = match &params.dir {
        Some(d) if !d.is_empty() => {
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
        _ => canonical_base,
    };

    let mut entries = Vec::new();
    if let Ok(mut read_dir) = tokio::fs::read_dir(&dir).await {
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let metadata = match entry.metadata().await {
                Ok(m) => m,
                Err(_) => continue,
            };
            entries.push(KnowledgeTreeEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir: metadata.is_dir(),
                size: metadata.len(),
            });
        }
    }

    // Sort: directories first, then alphabetical
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));

    Ok(Json(entries))
}

/// GET /api/knowledge/file/{*path} — Read a knowledge file.
pub(crate) async fn handle_knowledge_file_get(
    state: State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let full_path = resolve_knowledge_path(&state, &path)?;

    let canonical_base = knowledge_base_path(&state)
        .canonicalize()
        .unwrap_or_else(|_| knowledge_base_path(&state));
    let canonical_file = match full_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return Err(AppError::NotFound("file not found".into())),
    };

    if !canonical_file.starts_with(&canonical_base) {
        return Err(AppError::Forbidden("path traversal denied".into()));
    }

    match tokio::fs::read_to_string(&canonical_file).await {
        Ok(content) => {
            let mime = guess_knowledge_mime(&path);
            Ok((
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, mime)],
                content,
            )
                .into_response())
        }
        Err(_) => Err(AppError::NotFound("file not found".into())),
    }
}

/// PUT /api/knowledge/file/{*path} — Write/update a knowledge file.
pub(crate) async fn handle_knowledge_file_put(
    state: State<Arc<AppState>>,
    Path(path): Path<String>,
    body: String,
) -> Result<StatusCode, AppError> {
    // Validate file size (max 5MB for knowledge files)
    const MAX_KNOWLEDGE_FILE_SIZE: usize = 5 * 1024 * 1024;
    if body.len() > MAX_KNOWLEDGE_FILE_SIZE {
        return Err(AppError::PayloadTooLarge {
            size: body.len(),
            limit: MAX_KNOWLEDGE_FILE_SIZE,
        });
    }

    let full_path = resolve_knowledge_path(&state, &path)?;

    match tokio::fs::write(&full_path, &body).await {
        Ok(_) => {
            tracing::info!(path = %path, "Knowledge file written");
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) => {
            tracing::error!(path = %path, error = %e, "Failed to write knowledge file");
            Err(AppError::Internal("failed to write file".into()))
        }
    }
}

/// DELETE /api/knowledge/file/{*path} — Delete a knowledge file.
pub(crate) async fn handle_knowledge_file_delete(
    state: State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Result<StatusCode, AppError> {
    let full_path = resolve_knowledge_path(&state, &path)?;

    let canonical_base = knowledge_base_path(&state)
        .canonicalize()
        .unwrap_or_else(|_| knowledge_base_path(&state));
    let canonical_file = match full_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return Err(AppError::NotFound("file not found".into())),
    };

    if !canonical_file.starts_with(&canonical_base) {
        return Err(AppError::Forbidden("path traversal denied".into()));
    }

    match tokio::fs::remove_file(&canonical_file).await {
        Ok(_) => {
            tracing::info!(path = %path, "Knowledge file deleted");
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(AppError::NotFound("file not found".into()))
        }
        Err(e) => {
            tracing::error!(path = %path, error = %e, "Failed to delete knowledge file");
            Err(AppError::Internal("failed to delete file".into()))
        }
    }
}

/// POST /api/knowledge/search — Search knowledge files.
///
/// Performs a simple text search across all .md files in the knowledge directory.
/// TODO: Replace with HNSW semantic search when KnowledgeApi is wired.
pub(crate) async fn handle_knowledge_search(
    state: State<Arc<AppState>>,
    Json(body): Json<KnowledgeSearchBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let base = knowledge_base_path(&state);

    if !base.exists() {
        return Ok(Json(serde_json::json!({
            "results": [],
            "count": 0,
            "query": body.query,
        })));
    }

    let query_lower = body.query.to_lowercase();
    let limit = body.limit.min(100);
    let mut results: Vec<KnowledgeSearchHit> = Vec::new();

    // Recursively search .md files
    search_dir(&base, &base, &query_lower, limit, &mut results).await?;

    Ok(Json(serde_json::json!({
        "results": results,
        "count": results.len(),
        "query": body.query,
    })))
}

/// Recursively search a directory for .md files containing the query.
async fn search_dir(
    base: &std::path::Path,
    current: &std::path::Path,
    query_lower: &str,
    limit: usize,
    results: &mut Vec<KnowledgeSearchHit>,
) -> Result<(), AppError> {
    if results.len() >= limit {
        return Ok(());
    }

    let mut entries = tokio::fs::read_dir(current)
        .await
        .map_err(|e| AppError::Internal(format!("failed to read directory: {e}")))?;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| AppError::Internal(format!("failed to read entry: {e}")))?
    {
        if results.len() >= limit {
            break;
        }

        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().into_owned();

        let metadata = entry
            .metadata()
            .await
            .map_err(|e| AppError::Internal(format!("failed to read metadata: {e}")))?;

        if metadata.is_dir() {
            // Skip hidden dirs and media
            if !file_name.starts_with('.') && file_name != "media" {
                Box::pin(search_dir(base, &path, query_lower, limit, results)).await?;
            }
        } else if file_name.ends_with(".md") || file_name.ends_with(".txt") {
            // Text search in file
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                let content_lower = content.to_lowercase();
                if let Some(idx) = content_lower.find(query_lower) {
                    let relative = path
                        .strip_prefix(base)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .into_owned();

                    // Get a snippet around the match
                    let start = idx.saturating_sub(60);
                    let end = (idx + query_lower.len() + 80).min(content.len());
                    let snippet = if start > 0 || end < content.len() {
                        format!(
                            "{}{}{}",
                            if start > 0 { "..." } else { "" },
                            &content[start..end],
                            if end < content.len() { "..." } else { "" }
                        )
                    } else {
                        content.clone()
                    };

                    // Find line number
                    let line = content[..idx].lines().count() + 1;

                    results.push(KnowledgeSearchHit {
                        path: relative,
                        name: file_name,
                        snippet,
                        line,
                    });
                }
            }
        }
    }

    Ok(())
}

/// GET /api/knowledge/backlinks — Get backlinks for a file.
///
/// Scans all .md files for wikilinks pointing to the requested file.
/// TODO: Replace with BacklinkIndex when KnowledgeApi is wired.
pub(crate) async fn handle_knowledge_backlinks(
    state: State<Arc<AppState>>,
    Query(params): Query<KnowledgeBacklinksParams>,
) -> Result<Json<Vec<KnowledgeBacklink>>, AppError> {
    let base = knowledge_base_path(&state);

    if !base.exists() {
        return Ok(Json(Vec::new()));
    }

    let target_name = params
        .path
        .rsplit('/')
        .next()
        .unwrap_or(&params.path)
        .trim_end_matches(".md")
        .to_lowercase();
    let target_path_lower = params.path.trim_start_matches('/').to_lowercase();

    let mut backlinks = Vec::new();

    // Scan all .md files for links to the target
    scan_for_links(&base, &base, &target_name, &target_path_lower, &mut backlinks).await?;

    Ok(Json(backlinks))
}

/// Recursively scan for links pointing to a target file.
async fn scan_for_links(
    base: &std::path::Path,
    current: &std::path::Path,
    target_name: &str,
    target_path_lower: &str,
    results: &mut Vec<KnowledgeBacklink>,
) -> Result<(), AppError> {
    let mut entries = tokio::fs::read_dir(current)
        .await
        .map_err(|e| AppError::Internal(format!("failed to read directory: {e}")))?;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| AppError::Internal(format!("failed to read entry: {e}")))?
    {
        let path = entry.path();
        let metadata = entry
            .metadata()
            .await
            .map_err(|e| AppError::Internal(format!("failed to read metadata: {e}")))?;

        if metadata.is_dir() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with('.') && name != "media" {
                Box::pin(scan_for_links(
                    base,
                    &path,
                    target_name,
                    target_path_lower,
                    results,
                ))
                .await?;
            }
        } else if entry.file_name().to_string_lossy().ends_with(".md") {
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                // Look for markdown links: [text](path) and [[wikilinks]]
                let relative = path
                    .strip_prefix(base)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned();

                // Skip self-references
                if relative.to_lowercase() == target_path_lower
                    || relative.to_lowercase().trim_end_matches(".md")
                        == target_path_lower.trim_end_matches(".md")
                {
                    continue;
                }

                // Search for markdown links [text](target)
                for line in content.lines() {
                    let line_lower = line.to_lowercase();
                    // Simple pattern: look for target name in link syntax
                    if line_lower.contains(&format!("({}", target_name))
                        || line_lower.contains(&format!("({}.md)", target_name))
                        || line_lower.contains(&format!("[[{}]]", target_name))
                    {
                        // Extract context around the link
                        let snippet: String = if line.len() > 120 {
                            format!("{}...", &line[..120])
                        } else {
                            line.to_string()
                        };

                        results.push(KnowledgeBacklink {
                            source_path: relative.clone(),
                            link_text: target_name.to_string(),
                            context: snippet,
                        });
                        break; // One backlink per file
                    }
                }
            }
        }
    }

    Ok(())
}

/// GET /api/knowledge/graph — Get link graph for visualization.
///
/// Scans all .md files for inter-note links and builds a graph.
/// TODO: Replace with BacklinkIndex.link_graph() when KnowledgeApi is wired.
pub(crate) async fn handle_knowledge_graph(
    state: State<Arc<AppState>>,
) -> Result<Json<KnowledgeGraph>, AppError> {
    let base = knowledge_base_path(&state);

    if !base.exists() {
        return Ok(Json(KnowledgeGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        }));
    }

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Collect all .md files as nodes and scan for links
    collect_graph(&base, &base, &mut nodes, &mut edges).await?;

    Ok(Json(KnowledgeGraph { nodes, edges }))
}

/// Recursively collect graph nodes and edges.
async fn collect_graph(
    base: &std::path::Path,
    current: &std::path::Path,
    nodes: &mut Vec<KnowledgeGraphNode>,
    edges: &mut Vec<KnowledgeGraphEdge>,
) -> Result<(), AppError> {
    let mut entries = tokio::fs::read_dir(current)
        .await
        .map_err(|e| AppError::Internal(format!("failed to read directory: {e}")))?;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| AppError::Internal(format!("failed to read entry: {e}")))?
    {
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let metadata = entry
            .metadata()
            .await
            .map_err(|e| AppError::Internal(format!("failed to read metadata: {e}")))?;

        if metadata.is_dir() {
            if !file_name.starts_with('.') && file_name != "media" {
                Box::pin(collect_graph(base, &path, nodes, edges)).await?;
            }
        } else if file_name.ends_with(".md") {
            let relative = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();

            let label = file_name.trim_end_matches(".md").to_string();
            let group = relative
                .split('/')
                .next()
                .unwrap_or("")
                .to_string();

            nodes.push(KnowledgeGraphNode {
                id: relative.clone(),
                label,
                group,
            });

            // Scan for links
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                // Find markdown links: [text](path.md)
                for line in content.lines() {
                    // Simple regex-free link extraction
                    extract_links_from_line(line, &relative, edges);
                }
            }
        }
    }

    Ok(())
}

/// Extract markdown links from a line and add to edges.
fn extract_links_from_line(
    line: &str,
    source_path: &str,
    edges: &mut Vec<KnowledgeGraphEdge>,
) {
    let mut i = 0;
    let line_bytes = line.as_bytes();

    while i < line.len() {
        // Look for [[wikilink]] pattern FIRST (before single-bracket check)
        if i + 1 < line.len() && line_bytes[i] == b'[' && line_bytes[i + 1] == b'[' {
            if let Some(end) = line[i + 2..].find("]]") {
                let link_target = &line[i + 2..i + 2 + end];
                let target = if link_target.ends_with(".md") {
                    link_target.to_string()
                } else {
                    format!("{}.md", link_target)
                };
                edges.push(KnowledgeGraphEdge {
                    source: source_path.to_string(),
                    target,
                    label: link_target.to_string(),
                });
                i = i + 2 + end + 2;
                continue;
            }
        }

        // Look for [text](path) pattern
        if line_bytes[i] == b'[' {
            // Find closing ]
            if let Some(bracket_end) = line[i + 1..].find(']') {
                let link_start = i + 1 + bracket_end + 1;
                if link_start < line.len() && line_bytes[link_start] == b'(' {
                    // Find closing )
                    if let Some(paren_end) = line[link_start + 1..].find(')') {
                        let link_target =
                            &line[link_start + 1..link_start + 1 + paren_end];

                        // Only include .md or bare path links (not URLs)
                        if !link_target.starts_with("http://")
                            && !link_target.starts_with("https://")
                            && !link_target.starts_with('#')
                            && !link_target.is_empty()
                        {
                            let link_text =
                                &line[i + 1..i + 1 + bracket_end];

                            // Normalize target path
                            let target = if link_target.ends_with(".md") {
                                link_target.to_string()
                            } else if !link_target.contains('.') {
                                format!("{}.md", link_target)
                            } else {
                                link_target.to_string()
                            };

                            edges.push(KnowledgeGraphEdge {
                                source: source_path.to_string(),
                                target,
                                label: link_text.to_string(),
                            });
                        }

                        i = link_start + 1 + paren_end + 1;
                        continue;
                    }
                }
                i = i + 1 + bracket_end + 1;
                continue;
            }
        }

        i += 1;
    }
}

/// POST /api/knowledge/copilot — AI copilot chat.
///
/// Uses the kernel's Oxi engine to answer questions about knowledge files.
/// TODO: Replace with KnowledgeApi.copilot_chat() when Phase 2 is wired.
pub(crate) async fn handle_knowledge_copilot(
    state: State<Arc<AppState>>,
    Json(body): Json<KnowledgeCopilotBody>,
) -> Result<Json<KnowledgeCopilotResponse>, AppError> {
    // Validate question size
    const MAX_QUESTION_SIZE: usize = 10 * 1024;
    if body.question.len() > MAX_QUESTION_SIZE {
        return Err(AppError::PayloadTooLarge {
            size: body.question.len(),
            limit: MAX_QUESTION_SIZE,
        });
    }

    let base = knowledge_base_path(&state);
    let mut context_parts = Vec::new();
    let mut referenced_notes = Vec::new();

    // 1. Load context file if provided
    if let Some(ref ctx_path) = body.context_path {
        if !ctx_path.is_empty() {
            let full_path = base.join(ctx_path.as_str());
            if let Ok(content) = tokio::fs::read_to_string(&full_path).await {
                let snippet: String = content.chars().take(2000).collect();
                context_parts.push(format!("## Current file: {}\n\n{}", ctx_path, snippet));
                referenced_notes.push(ctx_path.clone());
            }
        }
    }

    // 2. Simple text search to find related files
    let query_lower = body.question.to_lowercase();
    if base.exists() {
        let mut search_results: Vec<(String, String)> = Vec::new();
        find_relevant_files(&base, &base, &query_lower, 5, &mut search_results).await?;

        for (path, snippet) in &search_results {
            context_parts.push(format!("## Related: {}\n\n{}", path, snippet));
            referenced_notes.push(path.clone());
        }
    }

    // 3. Build response (placeholder until oxi engine is integrated)
    // For now, return a helpful message indicating copilot is not yet wired
    let response = if context_parts.is_empty() {
        "The knowledge copilot is being set up. Please try again once the AI engine is fully connected.\n\nIn the meantime, you can:\n- Create and edit markdown notes\n- Use the sidebar to browse your knowledge base\n- Search for files with ⌘P".to_string()
    } else {
        format!(
            "I can see you have context from {} file(s), but the AI engine is still being wired up (Phase 4). \
            Your knowledge base is ready for editing.\n\nFiles in context: {}",
            context_parts.len(),
            referenced_notes.join(", ")
        )
    };

    Ok(Json(KnowledgeCopilotResponse {
        content: response,
        referenced_notes,
    }))
}

/// Find relevant files by simple text matching.
async fn find_relevant_files(
    base: &std::path::Path,
    current: &std::path::Path,
    query_lower: &str,
    limit: usize,
    results: &mut Vec<(String, String)>,
) -> Result<(), AppError> {
    if results.len() >= limit {
        return Ok(());
    }

    let mut entries = tokio::fs::read_dir(current)
        .await
        .map_err(|e| AppError::Internal(format!("failed to read directory: {e}")))?;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| AppError::Internal(format!("failed to read entry: {e}")))?
    {
        if results.len() >= limit {
            break;
        }

        let path = entry.path();
        let metadata = entry
            .metadata()
            .await
            .map_err(|e| AppError::Internal(format!("failed to read metadata: {e}")))?;

        if metadata.is_dir() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with('.') && name != "media" {
                Box::pin(find_relevant_files(base, &path, query_lower, limit, results)).await?;
            }
        } else if entry.file_name().to_string_lossy().ends_with(".md") {
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                let content_lower = content.to_lowercase();
                // Check for keyword matches
                let keywords: Vec<&str> = query_lower.split_whitespace().take(5).collect();
                let match_count = keywords
                    .iter()
                    .filter(|kw| content_lower.contains(*kw))
                    .count();

                if match_count > 0 {
                    let relative = path
                        .strip_prefix(base)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .into_owned();
                    let snippet: String = content.chars().take(500).collect();
                    results.push((relative, snippet));
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guess_knowledge_mime() {
        assert_eq!(
            guess_knowledge_mime("brain/Rust.md"),
            "text/markdown; charset=utf-8"
        );
        assert_eq!(
            guess_knowledge_mime("data.json"),
            "application/json"
        );
        assert_eq!(
            guess_knowledge_mime("image.png"),
            "image/png"
        );
        assert_eq!(
            guess_knowledge_mime("unknown.bin"),
            "text/plain; charset=utf-8"
        );
    }

    #[test]
    fn test_extract_links_basic() {
        let mut edges = Vec::new();
        extract_links_from_line(
            "See [Architecture](brain/Architecture.md) for details.",
            "brain/Overview.md",
            &mut edges,
        );
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].source, "brain/Overview.md");
        assert_eq!(edges[0].target, "brain/Architecture.md");
        assert_eq!(edges[0].label, "Architecture");
    }

    #[test]
    fn test_extract_links_wikilink() {
        let mut edges = Vec::new();
        extract_links_from_line(
            "Related: [[Rust]] and [[Go]]",
            "brain/Langs.md",
            &mut edges,
        );
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].target, "Rust.md");
        assert_eq!(edges[1].target, "Go.md");
    }

    #[test]
    fn test_extract_links_ignores_urls() {
        let mut edges = Vec::new();
        extract_links_from_line(
            "Visit [Google](https://google.com) for more.",
            "test.md",
            &mut edges,
        );
        assert!(edges.is_empty());
    }

    #[test]
    fn test_extract_links_bare_path() {
        let mut edges = Vec::new();
        extract_links_from_line(
            "See [Notes](brain/ideas) for context.",
            "root.md",
            &mut edges,
        );
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].target, "brain/ideas.md");
    }

    #[test]
    fn test_tree_entry_serialization() {
        let entry = KnowledgeTreeEntry {
            name: "Rust.md".into(),
            is_dir: false,
            size: 1024,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["name"], "Rust.md");
        assert_eq!(json["is_dir"], false);
        assert_eq!(json["size"], 1024);
    }

    #[test]
    fn test_search_hit_serialization() {
        let hit = KnowledgeSearchHit {
            path: "brain/Rust.md".into(),
            name: "Rust.md".into(),
            snippet: "...ownership model...".into(),
            line: 5,
        };
        let json = serde_json::to_value(&hit).unwrap();
        assert_eq!(json["path"], "brain/Rust.md");
        assert_eq!(json["line"], 5);
    }
}
