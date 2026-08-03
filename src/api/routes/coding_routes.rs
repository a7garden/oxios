//! Code Workspace API routes — `/api/code/*`.
//!
//! Session management, host filesystem operations, change tracking,
//! checkpoints, and PTY terminal management for the Code Workspace tab.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use oxios_kernel::code::{Checkpoint, CodeSession, FileChange};
use oxios_kernel::kernel_handle::coding_api::{DirEntry, FileContent};
use serde::{Deserialize, Serialize};

use crate::api::error::AppError;
use crate::api::server::AppState;

// ── Request/Response types ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct CreateSessionRequest {
    pub project_path: String,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BrowseQuery {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReadFileQuery {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WriteFileQuery {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateFileRequest {
    pub path: String,
    #[serde(default)]
    pub is_dir: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MoveFileRequest {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateTerminalRequest {
    #[serde(default)]
    pub shell: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateCheckpointRequest {
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct SessionResponse {
    pub session: CodeSession,
    pub pending_changes: usize,
    pub checkpoints: Vec<Checkpoint>,
    pub git_branch: Option<String>,
}

// ── Session handlers ─────────────────────────────────────────────────

/// POST /api/code/sessions — Create a coding session.
pub(crate) async fn handle_code_session_create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<CodeSession>, AppError> {
    let project_path = PathBuf::from(&req.project_path);
    let session = state
        .kernel
        .coding
        .create_session(project_path, req.model)
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    Ok(Json(session))
}

/// GET /api/code/sessions — List all coding sessions.
pub(crate) async fn handle_code_sessions_list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<CodeSession>>, AppError> {
    Ok(Json(state.kernel.coding.list_sessions()))
}

/// GET /api/code/sessions/:id — Get session details.
pub(crate) async fn handle_code_session_get(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<SessionResponse>, AppError> {
    let session_state = state
        .kernel
        .coding
        .get_session(&id)
        .ok_or_else(|| AppError::NotFound(format!("session not found: {id}")))?;

    let pending_changes = session_state.changes.read().await.pending_count();
    let checkpoints: Vec<Checkpoint> = session_state
        .checkpoints
        .read()
        .await
        .list()
        .into_iter()
        .cloned()
        .collect();

    Ok(Json(SessionResponse {
        session: session_state.session.clone(),
        pending_changes,
        checkpoints,
        git_branch: session_state.git_branch.clone(),
    }))
}

/// DELETE /api/code/sessions/:id — Delete a coding session.
pub(crate) async fn handle_code_session_delete(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, AppError> {
    if !state.kernel.coding.delete_session(&id) {
        return Err(AppError::NotFound(format!("session not found: {id}")));
    }
    Ok(StatusCode::NO_CONTENT)
}

// ── File operations ──────────────────────────────────────────────────

/// GET /api/code/fs/browse — Browse a directory on the host filesystem.
pub(crate) async fn handle_code_fs_browse(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BrowseQuery>,
) -> Result<Json<Vec<DirEntry>>, AppError> {
    let path = PathBuf::from(&query.path);
    let entries = state
        .kernel
        .coding
        .browse_dir(&path)
        .map_err(|e| AppError::NotFound(e.to_string()))?;
    Ok(Json(entries))
}

/// GET /api/code/fs/read — Read a file from the host filesystem.
pub(crate) async fn handle_code_fs_read(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ReadFileQuery>,
) -> Result<Json<FileContent>, AppError> {
    let path = PathBuf::from(&query.path);
    let content = state
        .kernel
        .coding
        .read_file(&path)
        .map_err(|e| AppError::NotFound(e.to_string()))?;
    Ok(Json(content))
}

/// PUT /api/code/fs/write — Write file content (raw body).
pub(crate) async fn handle_code_fs_write(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WriteFileQuery>,
    body: String,
) -> Result<StatusCode, AppError> {
    let path = PathBuf::from(&query.path);
    state
        .kernel
        .coding
        .write_file(&path, &body)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(StatusCode::OK)
}

/// POST /api/code/fs/create — Create a file or directory.
pub(crate) async fn handle_code_fs_create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateFileRequest>,
) -> Result<StatusCode, AppError> {
    let path = PathBuf::from(&req.path);
    state
        .kernel
        .coding
        .create_file_or_dir(&path, req.is_dir)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(StatusCode::CREATED)
}

/// DELETE /api/code/fs/delete — Delete a file or directory.
pub(crate) async fn handle_code_fs_delete(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ReadFileQuery>,
) -> Result<StatusCode, AppError> {
    let path = PathBuf::from(&query.path);
    state
        .kernel
        .coding
        .delete_path(&path)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/code/fs/move — Move or rename a file/directory.
pub(crate) async fn handle_code_fs_move(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MoveFileRequest>,
) -> Result<StatusCode, AppError> {
    let from = PathBuf::from(&req.from);
    let to = PathBuf::from(&req.to);
    state
        .kernel
        .coding
        .move_path(&from, &to)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(StatusCode::OK)
}

// ── File search ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct FsSearchQuery {
    pub path: String,
    pub q: String,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct FsSearchResult {
    pub file: String,
    pub line: usize,
    pub text: String,
}

/// GET /api/code/fs/search — Search file contents recursively.
/// Uses `grep -rn` with common ignore patterns (.git, node_modules, target).
pub(crate) async fn handle_code_fs_search(
    Query(query): Query<FsSearchQuery>,
) -> Result<Json<Vec<FsSearchResult>>, AppError> {
    let limit = query.limit.unwrap_or(100);
    let output = std::process::Command::new("grep")
        .args([
            "-rnI",
            "--exclude-dir=.git",
            "--exclude-dir=node_modules",
            "--exclude-dir=target",
            "--exclude-dir=.next",
            "--exclude-dir=dist",
            &query.q,
            &query.path,
        ])
        .output()
        .map_err(|e| AppError::Internal(format!("search failed: {e}")))?;

    let results: Vec<FsSearchResult> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .take(limit)
        .filter_map(|line| {
            let mut parts = line.splitn(3, ':');
            let file = parts.next()?.to_string();
            let line_num: usize = parts.next()?.parse().ok()?;
            let text = parts.next()?.to_string();
            Some(FsSearchResult {
                file,
                line: line_num,
                text,
            })
        })
        .collect();

    Ok(Json(results))
}

/// GET /api/code/fs/list — List all files recursively (for quick open).
/// Uses `find` with common ignore patterns. Capped at 1000 results.
pub(crate) async fn handle_code_fs_list(
    Query(query): Query<BrowseQuery>,
) -> Result<Json<Vec<String>>, AppError> {
    let output = std::process::Command::new("find")
        .args([
            &query.path,
            "-type", "f",
            "-not", "-path", "*/.git/*",
            "-not", "-path", "*/node_modules/*",
            "-not", "-path", "*/target/*",
            "-not", "-path", "*/dist/*",
            "-not", "-path", "*/.next/*",
            "-not", "-path", "*/__pycache__/*",
        ])
        .output()
        .map_err(|e| AppError::Internal(format!("list failed: {e}")))?;

    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .take(1000)
        .map(|s| s.to_string())
        .collect();

    Ok(Json(files))
}

// ── Changes ──────────────────────────────────────────────────────────

/// GET /api/code/sessions/:id/changes — List pending file changes.
pub(crate) async fn handle_code_changes_list(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Vec<FileChange>>, AppError> {
    let session_state = state
        .kernel
        .coding
        .get_session(&id)
        .ok_or_else(|| AppError::NotFound("session not found".into()))?;
    let changes: Vec<FileChange> =
        session_state.changes.read().await.list_changes().into_iter().cloned().collect();
    Ok(Json(changes))
}

/// POST /api/code/sessions/:id/changes/accept-all
pub(crate) async fn handle_code_changes_accept_all(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, AppError> {
    let session_state = state
        .kernel
        .coding
        .get_session(&id)
        .ok_or_else(|| AppError::NotFound("session not found".into()))?;
    session_state.changes.write().await.accept_all();
    Ok(StatusCode::OK)
}

/// POST /api/code/sessions/:id/changes/reject-all
pub(crate) async fn handle_code_changes_reject_all(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, AppError> {
    let session_state = state
        .kernel
        .coding
        .get_session(&id)
        .ok_or_else(|| AppError::NotFound("session not found".into()))?;
    session_state
        .changes
        .write()
        .await
        .reject_all()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(StatusCode::OK)
}

// ── Checkpoints ──────────────────────────────────────────────────────

/// POST /api/code/sessions/:id/checkpoint — Create a checkpoint.
pub(crate) async fn handle_code_checkpoint_create(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<CreateCheckpointRequest>,
) -> Result<Json<Checkpoint>, AppError> {
    let session_state = state
        .kernel
        .coding
        .get_session(&id)
        .ok_or_else(|| AppError::NotFound("session not found".into()))?;

    // Snapshot all files currently tracked by the ChangeTracker.
    let paths: Vec<PathBuf> = session_state
        .changes
        .read()
        .await
        .list_changes()
        .iter()
        .map(|c| c.path.clone())
        .collect();

    let checkpoint = session_state
        .checkpoints
        .write()
        .await
        .create(&req.description, &paths)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(checkpoint))
}

/// GET /api/code/sessions/:id/checkpoints — List checkpoints.
pub(crate) async fn handle_code_checkpoints_list(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Vec<Checkpoint>>, AppError> {
    let session_state = state
        .kernel
        .coding
        .get_session(&id)
        .ok_or_else(|| AppError::NotFound("session not found".into()))?;
    let checkpoints: Vec<Checkpoint> = session_state
        .checkpoints
        .read()
        .await
        .list()
        .into_iter()
        .cloned()
        .collect();
    Ok(Json(checkpoints))
}

/// POST /api/code/sessions/:id/checkpoints/:cp/revert
pub(crate) async fn handle_code_checkpoint_revert(
    State(state): State<Arc<AppState>>,
    AxumPath((id, cp)): AxumPath<(String, String)>,
) -> Result<StatusCode, AppError> {
    let session_state = state
        .kernel
        .coding
        .get_session(&id)
        .ok_or_else(|| AppError::NotFound("session not found".into()))?;
    session_state
        .checkpoints
        .read()
        .await
        .revert_to(&cp)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(StatusCode::OK)
}

// ── Terminal ─────────────────────────────────────────────────────────

/// POST /api/code/sessions/:id/terminal — Create a new PTY terminal.
pub(crate) async fn handle_code_terminal_create(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<CreateTerminalRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let session_state = state
        .kernel
        .coding
        .get_session(&id)
        .ok_or_else(|| AppError::NotFound("session not found".into()))?;

    let terminal_id = state
        .kernel
        .coding
        .pty_manager()
        .create(&session_state.session.project_path, req.shell.as_deref())
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "terminal_id": terminal_id })))
}

/// DELETE /api/code/terminal/:tid — Close a terminal.
pub(crate) async fn handle_code_terminal_delete(
    State(state): State<Arc<AppState>>,
    AxumPath(tid): AxumPath<String>,
) -> Result<StatusCode, AppError> {
    state
        .kernel
        .coding
        .pty_manager()
        .kill(&tid)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/code/terminal/:tid — WebSocket upgrade for bidirectional terminal I/O.
pub(crate) async fn handle_code_terminal_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    AxumPath(tid): AxumPath<String>,
) -> Result<impl IntoResponse, AppError> {
    let pty = Arc::clone(state.kernel.coding.pty_manager());

    // Subscribe to output before accepting the WebSocket.
    let mut rx = pty
        .subscribe(&tid)
        .await
        .map_err(|_| AppError::NotFound("terminal not found".into()))?;

    Ok(ws.on_upgrade(move |socket: WebSocket| async move {
        let (ws_sink, mut ws_stream) = socket.split();

        use futures_util::{SinkExt, StreamExt};

        let ws_sink = Arc::new(tokio::sync::Mutex::new(ws_sink));

        // PTY → WebSocket relay
        let ws_sink_clone = Arc::clone(&ws_sink);
        let relay_out = tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(data) => {
                        if let Ok(text) = String::from_utf8(data.to_vec()) {
                            let msg = Message::Text(text.into());
                            if ws_sink_clone.lock().await.send(msg).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
        });

        // WebSocket → PTY relay
        let relay_in = tokio::spawn(async move {
            while let Some(Ok(msg)) = ws_stream.next().await {
                match msg {
                    Message::Text(text) => {
                        // Distinguish control messages (JSON with a "type"
                        // field) from raw terminal input. The frontend sends
                        // keystrokes as plain text and resize/input commands
                        // as JSON objects.
                        let trimmed = text.trim_start();
                        if trimmed.starts_with('{')
                            && let Ok(ctrl) =
                                serde_json::from_str::<serde_json::Value>(&text)
                        {
                            match ctrl.get("type").and_then(|t| t.as_str()) {
                                Some("resize") => {
                                    let cols = ctrl
                                        .get("cols")
                                        .and_then(|c| c.as_u64())
                                        .unwrap_or(80) as u16;
                                    let rows = ctrl
                                        .get("rows")
                                        .and_then(|r| r.as_u64())
                                        .unwrap_or(24) as u16;
                                    let _ = pty.resize(&tid, rows, cols).await;
                                    continue;
                                }
                                Some("input") => {
                                    if let Some(data) =
                                        ctrl.get("data").and_then(|d| d.as_str())
                                    {
                                        let _ = pty.write(&tid, data.as_bytes()).await;
                                    }
                                    continue;
                                }
                                _ => {} // fall through to raw write
                            }
                        }
                        let _ = pty.write(&tid, text.as_bytes()).await;
                    }
                    Message::Binary(data) => {
                        let _ = pty.write(&tid, &data).await;
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            // Clean up terminal when WebSocket closes
            let _ = pty.kill(&tid).await;
        });

        let _ = tokio::join!(relay_out, relay_in);
    }))
}

// ── Agent messaging ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct CodeMessageRequest {
    pub content: String,
    #[serde(default)]
    pub context_files: Vec<String>,
}

/// POST /api/code/sessions/:id/message — Send a message to the coding agent.
///
/// Builds an IncomingMessage with persona="code", cspace_hint="coder",
/// and workspace_dir set to the session's project path. Sends via the
/// gateway bridge (same path as /api/chat).
pub(crate) async fn handle_code_message(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<CodeMessageRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let session_state = state
        .kernel
        .coding
        .get_session(&id)
        .ok_or_else(|| AppError::NotFound("session not found".into()))?;

    let project_path = session_state.session.project_path.clone();
    let project_path_str = project_path.to_string_lossy().to_string();

    // ── Fix 1: Activate the "code" persona ──────────────────────────
    // The agent runtime reads persona_manager.active_system_prompt() to
    // build the system prompt. Without this, the agent runs with whatever
    // persona was previously active (likely the default), NOT the coding-
    // specialized prompt. set_active persists to disk + re-seeds intent.
    if state.kernel.persona.get("code").is_some() {
        if let Err(e) = state.kernel.persona.set_active("code").await {
            tracing::warn!("Failed to activate 'code' persona: {e}");
        }
    } else {
        tracing::warn!("'code' persona not found in store — agent will use default persona");
    }

    // ── Snapshot pre-turn file contents ──────────────────────────────
    // Capture the content of every git-modified file BEFORE the agent
    // runs. After the turn we compare to detect which files the agent
    // actually changed — including re-edits of already-modified files.
    // For files new to this turn, the original comes from `git show
    // HEAD:path`.
    let pre_snapshot: HashMap<String, String> = git_changed_files(&project_path)
        .iter()
        .filter_map(|f| {
            let abs = project_path.join(&f.path);
            std::fs::read_to_string(&abs).ok().map(|c| (f.path.clone(), c))
        })
        .collect();

    // Build the incoming message with coding-specific metadata.
    let mut msg = oxios_gateway::message::IncomingMessage::new("web", "code-user", &req.content);
    msg.metadata.insert("session_id".to_owned(), id.clone());
    msg.metadata.insert("persona_role".to_owned(), "code".to_owned());
    msg.metadata.insert("cspace_hint".to_owned(), "coder".to_owned());
    msg.metadata.insert("workspace_dir".to_owned(), project_path_str);

    if let Some(model) = &session_state.session.model {
        msg.metadata.insert("model_override".to_owned(), model.clone());
    }
    if !req.context_files.is_empty() {
        msg.metadata
            .insert("context_files".to_owned(), req.context_files.join(","));
    }

    // Send through the gateway bridge.
    let result = state.bridge.send_and_wait(msg).await;

    // ── Detect changes AFTER agent turn ──────────────────────────────
    // For every file git reports as changed, compare its current content
    // to the pre-turn snapshot. If it differs (or the file is brand-new),
    // record it with the correct original baseline so the diff is accurate
    // and "reject" actually reverts to the pre-edit state.
    let post_changes = git_changed_files(&project_path);
    if !post_changes.is_empty() {
        let mut tracker = session_state.changes.write().await;
        let mut detected = 0;
        for change in &post_changes {
            let abs_path = project_path.join(&change.path);
            let new_content = if abs_path.exists() {
                std::fs::read_to_string(&abs_path).ok()
            } else {
                // File was deleted during the turn.
                None
            };

            // Determine the true pre-edit baseline:
            // 1. If the file was already modified before the turn, use the
            //    pre-turn snapshot (not HEAD — the user may have had their
            //    own changes that we must preserve on reject).
            // 2. If the file is new this turn, use HEAD content (or None
            //    for untracked/created files).
            let original = pre_snapshot
                .get(&change.path)
                .cloned()
                .or_else(|| git_show_head(&project_path, &change.path));

            // Skip files whose content didn't actually change during the
            // turn (pre-snapshot identical to current disk content).
            if let (Some(orig), Some(new)) = (&original, &new_content)
                && orig == new
            {
                continue;
            }

            tracker.record_write_with_original(
                &abs_path,
                original.as_deref(),
                new_content.as_deref().unwrap_or(""),
                None,
            )?;
            detected += 1;
        }
        if detected > 0 {
            tracing::info!("Detected {detected} file changes from agent turn");
        }
    }

    match result {
        Ok(response) => Ok(Json(serde_json::json!({
            "reply": response.content,
            "session_id": id,
        }))),
        Err(e) => {
            tracing::error!("Code agent message failed: {e}");
            Err(AppError::Internal(format!("agent error: {e}")))
        }
    }
}

// ── Git change detection helpers ─────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct GitChangedFile {
    path: String,
    status: char, // 'M' = modified, 'A'/'?' = added/untracked, 'D' = deleted
}

/// Get all changed files in a git repo (modified, untracked, deleted).
fn git_changed_files(project_root: &std::path::Path) -> Vec<GitChangedFile> {
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(project_root)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|line| {
                    if line.len() < 4 {
                        return None;
                    }
                    let status = line.chars().next().unwrap_or(' ');
                    let path = line[3..].trim().to_string();
                    if path.is_empty() {
                        return None;
                    }
                    Some(GitChangedFile { path, status })
                })
                .collect()
        }
        _ => Vec::new(), // Not a git repo or git unavailable
    }
}

/// Get the original content of a file from HEAD (before any changes).
fn git_show_head(project_root: &std::path::Path, path: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["show", &format!("HEAD:{path}")])
        .current_dir(project_root)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None // File doesn't exist in HEAD (newly created)
    }
}
