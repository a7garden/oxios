//! Knowledge API route handlers — direct adapters over KnowledgeBase.
//!
//! Most file I/O, backlink tracking, and app features are delegated to
//! `state.knowledge` (KnowledgeBase). This layer only handles HTTP request
//! parsing and JSON serialization.
//!
//! AI-powered features (copilot_chat) go through `state.kernel.knowledge`
//! (KnowledgeLens in Phase 3).
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
//!
//! Checklist:
//! - POST /api/knowledge/checklist/items   — list checklist items
//! - POST /api/knowledge/checklist/add     — add checklist item
//! - POST /api/knowledge/checklist/complete — complete checklist item
//! - POST /api/knowledge/checklist/remove  — remove checklist item
//!
//! Chat:
//! - POST /api/knowledge/chat/append   — append chat message
//! - GET  /api/knowledge/chat/messages  — list chat messages
//! - POST /api/knowledge/chat/delete    — delete chat message
//! - POST /api/knowledge/chat/move      — move chat message to file
//!
//! Journal:
//! - POST /api/knowledge/journal/add   — add journal record
//! - POST /api/knowledge/journal/emoji — add journal emoji
//! - GET  /api/knowledge/journal/today — today's journal path
//!
//! Habits:
//! - GET  /api/knowledge/habits         — habit data for a year
//! - GET  /api/knowledge/habits/last-week — last week's habits
//!
//! Stats:
//! - GET  /api/knowledge/stats/today    — today's completion report
//! - GET  /api/knowledge/stats/done-today — files completed today
//!
//! Config:
//! - GET  /api/knowledge/config         — read config
//! - PUT  /api/knowledge/config         — write config
//!
//! Worker:
//! - POST /api/knowledge/worker/nightly   — run nightly cleanup
//! - POST /api/knowledge/worker/scheduled — run scheduled tasks
//!
//! Convert:
//! - POST /api/knowledge/convert/html   — markdown → HTML
//! - GET  /api/knowledge/emoji           — auto-emoji lookup

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::Datelike;
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

// ---------------------------------------------------------------------------
// Checklist request / response types
// ---------------------------------------------------------------------------

/// Checklist items request body.
#[derive(Debug, Deserialize)]
pub(crate) struct ChecklistItemsBody {
    /// File path to read checklist from.
    pub path: String,
}

/// Checklist add request body.
#[derive(Debug, Deserialize)]
pub(crate) struct ChecklistAddBody {
    /// File path to add checklist item to.
    pub path: String,
    /// Checklist item text.
    pub item: String,
    /// Whether the item starts checked.
    #[serde(default)]
    pub checked: bool,
}

/// Checklist complete request body.
#[derive(Debug, Deserialize)]
pub(crate) struct ChecklistCompleteBody {
    /// File path.
    pub path: String,
    /// Hash of the item to complete.
    pub item_hash: String,
}

/// Checklist remove request body.
#[derive(Debug, Deserialize)]
pub(crate) struct ChecklistRemoveBody {
    /// File path.
    pub path: String,
    /// Item text or hash to remove.
    pub item_or_hash: String,
}

/// Checklist items response.
#[derive(Debug, Serialize)]
pub(crate) struct ChecklistItemsResponse {
    /// All checklist items.
    pub items: Vec<String>,
    /// Incomplete items only.
    pub incomplete: Vec<String>,
}

// ---------------------------------------------------------------------------
// Chat request / response types
// ---------------------------------------------------------------------------

/// Chat append request body.
#[derive(Debug, Deserialize)]
pub(crate) struct ChatAppendBody {
    /// Message text to append.
    pub message: String,
}

/// Chat delete request body.
#[derive(Debug, Deserialize)]
pub(crate) struct ChatDeleteBody {
    /// Hash of the message to delete.
    pub msg_hash: String,
}

/// Chat move request body.
#[derive(Debug, Deserialize)]
pub(crate) struct ChatMoveBody {
    /// Hash of the message to move.
    pub msg_hash: String,
    /// Target file path.
    pub target_path: String,
}

