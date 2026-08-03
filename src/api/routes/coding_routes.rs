//! Code Workspace API routes — `/api/code/*`.
//!
//! Session management, host filesystem operations, change tracking,
//! checkpoints, and PTY terminal management for the Code Workspace tab.

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
