use std::sync::Arc;

use axum::extract::Query;
use axum::extract::{
    ws::{Message, WebSocket},
    State, WebSocketUpgrade,
};
use axum::response::IntoResponse;
use axum::Json;
use futures_util::{SinkExt, StreamExt as FuturesStreamExt};
use serde::{Deserialize, Serialize};

use oxios_gateway::message::IncomingMessage;

use crate::error::AppError;
use crate::server::AppState;

// ---------------------------------------------------------------------------
// Chat
// ---------------------------------------------------------------------------

/// Request body for the chat endpoint.
#[derive(Debug, Deserialize)]
pub(crate) struct ChatRequest {
    /// The user's message content.
    #[serde(alias = "message")]
    content: String,
    /// Optional user identifier (defaults to "default").
    #[serde(default = "default_user")]
    user_id: String,
    /// Optional session ID for multi-turn conversations.
    #[serde(default)]
    session_id: String,
}

pub(crate) fn default_user() -> String {
    "default".into()
}

/// Response body for the chat endpoint.
#[derive(Debug, Serialize)]
pub(crate) struct ChatResponse {
    /// The message ID.
    id: String,
    /// Echo of the user's message.
    echo: String,
    /// The response from the orchestrator.
    reply: String,
    /// Session ID for multi-turn conversations.
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    /// Phase reached during orchestration.
    #[serde(skip_serializing_if = "Option::is_none")]
    phase: Option<String>,
}

