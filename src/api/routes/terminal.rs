//! HTTP/WebSocket routes for the Interactive Terminal (RFC-038).
//!
//! - `POST /api/terminal/ticket` — issue a one-time WS ticket.
//! - `GET  /api/terminal/stream` — upgrade to WebSocket, attach to PTY.
//! - `GET  /api/terminal/sessions` — list active sessions for the caller.
//! - `POST /api/terminal/pty/start` — spawn a new PTY session.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use futures_util::{SinkExt, StreamExt as FuturesStreamExt};
use serde::{Deserialize, Serialize};

use oxios_kernel::pty::PtySize;
use oxios_kernel::PtyError;

use crate::api::error::AppError;
use crate::api::server::AppState;

// ── Auth ticket ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct WsParams {
    ticket: Option<String>,
    token: Option<String>,
}

/// POST /api/terminal/ticket — issue one-time WS ticket.
pub(crate) async fn handle_terminal_ticket(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let ticket = state.kernel.security.generate_ws_ticket();
    Ok(Json(serde_json::json!({ "ticket": ticket })))
}

// ── Spawn session ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SpawnRequest {
    #[serde(default)]
    pub principal: Option<String>,
    #[serde(default)]
    pub shell: Option<String>,
    #[serde(default = "default_cols")]
    pub cols: u16,
    #[serde(default = "default_rows")]
    pub rows: u16,
}

fn default_cols() -> u16 {
    80
}
fn default_rows() -> u16 {
    24
}

#[derive(Debug, Serialize)]
pub struct SpawnResponse {
    pub session_id: String,
    pub shell: String,
    pub cols: u16,
    pub rows: u16,
}

/// POST /api/terminal/pty/start — spawn a new PTY session.
pub(crate) async fn handle_pty_start(
    state: State<Arc<AppState>>,
    Json(body): Json<SpawnRequest>,
) -> Result<Json<SpawnResponse>, AppError> {
    let principal = body
        .principal
        .unwrap_or_else(|| "default".to_string());
    let size = PtySize {
        cols: body.cols,
        rows: body.rows,
        pixel_width: 0,
        pixel_height: 0,
    };
    let session_id = state
        .kernel
        .pty
        .open(&principal, body.shell.clone(), size)
        .map_err(map_pty_err)?;
    // Get the resolved shell from the session list.
    let info = state
        .kernel
        .pty
        .list_sessions(&principal)
        .into_iter()
        .find(|s| s.id == session_id);
    let shell = info.map(|i| i.shell).unwrap_or_default();
    Ok(Json(SpawnResponse {
        session_id,
        shell,
        cols: body.cols,
        rows: body.rows,
    }))
}

// ── List sessions ────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SessionsResponse {
    pub sessions: Vec<oxios_kernel::pty::PtySessionInfo>,
}

/// GET /api/terminal/sessions — list active sessions for the caller.
pub(crate) async fn handle_pty_sessions(
    state: State<Arc<AppState>>,
) -> Result<Json<SessionsResponse>, AppError> {
    // In single-user local-first deployment, list all sessions.
    let mut all = Vec::new();
    let principals = vec!["default".to_string()];
    for p in principals {
        all.extend(state.kernel.pty.list_sessions(&p));
    }
    Ok(Json(SessionsResponse { sessions: all }))
}

// ── WebSocket upgrade ────────────────────────────────────────────────