// ---------------------------------------------------------------------------
// Journal request / response types
// ---------------------------------------------------------------------------

/// Journal add record request body.
#[derive(Debug, Deserialize)]
pub(crate) struct JournalAddRecordBody {
    /// Record text to add.
    pub record: String,
}

/// Journal add emoji request body.
#[derive(Debug, Deserialize)]
pub(crate) struct JournalAddEmojiBody {
    /// Emoji to add.
    pub emoji: String,
}

/// Journal today response.
#[derive(Debug, Serialize)]
pub(crate) struct JournalTodayResponse {
    /// Path to today's journal file.
    pub path: String,
}

// ---------------------------------------------------------------------------
// Habits query params
// ---------------------------------------------------------------------------

/// Habits query parameters.
#[derive(Debug, Deserialize)]
pub(crate) struct HabitsParams {
    /// Year to fetch habits for.
    #[serde(default = "default_habits_year")]
    pub year: Option<i32>,
}

fn default_habits_year() -> Option<i32> {
    None
}

// ---------------------------------------------------------------------------
// Config request body (PUT)
// ---------------------------------------------------------------------------

/// Config update request body.
#[derive(Debug, Deserialize)]
pub(crate) struct KnowledgeConfigBody {
    /// Language code.
    #[serde(default)]
    pub language: Option<String>,
    /// Timezone.
    #[serde(default)]
    pub timezone: Option<String>,
    /// Move-to commands.
    #[serde(default)]
    pub move_to_commands: Option<Vec<String>>,
    /// Pomodoro duration in minutes.
    #[serde(default)]
    pub pomodoro_duration_in_minutes: Option<i64>,
    /// Scheduled tasks.
    #[serde(default)]
    pub schedules: Option<Vec<serde_json::Value>>,
    /// Quick commands.
    #[serde(default)]
    pub quick_commands: Option<Vec<String>>,
    /// Two emojis enabled.
    #[serde(default)]
    pub two_emojis_enabled: Option<bool>,
    /// Mode.
    #[serde(default)]
    pub mode: Option<String>,
    /// Quick habits enabled.
    #[serde(default)]
    pub quick_habits_enabled: Option<bool>,
    /// Associated channels.
    #[serde(default)]
    pub channels: Option<Vec<i64>>,
}

// ---------------------------------------------------------------------------
// Convert request / response types
// ---------------------------------------------------------------------------

/// Markdown to HTML request body.
#[derive(Debug, Deserialize)]
pub(crate) struct ConvertHtmlBody {
    /// Markdown text to convert.
    pub md: String,
}

/// Convert HTML response.
#[derive(Debug, Serialize)]
pub(crate) struct ConvertHtmlResponse {
    /// Converted HTML.
    pub html: String,
}

// ---------------------------------------------------------------------------
// Emoji query params
// ---------------------------------------------------------------------------

/// Emoji query parameters.
#[derive(Debug, Deserialize)]
pub(crate) struct EmojiQueryParams {
    /// Text to find an emoji for.
    pub text: String,
}

/// Emoji response.
#[derive(Debug, Serialize)]
pub(crate) struct EmojiResponse {
    /// Found emoji.
    pub emoji: String,
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
    let backlinks = state.knowledge.backlinks_for(&params.path);

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
    let graph = state.knowledge.link_graph();

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

// ---------------------------------------------------------------------------
// Checklist handlers
// ---------------------------------------------------------------------------

/// POST /api/knowledge/checklist/items — list checklist items.
pub(crate) async fn handle_knowledge_checklist_items(
    state: State<Arc<AppState>>,
    Json(body): Json<ChecklistItemsBody>,
) -> Result<Json<ChecklistItemsResponse>, AppError> {
    let (items, _completed) = state
        .kernel
        .knowledge
        .checklist_items(&body.path)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let incomplete = state
        .kernel
        .knowledge
        .checklist_incomplete(&body.path)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(ChecklistItemsResponse { items, incomplete }))
}