/// POST /api/chat — Send a message to the kernel via gateway and get response.
pub(crate) async fn handle_chat(
    state: State<Arc<AppState>>,
    Json(body): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, AppError> {
    // Validate chat content size (max 64KB for user message)
    const MAX_CHAT_LENGTH: usize = 64 * 1024;
    if body.content.len() > MAX_CHAT_LENGTH {
        return Err(AppError::PayloadTooLarge {
            size: body.content.len(),
            limit: MAX_CHAT_LENGTH,
        });
    }

    tracing::info!(content = %body.content, user = %body.user_id, "Chat message received");

    // Build the incoming message.
    let mut msg = IncomingMessage::new("web", &body.user_id, &body.content);

    // Include session_id from request if provided (for multi-turn conversations).
    if !body.session_id.is_empty() {
        msg.metadata
            .insert("session_id".to_owned(), body.session_id.clone());
    }

    let msg_id = msg.id.to_string();
    let content_echo = body.content.clone();

    // Send and wait for response from the gateway pipeline.
    tracing::info!("Sending message to gateway...");
    match state.channel.send_and_wait(msg).await {
        Ok(response) => {
            tracing::info!(reply_len = response.content.len(), "Chat response received");

            // Persist session
            {
                let session_id_for_save = response
                    .metadata
                    .get("session_id")
                    .cloned()
                    .unwrap_or_else(|| msg_id.clone());
                let session_id = oxios_kernel::state_store::SessionId(session_id_for_save.clone());
                match state.kernel.state.load_session(&session_id).await {
                    Ok(Some(mut session)) => {
                        session.add_user_message(&content_echo);
                        session.add_agent_response(oxios_kernel::state_store::AgentResponse {
                            content: response.content.clone(),
                            session_id: Some(session_id.0),
                            seed_id: response.metadata.get("seed_id").cloned(),
                            phase_reached: response.metadata.get("phase").cloned(),
                            evaluation_passed: response
                                .metadata
                                .get("evaluation_passed")
                                .and_then(|v| v.parse().ok()),
                            timestamp: chrono::Utc::now(),
                        });
                        if let Err(e) = state.kernel.state.save_session(&session).await {
                            tracing::warn!(error = %e, "Failed to persist session");
                        }
                    }
                    Ok(None) => {
                        // Create new session
                        let mut session =
                            oxios_kernel::state_store::Session::new(body.user_id.clone());
                        session.id =
                            oxios_kernel::state_store::SessionId(session_id_for_save.clone());
                        session.add_user_message(&content_echo);
                        session.add_agent_response(oxios_kernel::state_store::AgentResponse {
                            content: response.content.clone(),
                            session_id: Some(session_id.0),
                            seed_id: response.metadata.get("seed_id").cloned(),
                            phase_reached: response.metadata.get("phase").cloned(),
                            evaluation_passed: response
                                .metadata
                                .get("evaluation_passed")
                                .and_then(|v| v.parse().ok()),
                            timestamp: chrono::Utc::now(),
                        });
                        if let Err(e) = state.kernel.state.save_session(&session).await {
                            tracing::warn!(error = %e, "Failed to create session");
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "Failed to load/create session"),
                }
            }

            Ok(Json(ChatResponse {
                id: msg_id,
                echo: content_echo,
                reply: response.content,
                session_id: response.metadata.get("session_id").cloned(),
                phase: response.metadata.get("phase").cloned(),
            }))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to get response from gateway");
            Err(AppError::Internal("gateway response failed".into()))
        }
    }
}

/// Query parameters for WebSocket connections.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct WsParams {
    /// Bearer token for authentication.
    token: Option<String>,
}

/// GET /api/chat/stream — WebSocket endpoint for real-time chat streaming.
pub(crate) async fn handle_chat_stream(
    ws: WebSocketUpgrade,
    state: State<Arc<AppState>>,
    Query(params): Query<WsParams>,
) -> impl axum::response::IntoResponse {
    // Authenticate if auth is enabled
    if state.config.read().security.auth_enabled {
        let token = params.token.as_deref().unwrap_or("");
        if !state.kernel.security.validate_token(token) {
            return axum::http::StatusCode::UNAUTHORIZED.into_response();
        }
    }
    ws.on_upgrade(move |socket| handle_chat_websocket(socket, state.0))
}

/// Handles a WebSocket connection for chat streaming.
///
/// Protocol:
/// - **Incoming** (frontend → backend):
///   `{ type: "message", content: "...", session_id?: "...", space_id?: "..." }`
/// - **Outgoing token** (backend → frontend):
///   `{ type: "token", content: "...", session_id?, space_id? }`
/// - **Outgoing done** (backend → frontend):
///   `{ type: "done", session_id?, space_id?, phase?, evaluation_passed? }`
pub(crate) async fn handle_chat_websocket(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Subscribe to outgoing messages from the web channel (not kernel event bus).
    // WebChannel::send() broadcasts OutgoingMessage here; the kernel event bus
    // carries KernelEvents which are a different type entirely.
    let mut outgoing_rx = state.channel.subscribe();

    // Clone handles for the spawned tasks.
    let incoming_tx = state.channel.incoming_tx.clone();
    let state_store = state.kernel.state.store().clone();

    // Track the last user message for session persistence.
    // The send_task sets this before forwarding to gateway;
    // the recv_task reads it when the response arrives.
    let pending_user_msg: Arc<tokio::sync::Mutex<Option<PendingMessage>>> =
        Arc::new(tokio::sync::Mutex::new(None));

    let _pending_for_recv = pending_user_msg.clone();
    let pending_for_send = pending_user_msg.clone();

    // ── Forward gateway responses → WebSocket client ──
    //
    // Each chunk carries session_id + space_id so the frontend can
    // maintain multi-turn context. After the "done" chunk we persist
    // the session to disk (same as the POST handler).
    let recv_task = tokio::spawn(async move {
        while let Ok(msg) = outgoing_rx.recv().await {
            let session_id = msg.metadata.get("session_id").cloned();
            let space_id = msg.metadata.get("space_id").cloned();
            let phase = msg.metadata.get("phase").cloned();
            let evaluation_passed = msg.metadata.get("evaluation_passed").cloned();

            // Send the response content as a "token" chunk with metadata
            let token_chunk = serde_json::json!({
                "type": "token",
                "content": msg.content,
                "session_id": session_id,
                "space_id": space_id,
            });
            let json = match serde_json::to_string(&token_chunk) {
                Ok(j) => j,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to serialize outgoing message");
                    continue;
                }
            };
            if ws_tx.send(Message::Text(json.into())).await.is_err() {
                break;
            }

            // Send "done" chunk with final metadata
            let done_chunk = serde_json::json!({
                "type": "done",
                "session_id": session_id,
                "space_id": space_id,
                "phase": phase,
                "evaluation_passed": evaluation_passed,
            });
            let done_json = match serde_json::to_string(&done_chunk) {
                Ok(j) => j,
                Err(_) => break,
            };
            if ws_tx.send(Message::Text(done_json.into())).await.is_err() {
                break;
            }

            // ── Persist session to disk ──
            // Uses the same logic as the POST /api/chat handler.
            if let Some(ref sid) = session_id {
                let user_msg = pending_user_msg.lock().await.take();
                if let Some(pm) = user_msg {
                    persist_session(
                        &state_store,
                        sid,
                        pm.content.as_str(),
                        pm.user_id.as_str(),
                        &msg.content,
                        space_id.as_deref(),
                        &msg.metadata,
                    )
                    .await;
                }
            }
        }
    });

    // ── Receive from WebSocket client → gateway ──
    //
    // Frontend sends JSON:
    //   `{ type: "message", content: "...", session_id?, space_id? }`
    let send_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = FuturesStreamExt::next(&mut ws_rx).await {
            match msg {
                Message::Text(text) => {
                    let parsed: serde_json::Value = match serde_json::from_str(&text) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let content = parsed
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let incoming_session_id = parsed
                        .get("session_id")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(String::from);

                    let incoming_space_id = parsed
                        .get("space_id")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(String::from);

                    if content.is_empty() {
                        continue;
                    }

                    // Save user message for session persistence in recv_task.
                    {
                        let mut pending = pending_for_send.lock().await;
                        *pending = Some(PendingMessage {
                            content: content.clone(),
                            user_id: "web-user".to_string(),
                        });
                    }

                    let mut incoming = IncomingMessage::new("web", "web-user", content);

                    if let Some(ref sid) = incoming_session_id {
                        incoming.metadata.insert("session_id".into(), sid.clone());
                    }
                    if let Some(ref vid) = incoming_space_id {
                        incoming.metadata.insert("space_id".into(), vid.clone());
                    }

                    if incoming_tx.send(incoming).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either side to finish
    tokio::select! {
        _ = recv_task => {}
        _ = send_task => {}
    }
}

/// User message awaiting a gateway response for session persistence.
struct PendingMessage {
    content: String,
    user_id: String,
}

/// Persist a chat exchange (user message + agent response) to the session store.
///
/// Mirrors the logic in the POST `/api/chat` handler so that WebSocket-based
/// conversations are also durable across tab switches and browser restarts.
async fn persist_session(
    state_store: &oxios_kernel::state_store::StateStore,
    session_id: &str,
    user_content: &str,
    user_id: &str,
    agent_content: &str,
    space_id: Option<&str>,
    metadata: &std::collections::HashMap<String, String>,
) {
    let sid = oxios_kernel::state_store::SessionId(session_id.to_string());
    match state_store.load_session(&sid).await {
        Ok(Some(mut session)) => {
            session.add_user_message(user_content);
            session.add_agent_response(oxios_kernel::state_store::AgentResponse {
                content: agent_content.to_string(),
                session_id: Some(sid.0.clone()),
                seed_id: metadata.get("seed_id").cloned(),
                phase_reached: metadata.get("phase").cloned(),
                evaluation_passed: metadata
                    .get("evaluation_passed")
                    .and_then(|v| v.parse().ok()),
                timestamp: chrono::Utc::now(),
            });
            // Attach space_id to session metadata if provided
            if let Some(vid) = space_id {
                session.set_metadata(
                    "space_id",
                    serde_json::json!(vid),
                );
            }
            if let Err(e) = state_store.save_session(&session).await {
                tracing::warn!(error = %e, "WS: failed to persist session");
            }
        }
        Ok(None) => {
            let mut session = oxios_kernel::state_store::Session::new(user_id);
            session.id = oxios_kernel::state_store::SessionId(session_id.to_string());
            session.add_user_message(user_content);
            session.add_agent_response(oxios_kernel::state_store::AgentResponse {
                content: agent_content.to_string(),
                session_id: Some(sid.0.clone()),
                seed_id: metadata.get("seed_id").cloned(),
                phase_reached: metadata.get("phase").cloned(),
                evaluation_passed: metadata
                    .get("evaluation_passed")
                    .and_then(|v| v.parse().ok()),
                timestamp: chrono::Utc::now(),
            });
            // Attach space_id to session metadata
            if let Some(vid) = space_id {
                session.set_metadata(
                    "space_id",
                    serde_json::json!(vid),
                );
            }
            if let Err(e) = state_store.save_session(&session).await {
                tracing::warn!(error = %e, "WS: failed to create session");
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "WS: failed to load/create session");
        }
    }
}
