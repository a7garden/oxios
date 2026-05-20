//! Knowledge API route handlers — thin adapters over KnowledgeApi.
//!
//! All file I/O, backlink tracking, and AI copilot are delegated to
//! `state.kernel.knowledge` (KnowledgeApi). This layer only handles
//! HTTP request parsing and JSON serialization.
//!
//! Endpoints:
//! - GET  /api/knowledge/tree/{*path} — file tree listing
//! - GET  /api/knowledge/file/{*path} — read file
//! - PUT  /api/knowledge/file/{*path} — write file
//! - DELETE /api/knowledge/file/{*path} — delete file
//! - POST /api/knowledge/search       — search knowledge files
//! - GET  /api/knowledge/backlinks    — get backlinks for a file
//! - GET  /api/knowledge/graph        — link graph for visualization
//! - POST /api/knowledge/copilot      — AI copilot chat

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
    /// Display name.
    pub name: String,
    /// Content snippet.
    pub snippet: String,
    /// Semantic similarity score (0.0–1.0).
    pub semantic_score: Option<f32>,
    /// Number of backlinks.
    pub backlink_count: usize,
    /// Name similarity score (0–100).
    pub name_similarity: i32,
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
// Handlers — all delegate to KnowledgeApi
// ---------------------------------------------------------------------------

/// GET /api/knowledge/tree — File tree of the knowledge directory.
pub(crate) async fn handle_knowledge_tree(
    state: State<Arc<AppState>>,
    Query(params): Query<KnowledgeTreeParams>,
) -> Result<Json<Vec<KnowledgeTreeEntry>>, AppError> {
    let dir = params.dir.as_deref().unwrap_or("");
    let entries = state
        .kernel
        .knowledge
        .note_tree(dir)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut result: Vec<KnowledgeTreeEntry> = entries
        .into_iter()
        .filter(|e| !e.name.starts_with('.') && e.name != ".DS_Store")
        .map(|e| KnowledgeTreeEntry {
            name: e.name,
            is_dir: e.is_dir,
            size: 0, // VirtualFs doesn't track file size
        })
        .collect();

    // Sort: directories first, then alphabetical
    result.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));

    Ok(Json(result))
}

/// GET /api/knowledge/file/{*path} — Read a knowledge file.
pub(crate) async fn handle_knowledge_file_get(
    state: State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let content = state
        .kernel
        .knowledge
        .note_read(&path)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("file not found".into()))?;

    let mime = guess_knowledge_mime(&path);
    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, mime)],
        content,
    )
        .into_response())
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

    state
        .kernel
        .knowledge
        .note_write(&path, &body)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(path = %path, "Knowledge file written");
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/knowledge/file/{*path} — Delete a knowledge file.
pub(crate) async fn handle_knowledge_file_delete(
    state: State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Result<StatusCode, AppError> {
    state
        .kernel
        .knowledge
        .note_delete(&path)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(path = %path, "Knowledge file deleted");
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/knowledge/search — Search knowledge files.
pub(crate) async fn handle_knowledge_search(
    state: State<Arc<AppState>>,
    Json(body): Json<KnowledgeSearchBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let hits = state
        .kernel
        .knowledge
        .search(&body.query, body.limit)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let results: Vec<KnowledgeSearchHit> = hits
        .into_iter()
        .map(|h| KnowledgeSearchHit {
            path: h.path,
            name: h.name,
            snippet: h.snippet,
            semantic_score: h.semantic_score,
            backlink_count: h.backlink_count,
            name_similarity: h.name_similarity,
        })
        .collect();

    let count = results.len();
    Ok(Json(serde_json::json!({
        "results": results,
        "count": count,
        "query": body.query,
    })))
}

/// GET /api/knowledge/backlinks — Get backlinks for a file.
pub(crate) async fn handle_knowledge_backlinks(
    state: State<Arc<AppState>>,
    Query(params): Query<KnowledgeBacklinksParams>,
) -> Result<Json<Vec<KnowledgeBacklink>>, AppError> {
    let backlinks = state.kernel.knowledge.backlinks_for(&params.path);

    let result: Vec<KnowledgeBacklink> = backlinks
        .into_iter()
        .map(|bl| KnowledgeBacklink {
            source_path: bl.source_path,
            link_text: bl.link_text,
            context: format!("line {}", bl.line_number),
        })
        .collect();

    Ok(Json(result))
}

/// GET /api/knowledge/graph — Get link graph for visualization.
pub(crate) async fn handle_knowledge_graph(
    state: State<Arc<AppState>>,
) -> Result<Json<KnowledgeGraph>, AppError> {
    let graph = state.kernel.knowledge.link_graph();

    Ok(Json(KnowledgeGraph {
        nodes: graph
            .nodes
            .into_iter()
            .map(|n| KnowledgeGraphNode {
                id: n.id,
                label: n.label,
                group: n.group,
            })
            .collect(),
        edges: graph
            .edges
            .into_iter()
            .map(|e| KnowledgeGraphEdge {
                source: e.source,
                target: e.target,
                label: e.label,
            })
            .collect(),
    }))
}

/// POST /api/knowledge/copilot — AI copilot chat.
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

    let result = state
        .kernel
        .knowledge
        .copilot_chat(&body.question, body.context_path.as_deref())
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(KnowledgeCopilotResponse {
        content: result.content,
        referenced_notes: result.referenced_notes,
    }))
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
        assert_eq!(guess_knowledge_mime("data.json"), "application/json");
        assert_eq!(guess_knowledge_mime("image.png"), "image/png");
        assert_eq!(
            guess_knowledge_mime("unknown.bin"),
            "text/plain; charset=utf-8"
        );
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
            semantic_score: Some(0.87),
            backlink_count: 3,
            name_similarity: 95,
        };
        let json = serde_json::to_value(&hit).unwrap();
        assert_eq!(json["path"], "brain/Rust.md");
        assert!(json["semantic_score"].as_f64().unwrap() - 0.87 < 0.01);
        assert_eq!(json["backlink_count"], 3);
    }

    #[test]
    fn test_backlink_serialization() {
        let bl = KnowledgeBacklink {
            source_path: "brain/Overview.md".into(),
            link_text: "Architecture".into(),
            context: "See [Architecture](brain/Architecture.md)".into(),
        };
        let json = serde_json::to_value(&bl).unwrap();
        assert_eq!(json["source_path"], "brain/Overview.md");
    }

    #[test]
    fn test_graph_serialization() {
        let graph = KnowledgeGraph {
            nodes: vec![KnowledgeGraphNode {
                id: "brain/Rust.md".into(),
                label: "Rust".into(),
                group: "brain".into(),
            }],
            edges: vec![KnowledgeGraphEdge {
                source: "brain/Rust.md".into(),
                target: "brain/Ownership.md".into(),
                label: "Ownership".into(),
            }],
        };
        let json = serde_json::to_value(&graph).unwrap();
        assert_eq!(json["nodes"][0]["label"], "Rust");
        assert_eq!(json["edges"][0]["target"], "brain/Ownership.md");
    }
}