/// POST /api/knowledge/checklist/add — add a checklist item.
pub(crate) async fn handle_knowledge_checklist_add(
    state: State<Arc<AppState>>,
    Json(body): Json<ChecklistAddBody>,
) -> Result<StatusCode, AppError> {
    state
        .kernel
        .knowledge
        .checklist_add(&body.path, &body.item, body.checked)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/knowledge/checklist/complete — complete a checklist item.
pub(crate) async fn handle_knowledge_checklist_complete(
    state: State<Arc<AppState>>,
    Json(body): Json<ChecklistCompleteBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let found = state
        .kernel
        .knowledge
        .checklist_complete(&body.path, &body.item_hash)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "found": found })))
}

/// POST /api/knowledge/checklist/remove — remove a checklist item.
pub(crate) async fn handle_knowledge_checklist_remove(
    state: State<Arc<AppState>>,
    Json(body): Json<ChecklistRemoveBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let found = state
        .kernel
        .knowledge
        .checklist_remove(&body.path, &body.item_or_hash)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "found": found })))
}

// ---------------------------------------------------------------------------
// Chat handlers
// ---------------------------------------------------------------------------

/// POST /api/knowledge/chat/append — append a chat message.
pub(crate) async fn handle_knowledge_chat_append(
    state: State<Arc<AppState>>,
    Json(body): Json<ChatAppendBody>,
) -> Result<StatusCode, AppError> {
    state
        .kernel
        .knowledge
        .chat_append(&body.message)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/knowledge/chat/messages — list chat messages.
pub(crate) async fn handle_knowledge_chat_messages(
    state: State<Arc<AppState>>,
) -> Result<Json<Vec<String>>, AppError> {
    let messages = state
        .kernel
        .knowledge
        .chat_messages()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(messages))
}

/// POST /api/knowledge/chat/delete — delete a chat message.
pub(crate) async fn handle_knowledge_chat_delete(
    state: State<Arc<AppState>>,
    Json(body): Json<ChatDeleteBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let deleted = state
        .kernel
        .knowledge
        .chat_delete(&body.msg_hash)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "deleted": deleted })))
}

/// POST /api/knowledge/chat/move — move a chat message to a file.
pub(crate) async fn handle_knowledge_chat_move(
    state: State<Arc<AppState>>,
    Json(body): Json<ChatMoveBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let moved = state
        .kernel
        .knowledge
        .chat_move_to(&body.msg_hash, &body.target_path)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "moved": moved })))
}

// ---------------------------------------------------------------------------
// Journal handlers
// ---------------------------------------------------------------------------

/// POST /api/knowledge/journal/add — add a journal record.
pub(crate) async fn handle_knowledge_journal_add(
    state: State<Arc<AppState>>,
    Json(body): Json<JournalAddRecordBody>,
) -> Result<StatusCode, AppError> {
    state
        .kernel
        .knowledge
        .journal_add_record(&body.record)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/knowledge/journal/emoji — add a journal emoji.
pub(crate) async fn handle_knowledge_journal_emoji(
    state: State<Arc<AppState>>,
    Json(body): Json<JournalAddEmojiBody>,
) -> Result<StatusCode, AppError> {
    state
        .kernel
        .knowledge
        .journal_add_emoji(&body.emoji)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/knowledge/journal/today — get today's journal path.
pub(crate) async fn handle_knowledge_journal_today(
    state: State<Arc<AppState>>,
) -> Result<Json<JournalTodayResponse>, AppError> {
    let path = state.knowledge.journal_today_path();
    Ok(Json(JournalTodayResponse { path }))
}

// ---------------------------------------------------------------------------
// Habits handlers
// ---------------------------------------------------------------------------

/// GET /api/knowledge/habits — get habits for a year.
pub(crate) async fn handle_knowledge_habits(
    state: State<Arc<AppState>>,
    Query(params): Query<HabitsParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let year = params.year.unwrap_or_else(|| chrono::Local::now().year());
    let habits = state
        .kernel
        .knowledge
        .habits(year)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::to_value(habits).unwrap_or_default()))
}