/// GET /api/terminal/stream?ticket=... — upgrade to WebSocket.
pub(crate) async fn handle_terminal_stream(
    ws: WebSocketUpgrade,
    state: State<Arc<AppState>>,
    Query(params): Query<WsParams>,
) -> impl IntoResponse {
    if state.config.read().security.auth_enabled {
        let authed = if let Some(ref t) = params.ticket {
            state.kernel.security.validate_ws_ticket(t)
        } else if let Some(ref t) = params.token {
            state.kernel.security.validate_token(t)
        } else {
            false
        };
        if !authed {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
    ws.on_upgrade(move |socket| handle_terminal_websocket(socket, state.0))
}

// ── WS control protocol ──────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TerminalControl {
    Open {
        session_id: Option<String>,
        shell: Option<String>,
        cols: u16,
        rows: u16,
    },
    Opened {
        session_id: String,
        shell: String,
        cols: u16,
        rows: u16,
    },
    Resize { cols: u16, rows: u16 },
    Close { reason: Option<String> },
    Exit {
        code: Option<i32>,
        signal: Option<i32>,
    },
    Error { message: String },
}

/// Per-WS-connection: spawn session (or attach), then bridge PTY ↔ WS bytes.
async fn handle_terminal_websocket(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let mut session_id: Option<String> = None;
    let mut principal: String = "default".to_string();

    // First frame must be Open.
    let open_outcome = async {
        let Some(msg) = ws_rx.next().await else {
            return Err("ws closed before open".to_string());
        };
        let msg = msg.map_err(|e| format!("ws recv: {e}"))?;
        let text = match msg {
            Message::Text(t) => t,
            _ => return Err("first frame must be text Open".to_string()),
        };
        let open: TerminalControl = serde_json::from_str(&text)
            .map_err(|e| format!("invalid open frame: {e}"))?;
        match open {
            TerminalControl::Open {
                session_id: sid,
                shell,
                cols,
                rows,
            } => {
                let size = PtySize {
                    cols,
                    rows,
                    pixel_width: 0,
                    pixel_height: 0,
                };
                if let Some(sid) = sid {
                    // Re-attach.
                    state
                        .kernel
                        .pty
                        .attach(&principal, &sid)
                        .map_err(|e| format!("attach: {e}"))?;
                    state.kernel.pty.mark_attached(&sid);
                    Ok((sid, size))
                } else {
                    let new_id = state
                        .kernel
                        .pty
                        .open(&principal, shell, size)
                        .map_err(|e| format!("open: {e}"))?;
                    state.kernel.pty.mark_attached(&new_id);
                    Ok((new_id, size))
                }
            }
            _ => Err("expected Open".to_string()),
        }
    }
    .await;

    let (sid, _size) = match open_outcome {
        Ok(v) => v,
        Err(e) => {
            let _ = ws_tx
                .send(Message::Text(
                    serde_json::to_string(&TerminalControl::Error { message: e }).unwrap(),
                ))
                .await;
            return;
        }
    };
    session_id = Some(sid.clone());

    // Send Opened.
    let info = state
        .kernel
        .pty
        .list_sessions(&principal)
        .into_iter()
        .find(|s| s.id == sid);
    let opened = TerminalControl::Opened {
        session_id: sid.clone(),
        shell: info.as_ref().map(|i| i.shell.clone()).unwrap_or_default(),
        cols: info.as_ref().map(|i| i.cols).unwrap_or(80),
        rows: info.as_ref().map(|i| i.rows).unwrap_or(24),
    };
    if ws_tx
        .send(Message::Text(serde_json::to_string(&opened).unwrap()))
        .await
        .is_err()
    {
        return;
    }

    // Start PTY reader task: master → WS.
    let reader = match state.kernel.pty.try_clone_reader(&sid) {
        Ok(r) => r,
        Err(e) => {
            let _ = ws_tx
                .send(Message::Text(
                    serde_json::to_string(&TerminalControl::Error {
                        message: format!("reader: {e}"),
                    })
                    .unwrap(),
                ))
                .await;
            return;
        }
    };
    let mut reader = reader;
    let (pty_tx, mut pty_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);
    let read_task = tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 4096];
        loop {
            use std::io::Read;
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if pty_tx.blocking_send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    let mut send_task = {
        let mut ws_tx = ws_tx;
        tokio::spawn(async move {
            while let Some(bytes) = pty_rx.recv().await {
                if ws_tx.send(Message::Binary(bytes)).await.is_err() {
                    break;
                }
            }
            let _ = ws_tx
                .send(Message::Text(
                    serde_json::to_string(&TerminalControl::Exit {
                        code: None,
                        signal: None,
                    })
                    .unwrap(),
                ))
                .await;
        })
    };

    // Receive loop: WS → stdin (binary) + control frames (text).
    while let Some(msg) = ws_rx.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(_) => break,
        };
        match msg {
            Message::Binary(bytes) => {
                if let Some(sid) = &session_id {
                    let _ = state.kernel.pty.write(sid, &bytes);
                }
            }
            Message::Text(text) => {
                if let Ok(ctrl) = serde_json::from_str::<TerminalControl>(&text) {
                    match ctrl {
                        TerminalControl::Resize { cols, rows } => {
                            if let Some(sid) = &session_id {
                                let _ = state.kernel.pty.resize(sid, cols, rows);
                            }
                        }
                        TerminalControl::Close { .. } => break,
                        _ => {}
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Cleanup.
    if let Some(sid) = &session_id {
        let _ = state.kernel.pty.mark_detached(sid);
    }
    read_task.abort();
    send_task.abort();
}

// ── Error mapping ────────────────────────────────────────────────────

fn map_pty_err(e: PtyError) -> AppError {
    match e {
        PtyError::Disabled => AppError::BadRequest("pty disabled".into()),
        PtyError::SessionCapReached { .. } => AppError::BadRequest("session cap reached".into()),
        PtyError::ShellNotAllowed { shell } => {
            AppError::BadRequest(format!("shell not allowed: {shell}"))
        }
        PtyError::NotFound(id) => AppError::NotFound(format!("session {id} not found")),
        PtyError::NotOwner(id) => AppError::Forbidden(format!("session {id} not owned")),
        PtyError::Closed(id) => AppError::BadRequest(format!("session {id} closed")),
        other => AppError::Internal(format!("pty: {other}")),
    }
}