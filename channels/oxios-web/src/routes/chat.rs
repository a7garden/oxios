use std::sync::Arc;

use axum::extract::{ws::{Message, WebSocket}, State, WebSocketUpgrade};
use axum::extract::Query;
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
    tracing::info!(content = %body.content, user = %body.user_id, "Chat message received");

    // Build the incoming message.
    let mut msg = IncomingMessage::new(
        "web",
        &body.user_id,
        &body.content,
    );

    // Include session_id from request if provided (for multi-turn conversations).
    if !body.session_id.is_empty() {
        msg.metadata.insert("session_id".to_owned(), body.session_id.clone());
    }

    let msg_id = msg.id.to_string();
    let content_echo = body.content.clone();

    // Send and wait for response from the gateway pipeline.
    match state.channel.send_and_wait(msg).await {
        Ok(response) => {
            tracing::info!(reply_len = response.content.len(), "Chat response received");
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
    if state.config.security.auth_enabled {
        let token = params.token.as_deref().unwrap_or("");
        let valid = { state.auth_manager.lock().validate(token) };
        if !valid {
            return axum::http::StatusCode::UNAUTHORIZED.into_response();
        }
    }
    ws.on_upgrade(move |socket| handle_chat_websocket(socket, state.0))
}

/// Handles a WebSocket connection for chat streaming.
pub(crate) async fn handle_chat_websocket(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Subscribe to outgoing messages
    let mut outgoing_rx = state.channel.subscribe();

    // Forward outgoing messages to the WebSocket
    let recv_task = tokio::spawn(async move {
        while let Ok(msg) = outgoing_rx.recv().await {
            let json = match serde_json::to_string(&msg) {
                Ok(j) => j,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to serialize outgoing message");
                    continue;
                }
            };
            if ws_tx.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    // Receive messages from the WebSocket and push to gateway
    let send_tx = state.channel.incoming_tx.clone();
    let send_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = FuturesStreamExt::next(&mut ws_rx).await {
            match msg {
                Message::Text(text) => {
                    let incoming = oxios_gateway::message::IncomingMessage::new(
                        "web",
                        "session",  // TODO: derive from token when user system exists
                        text.to_string(),
                    );
                    if send_tx.send(incoming).await.is_err() {
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
        _ = recv_task => {},
        _ = send_task => {},
    }
}