/// GET /api/knowledge/habits/last-week — get last week's habits.
pub(crate) async fn handle_knowledge_habits_last_week(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let habits = state
        .kernel
        .knowledge
        .habits_last_week()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::to_value(habits).unwrap_or_default()))
}

// ---------------------------------------------------------------------------
// Stats handlers
// ---------------------------------------------------------------------------

/// GET /api/knowledge/stats/today — today's completion report.
pub(crate) async fn handle_knowledge_stats_today(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let report = state
        .kernel
        .knowledge
        .today_report()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::to_value(report).unwrap_or_default()))
}

/// GET /api/knowledge/stats/done-today — files completed today.
pub(crate) async fn handle_knowledge_stats_done_today(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let entries = state
        .kernel
        .knowledge
        .done_today()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "items": entries,
        "count": entries.len(),
    })))
}

// ---------------------------------------------------------------------------
// Config handlers
// ---------------------------------------------------------------------------

/// GET /api/knowledge/config — read knowledge config.
pub(crate) async fn handle_knowledge_config_get(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state
        .kernel
        .knowledge
        .config()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::to_value(config).unwrap_or_default()))
}

/// PUT /api/knowledge/config — update knowledge config.
pub(crate) async fn handle_knowledge_config_put(
    state: State<Arc<AppState>>,
    Json(body): Json<KnowledgeConfigBody>,
) -> Result<StatusCode, AppError> {
    let mut config = state
        .kernel
        .knowledge
        .config()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Merge non-null fields
    if let Some(v) = body.language { config.language = v; }
    if let Some(v) = body.timezone { config.timezone = v; }
    if let Some(v) = body.move_to_commands { config.move_to_commands = v; }
    if let Some(v) = body.pomodoro_duration_in_minutes { config.pomodoro_duration_in_minutes = v; }
    if let Some(v) = body.schedules {
        config.schedules = v.into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
    }
    if let Some(v) = body.quick_commands { config.quick_commands = v; }
    if let Some(v) = body.two_emojis_enabled { config.two_emojis_enabled = v; }
    if let Some(v) = body.mode { config.mode = v; }
    if let Some(v) = body.quick_habits_enabled { config.quick_habits_enabled = v; }
    if let Some(v) = body.channels { config.channels = v; }

    state
        .kernel
        .knowledge
        .set_config(&config)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Worker handlers
// ---------------------------------------------------------------------------

/// POST /api/knowledge/worker/nightly — run nightly cleanup.
pub(crate) async fn handle_knowledge_worker_nightly(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let report = state
        .kernel
        .knowledge
        .run_nightly_cleanup()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::to_value(report).unwrap_or_default()))
}

/// POST /api/knowledge/worker/scheduled — run scheduled tasks.
pub(crate) async fn handle_knowledge_worker_scheduled(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let moved = state
        .kernel
        .knowledge
        .run_scheduled_tasks()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "moved": moved,
        "count": moved.len(),
    })))
}

// ---------------------------------------------------------------------------
// Convert handler
// ---------------------------------------------------------------------------

/// POST /api/knowledge/convert/html — convert markdown to HTML.
pub(crate) async fn handle_knowledge_convert_html(
    state: State<Arc<AppState>>,
    Json(body): Json<ConvertHtmlBody>,
) -> Result<Json<ConvertHtmlResponse>, AppError> {
    let html = state.knowledge.markdown_to_html(&body.md);
    Ok(Json(ConvertHtmlResponse { html }))
}

// ---------------------------------------------------------------------------
// Emoji handler
// ---------------------------------------------------------------------------

/// GET /api/knowledge/emoji — find emoji for text.
pub(crate) async fn handle_knowledge_emoji(
    state: State<Arc<AppState>>,
    Query(params): Query<EmojiQueryParams>,
) -> Result<Json<EmojiResponse>, AppError> {
    let emoji = state.knowledge.auto_emoji(&params.text);
    Ok(Json(EmojiResponse { emoji }))
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
