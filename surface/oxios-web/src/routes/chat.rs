use std::sync::Arc;

use axum::extract::{
    ws::{Message, WebSocket},
    State, WebSocketUpgrade,
};
use axum::extract::{Path, Query};
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
    /// Optional space ID for context partitioning.
    #[serde(default)]
    project_id: String,
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
    /// Space ID for context partitioning.
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
    /// Phase reached during orchestration.
    #[serde(skip_serializing_if = "Option::is_none")]
    phase: Option<String>,
    /// RFC-014: Space tag decoration.
    #[serde(skip_serializing_if = "Option::is_none")]
    project_tag: Option<String>,
    /// RFC-014: Seed ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    seed_id: Option<String>,
    /// RFC-014: Evaluation passed.
    #[serde(skip_serializing_if = "Option::is_none")]
    evaluation_passed: Option<bool>,
    /// RFC-014: Duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u64>,
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

    // Include project_id from request if provided (for context partitioning).
    if !body.project_id.is_empty() {
        msg.metadata
            .insert("project_ids".to_owned(), body.project_id.clone());
    }

    let msg_id = msg.id.to_string();
    let content_echo = body.content.clone();

    // Send and wait for response from the gateway pipeline.
    tracing::info!("Sending message to gateway...");
    match state.bridge.send_and_wait(msg).await {
        Ok(response) => {
            tracing::info!(reply_len = response.content.len(), "Chat response received");

            // Extract from typed meta (RFC-014) or fall back to metadata HashMap
            let meta = response.meta.as_ref();
            let session_id = meta
                .and_then(|m| m.session_id.clone())
                .or_else(|| response.metadata.get("session_id").cloned());
            let project_id = meta
                .and_then(|m| m.project_id.clone())
                .or_else(|| response.metadata.get("project_id").cloned());
            let phase = meta
                .map(|m| m.phase.clone())
                .or_else(|| response.metadata.get("phase").cloned());
            let project_tag = meta.and_then(|m| m.project_tag.clone());
            let seed_id = meta.and_then(|m| m.seed_id.clone());
            let evaluation_passed = meta.map(|m| m.evaluation_passed);
            let duration_ms = meta.and_then(|m| m.duration_ms);

            // Persist session
            {
                // RFC-015: parse tool_calls into trajectory step records.
                let trajectory_steps: Vec<oxios_kernel::state_store::TrajectoryStepRecord> =
                    response
                        .metadata
                        .get("tool_calls")
                        .and_then(|v| serde_json::from_str::<Vec<serde_json::Value>>(v).ok())
                        .map(|calls| {
                            calls
                                .into_iter()
                                .enumerate()
                                .map(|(i, c)| oxios_kernel::state_store::TrajectoryStepRecord {
                                    tool_name: c
                                        .get("tool")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                    tool_args: c
                                        .get("input")
                                        .cloned()
                                        .unwrap_or(serde_json::Value::Null),
                                    output_summary: c
                                        .get("output")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                    duration_ms: c
                                        .get("duration_ms")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0),
                                    is_error: false,
                                    tool_call_id: format!("legacy-{i}"),
                                    timestamp: chrono::Utc::now(),
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                let session_id_for_save = session_id.clone().unwrap_or_else(|| msg_id.clone());
                let sid = oxios_kernel::state_store::SessionId(session_id_for_save.clone());
                match state.kernel.state.load_session(&sid).await {
                    Ok(Some(mut session)) => {
                        session.add_user_message(&content_echo);
                        session.add_agent_response(oxios_kernel::state_store::AgentResponse {
                            content: response.content.clone(),
                            session_id: Some(sid.0.clone()),
                            seed_id: seed_id.clone(),
                            phase_reached: phase.clone(),
                            evaluation_passed,
                            timestamp: chrono::Utc::now(),
                        });
                        // RFC-015: persist trajectory so the Web UI can re-render
                        // the execution timeline when the user re-opens the session.
                        session.extend_trajectory(trajectory_steps);
                        // Attach project_id to session metadata if provided by orchestrator
                        if let Some(ref vid) = project_id {
                            session.set_metadata("project_id", serde_json::json!(vid));
                        }
                        if let Err(e) = state.kernel.state.save_session(&session).await {
                            tracing::warn!(error = %e, "Failed to persist session");
                        }
                    }
                    Ok(None) => {
                        // Create new session
                        let mut session =
                            oxios_kernel::state_store::Session::new(body.user_id.clone());
                        session.id = oxios_kernel::state_store::SessionId(session_id_for_save);
                        session.add_user_message(&content_echo);
                        session.add_agent_response(oxios_kernel::state_store::AgentResponse {
                            content: response.content.clone(),
                            session_id: Some(sid.0.clone()),
                            seed_id: seed_id.clone(),
                            phase_reached: phase.clone(),
                            evaluation_passed,
                            timestamp: chrono::Utc::now(),
                        });
                        // RFC-015: persist trajectory on first save.
                        session.extend_trajectory(trajectory_steps);
                        // Attach project_id to session metadata if provided by orchestrator
                        if let Some(ref vid) = project_id {
                            session.set_metadata("project_id", serde_json::json!(vid));
                        }
                        if let Err(e) = state.kernel.state.save_session(&session).await {
                            tracing::warn!(error = %e, "Failed to create session");
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "Failed to load/create session"),
                }

                // Auto-prune sessions if configured (throttled to once per hour)
                let cfg = state.config.read();
                if cfg.session.auto_prune && state.kernel.state.should_auto_prune() {
                    let prune_config = oxios_kernel::state_store::PruneConfig {
                        max_sessions: cfg.session.max_sessions,
                        ttl_hours: cfg.session.ttl_hours,
                    };
                    drop(cfg); // release read lock before async
                    let kernel = state.kernel.clone();
                    tokio::spawn(async move {
                        if let Err(e) = kernel.state.prune_sessions(&prune_config).await {
                            tracing::warn!(error = %e, "Session auto-prune failed");
                        }
                    });
                }
            }

            Ok(Json(ChatResponse {
                id: msg_id,
                echo: content_echo,
                reply: response.content,
                session_id: session_id.clone(),
                project_id: project_id.clone(),
                phase,
                project_tag,
                seed_id,
                evaluation_passed,
                duration_ms,
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
    /// One-time ticket for authentication (preferred).
    ticket: Option<String>,
    /// Bearer token for authentication (fallback).
    token: Option<String>,
}

/// POST /api/chat/ticket — Generate a one-time WebSocket ticket.
pub(crate) async fn handle_chat_ticket(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Auth middleware already validated Bearer token if auth is enabled.
    let ticket = state.kernel.security.generate_ws_ticket();
    Ok(Json(serde_json::json!({ "ticket": ticket })))
}

/// GET /api/chat/stream — WebSocket endpoint for real-time chat streaming.
pub(crate) async fn handle_chat_stream(
    ws: WebSocketUpgrade,
    state: State<Arc<AppState>>,
    Query(params): Query<WsParams>,
) -> impl axum::response::IntoResponse {
    // Authenticate if auth is enabled
    if state.config.read().security.auth_enabled {
        let authenticated = if let Some(ref ticket) = params.ticket {
            state.kernel.security.validate_ws_ticket(ticket)
        } else if let Some(ref token) = params.token {
            state.kernel.security.validate_token(token)
        } else {
            false
        };
        if !authenticated {
            return axum::http::StatusCode::UNAUTHORIZED.into_response();
        }
    }
    ws.on_upgrade(move |socket| handle_chat_websocket(socket, state.0))
}

/// Handles a WebSocket connection for chat streaming.
///
/// Protocol:
/// - **Incoming** (frontend → backend):
///   `{ type: "message", content: "...", session_id?: "...", project_id?: "..." }`
/// - **Outgoing token** (backend → frontend):
///   `{ type: "token", content: "...", session_id?, project_id? }`
/// - **Outgoing done** (backend → frontend):
///   `{ type: "done", session_id?, project_id?, phase?, evaluation_passed? }`
pub(crate) async fn handle_chat_websocket(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Subscribe to outgoing messages from the web channel (not kernel event bus).
    // WebChannel::send() broadcasts OutgoingMessage here; the kernel event bus
    // carries KernelEvents which are a different type entirely.
    let mut outgoing_rx = state.bridge.subscribe();
    // RFC-015: subscribe to kernel event bus for real-time chat transparency
    // events (tool execution, token usage, memory recall, reasoning fragments).
    // Filtered by session_id in the recv loop to avoid leaking other agents' events.
    let mut kernel_event_rx = state.kernel.infra.subscribe();

    // Clone handles for the spawned tasks.
    let incoming_tx = state.bridge.incoming_tx.clone();
    let state_store = state.kernel.state.store().clone();

    // Read session prune config
    let prune_config = {
        let cfg = state.config.read();
        if cfg.session.auto_prune {
            Some(oxios_kernel::state_store::PruneConfig {
                max_sessions: cfg.session.max_sessions,
                ttl_hours: cfg.session.ttl_hours,
            })
        } else {
            None
        }
    };

    // Track the last user message for session persistence.
    // The send_task sets this before forwarding to gateway;
    // the recv_task reads it when the response arrives.
    // Keyed by message ID so only the matching response triggers persistence.
    let pending_user_msg: Arc<tokio::sync::Mutex<Option<(uuid::Uuid, PendingMessage)>>> =
        Arc::new(tokio::sync::Mutex::new(None));

    let pending_for_send = pending_user_msg.clone();

    // ── Forward gateway responses → WebSocket client ──
    //
    // Each chunk carries session_id + project_id so the frontend can
    // maintain multi-turn context. After the "done" chunk we persist
    // the session to disk (same as the POST handler).
    //
    // RFC-015: also forward real-time kernel events (tool execution, token
    // usage, memory recall, reasoning fragments) as WS chunks so the
    // frontend can show live progress.
    let recv_task = tokio::spawn(async move {
        // Track the active session so we only forward events tagged with it.
        // Multi-turn conversations keep the same session_id across messages.
        let mut active_session_id: Option<String> = None;

        loop {
            tokio::select! {
                // Bias toward gateway messages (text streaming + done).
                biased;
                msg_result = outgoing_rx.recv() => {
                    let Ok(msg) = msg_result else { break };
                    let msg_id = msg.id;
                    let session_id = msg
                        .meta
                        .as_ref()
                        .and_then(|m| m.session_id.clone())
                        .or_else(|| msg.metadata.get("session_id").cloned());
                    let project_id = msg
                        .meta
                        .as_ref()
                        .and_then(|m| m.project_id.clone())
                        .or_else(|| msg.metadata.get("project_id").cloned());
                    let phase = msg
                        .meta
                        .as_ref()
                        .map(|m| m.phase.clone())
                        .or_else(|| msg.metadata.get("phase").cloned());
                    let evaluation_passed = msg.meta.as_ref().map(|m| m.evaluation_passed);
                    let project_tag = msg.meta.as_ref().and_then(|m| m.project_tag.clone());
                    let seed_id = msg.meta.as_ref().and_then(|m| m.seed_id.clone());
                    let duration_ms = msg.meta.as_ref().and_then(|m| m.duration_ms);

                    // Remember the session we are forwarding for. Subsequent
                    // kernel events without a session_id are still forwarded
                    // (some events are system-wide).
                    if session_id.is_some() {
                        active_session_id = session_id.clone();
                    }

                    // ── Persist session to disk FIRST ──
                    // Always persist, even if WS send fails later. This ensures
                    // the exchange is durable even if the connection drops mid-stream.
                    if let Some(ref sid) = session_id {
                        let pending = pending_user_msg.lock().await;
                        if let Some((ref pending_id, _)) = *pending {
                            if *pending_id == msg_id {
                                drop(pending); // release lock before async I/O
                                let pm = {
                                    let mut guard = pending_user_msg.lock().await;
                                    guard.take()
                                };
                                if let Some((_, pm)) = pm {
                                    persist_session(
                                        &state_store,
                                        sid,
                                        pm.content.as_str(),
                                        pm.user_id.as_str(),
                                        &msg.content,
                                        project_id.as_deref(),
                                        &msg.metadata,
                                        prune_config.clone(),
                                    )
                                    .await;
                                }
                            }
                        }
                    }

                    // ── Forward to WebSocket client ──
                    // Send token chunk with metadata
                    let token_chunk = serde_json::json!({
                        "type": "token",
                        "content": msg.content,
                        "session_id": session_id,
                        "project_id": project_id,
                    });
                    let json = match serde_json::to_string(&token_chunk) {
                        Ok(j) => j,
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to serialize outgoing message");
                            continue;
                        }
                    };
                    if ws_tx.send(Message::Text(json.into())).await.is_err() {
                        break; // WS closed — session was already persisted above
                    }

                    // Send done chunk with final metadata
                    let done_chunk = serde_json::json!({
                        "type": "done",
                        "session_id": session_id,
                        "project_id": project_id,
                        "phase": phase,
                        "evaluation_passed": evaluation_passed,
                        "project_tag": project_tag,
                        "seed_id": seed_id,
                        "duration_ms": duration_ms,
                        // TODO: populate tool_calls from trajectory_steps once kernel provides them
                        "tool_calls": msg.metadata.get("tool_calls")
                            .and_then(|v| serde_json::from_str::<serde_json::Value>(v).ok())
                            .unwrap_or(serde_json::json!([])),
                    });
                    let done_json = match serde_json::to_string(&done_chunk) {
                        Ok(j) => j,
                        Err(_) => break,
                    };
                    if ws_tx.send(Message::Text(done_json.into())).await.is_err() {
                        break; // WS closed — session was already persisted above
                    }
                }
                event_result = kernel_event_rx.recv() => {
                    // Convert KernelEvent → WS chunk when relevant.
                    let Ok(event) = event_result else {
                        // Lagged or closed — skip and keep waiting.
                        continue;
                    };
                    if let Some(chunk) = kernel_event_to_ws_chunk(&event, &active_session_id) {
                        let json = match serde_json::to_string(&chunk) {
                            Ok(j) => j,
                            Err(e) => {
                                tracing::warn!(error = %e, "Failed to serialize transparency chunk");
                                continue;
                            }
                        };
                        if ws_tx.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    // ── Receive from WebSocket client → gateway ──
    //
    // Frontend sends JSON:
    //   `{ type: "message", content: "...", session_id?, project_id? }`
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

                    let incoming_project_id = parsed
                        .get("project_id")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(String::from);

                    if content.is_empty() {
                        continue;
                    }

                    let mut incoming = IncomingMessage::new("web", "default", content.clone());

                    if let Some(ref sid) = incoming_session_id {
                        incoming.metadata.insert("session_id".into(), sid.clone());
                    }
                    if let Some(ref vid) = incoming_project_id {
                        incoming.metadata.insert("project_id".into(), vid.clone());
                    }

                    // Save user message + its ID for correlated session persistence.
                    {
                        let mut pending = pending_for_send.lock().await;
                        *pending = Some((
                            incoming.id,
                            PendingMessage {
                                content,
                                user_id: "default".to_string(),
                            },
                        ));
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
#[allow(clippy::too_many_arguments)]
async fn persist_session(
    state_store: &oxios_kernel::state_store::StateStore,
    session_id: &str,
    user_content: &str,
    user_id: &str,
    agent_content: &str,
    project_id: Option<&str>,
    metadata: &std::collections::HashMap<String, String>,
    prune_config: Option<oxios_kernel::state_store::PruneConfig>,
) {
    let sid = oxios_kernel::state_store::SessionId(session_id.to_string());
    // RFC-015: parse tool_calls JSON into trajectory step records.
    let trajectory_steps: Vec<oxios_kernel::state_store::TrajectoryStepRecord> = metadata
        .get("tool_calls")
        .and_then(|v| serde_json::from_str::<Vec<serde_json::Value>>(v).ok())
        .map(|calls| {
            calls
                .into_iter()
                .enumerate()
                .map(|(i, c)| oxios_kernel::state_store::TrajectoryStepRecord {
                    tool_name: c
                        .get("tool")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    tool_args: c.get("input").cloned().unwrap_or(serde_json::Value::Null),
                    output_summary: c
                        .get("output")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    duration_ms: c.get("duration_ms").and_then(|v| v.as_u64()).unwrap_or(0),
                    is_error: false,
                    tool_call_id: format!("legacy-{i}"),
                    timestamp: chrono::Utc::now(),
                })
                .collect()
        })
        .unwrap_or_default();

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
            // RFC-015: persist trajectory so the Web UI can re-render the
            // execution timeline when the user re-opens the session.
            session.extend_trajectory(trajectory_steps);
            // Attach project_id to session metadata if provided
            if let Some(vid) = project_id {
                session.set_metadata("project_id", serde_json::json!(vid));
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
            // RFC-015: persist trajectory on first save.
            session.extend_trajectory(trajectory_steps);
            // Attach project_id to session metadata
            if let Some(vid) = project_id {
                session.set_metadata("project_id", serde_json::json!(vid));
            }
            if let Err(e) = state_store.save_session(&session).await {
                tracing::warn!(error = %e, "WS: failed to create session");
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "WS: failed to load/create session");
        }
    }

    // Auto-prune in background after session save (throttled)
    if let Some(config) = prune_config {
        // Only prune if at least 1 hour has passed since the last prune.
        // Uses a process-global throttle to avoid spawning on every message.
        static LAST_PRUNE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let last = LAST_PRUNE.load(std::sync::atomic::Ordering::Relaxed);
        if now_secs.saturating_sub(last) >= 3600 {
            LAST_PRUNE.store(now_secs, std::sync::atomic::Ordering::Relaxed);
            let store = state_store.clone();
            tokio::spawn(async move {
                if let Err(e) = store.prune_sessions(&config).await {
                    tracing::warn!(error = %e, "WS: session auto-prune failed");
                }
            });
        }
    }
}

/// Convert a `KernelEvent` into a WebSocket chunk for chat transparency
/// (RFC-015). Returns `None` when the event is not relevant to chat
/// progress (e.g. agent lifecycle events unrelated to the current
/// session) or when it does not belong to the active session.
fn kernel_event_to_ws_chunk(
    event: &oxios_kernel::event_bus::KernelEvent,
    active_session_id: &Option<String>,
) -> Option<serde_json::Value> {
    use oxios_kernel::event_bus::KernelEvent;

    // Helper: skip events that do not belong to the active session.
    // Events without a session_id (e.g. project lifecycle) are always passed
    // through — they are not high-frequency and can be useful context.
    let event_session_id: Option<&str> = match event {
        KernelEvent::ToolExecutionStarted { session_id, .. } => Some(session_id),
        KernelEvent::ToolExecutionFinished { session_id, .. } => Some(session_id),
        KernelEvent::ToolExecutionProgress { session_id, .. } => Some(session_id),
        KernelEvent::MemoryRecallUsed { session_id, .. } => Some(session_id),
        KernelEvent::TokenUsageUpdate { session_id, .. } => Some(session_id),
        KernelEvent::ReasoningFragment { session_id, .. } => Some(session_id),
        _ => None,
    };
    if let (Some(eid), Some(active)) = (event_session_id, active_session_id.as_ref()) {
        if eid != active.as_str() {
            return None;
        }
    }

    match event {
        KernelEvent::ToolExecutionStarted {
            tool_name,
            tool_call_id,
            tool_args,
            ..
        } => Some(serde_json::json!({
            "type": "tool_start",
            "tool_name": tool_name,
            "tool_call_id": tool_call_id,
            "tool_args": tool_args,
        })),
        KernelEvent::ToolExecutionFinished {
            tool_name,
            tool_call_id,
            duration_ms,
            is_error,
            output_summary,
            ..
        } => Some(serde_json::json!({
            "type": "tool_end",
            "tool_name": tool_name,
            "tool_call_id": tool_call_id,
            "duration_ms": duration_ms,
            "is_error": is_error,
            "output_summary": output_summary,
        })),
        KernelEvent::ToolExecutionProgress {
            tool_call_id,
            tool_name,
            progress,
            tab_id,
            ..
        } => {
            let mut obj = serde_json::json!({
                "type": "tool_progress",
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "progress": progress,
            });
            if let Some(id) = tab_id {
                obj["tab_id"] = serde_json::json!(id.to_string());
            }
            Some(obj)
        }
        KernelEvent::MemoryRecallUsed {
            query,
            count,
            source,
            ..
        } => Some(serde_json::json!({
            "type": "memory",
            "action": "recall",
            "query": query,
            "count": count,
            "source": source,
        })),
        KernelEvent::TokenUsageUpdate {
            input_tokens,
            output_tokens,
            ..
        } => Some(serde_json::json!({
            "type": "usage",
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
        })),
        KernelEvent::ReasoningFragment {
            content, source, ..
        } => Some(serde_json::json!({
            "type": "reasoning",
            "content": content,
            "source": source,
        })),
        // PhaseStarted / PhaseCompleted are not included in the WS stream
        // here because the orchestrator already publishes them with extra
        // metadata (result_summary) and we don't want to double-emit. The
        // global /api/events SSE channel carries them for the events page.
        _ => None,
    }
}

/// GET /api/sessions/{id}/tool-calls — Get tool call timeline for a session.
pub(crate) async fn handle_session_tool_calls(
    _state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Session tool calls are not yet stored persistently.
    // Return empty array for now — will be populated when trajectory_steps
    // are persisted to sessions.
    Ok(Json(serde_json::json!({
        "session_id": id,
        "tool_calls": []
    })))
}

#[cfg(test)]
mod rfc015_tests {
    use super::*;
    use oxios_kernel::event_bus::KernelEvent;
    use oxios_kernel::AgentId;

    /// Every RFC-015 KernelEvent should map to the documented WS chunk type
    /// (tool_start, tool_end, memory, usage, reasoning). This is the wire
    /// contract the frontend `chunkToActivity` depends on, so a regression
    /// here breaks the entire chat transparency UI.
    #[test]
    fn tool_started_emits_tool_start() {
        let event = KernelEvent::ToolExecutionStarted {
            session_id: "s1".into(),
            tool_name: "read_file".into(),
            tool_call_id: "c1".into(),
            tool_args: serde_json::json!({"path": "/x"}),
        };
        let chunk = kernel_event_to_ws_chunk(&event, &Some("s1".into())).unwrap();
        assert_eq!(chunk["type"], "tool_start");
        assert_eq!(chunk["tool_name"], "read_file");
        assert_eq!(chunk["tool_call_id"], "c1");
        assert_eq!(chunk["tool_args"]["path"], "/x");
    }

    #[test]
    fn tool_finished_emits_tool_end() {
        let event = KernelEvent::ToolExecutionFinished {
            session_id: "s1".into(),
            tool_call_id: "c1".into(),
            tool_name: "read_file".into(),
            duration_ms: 123,
            is_error: false,
            output_summary: "ok".into(),
        };
        let chunk = kernel_event_to_ws_chunk(&event, &Some("s1".into())).unwrap();
        assert_eq!(chunk["type"], "tool_end");
        assert_eq!(chunk["duration_ms"], 123);
        assert_eq!(chunk["is_error"], false);
    }

    /// Real-time tool progress (RFC-015 v0.12) must be forwarded as a
    /// `tool_progress` chunk so the Web UI can show a spinner and the
    /// latest progress text while the tool is still running. When the
    /// upstream event carries a `tab_id`, it must be included in the
    /// chunk so the frontend can badge concurrent tab activity.
    #[test]
    fn tool_progress_emits_tool_progress_chunk() {
        let tab_id = uuid::Uuid::new_v4();
        let event = KernelEvent::ToolExecutionProgress {
            session_id: "s1".into(),
            tool_call_id: "c1".into(),
            tool_name: "browse".into(),
            progress: "loading https://example.com".into(),
            tab_id: Some(tab_id),
        };
        let chunk = kernel_event_to_ws_chunk(&event, &Some("s1".into())).unwrap();
        assert_eq!(chunk["type"], "tool_progress");
        assert_eq!(chunk["tool_call_id"], "c1");
        assert_eq!(chunk["tool_name"], "browse");
        assert_eq!(chunk["progress"], "loading https://example.com");
        assert_eq!(chunk["tab_id"], tab_id.to_string());
    }

    /// Progress events must be filtered by session_id, same as start/end.
    #[test]
    fn tool_progress_foreign_session_is_filtered() {
        let event = KernelEvent::ToolExecutionProgress {
            session_id: "other".into(),
            tool_call_id: "c1".into(),
            tool_name: "browse".into(),
            progress: "leak me".into(),
            tab_id: None,
        };
        let chunk = kernel_event_to_ws_chunk(&event, &Some("s1".into()));
        assert!(chunk.is_none(), "foreign progress should be filtered");
    }

    /// When `tab_id` is `None` (legacy oxi-agent versions), the chunk must
    /// omit the `tab_id` key entirely so the frontend treats it as
    /// "no badge" rather than rendering `null`.
    #[test]
    fn tool_progress_chunk_omits_tab_id_when_none() {
        let event = KernelEvent::ToolExecutionProgress {
            session_id: "s1".into(),
            tool_call_id: "c1".into(),
            tool_name: "browse".into(),
            progress: "step 1".into(),
            tab_id: None,
        };
        let chunk = kernel_event_to_ws_chunk(&event, &Some("s1".into())).unwrap();
        assert!(
            chunk.get("tab_id").is_none(),
            "tab_id key should be absent when None; got: {chunk}"
        );
    }

    #[test]
    fn memory_recall_emits_memory_chunk() {
        let event = KernelEvent::MemoryRecallUsed {
            session_id: "s1".into(),
            query: "rust errors".into(),
            count: 3,
            source: "warm".into(),
        };
        let chunk = kernel_event_to_ws_chunk(&event, &Some("s1".into())).unwrap();
        assert_eq!(chunk["type"], "memory");
        assert_eq!(chunk["action"], "recall");
        assert_eq!(chunk["count"], 3);
    }

    #[test]
    fn token_usage_emits_usage_chunk() {
        let event = KernelEvent::TokenUsageUpdate {
            session_id: "s1".into(),
            input_tokens: 100,
            output_tokens: 50,
        };
        let chunk = kernel_event_to_ws_chunk(&event, &Some("s1".into())).unwrap();
        assert_eq!(chunk["type"], "usage");
        assert_eq!(chunk["input_tokens"], 100);
        assert_eq!(chunk["output_tokens"], 50);
    }

    #[test]
    fn reasoning_emits_reasoning_chunk() {
        let event = KernelEvent::ReasoningFragment {
            session_id: "s1".into(),
            content: "compaction done".into(),
            source: "compaction".into(),
        };
        let chunk = kernel_event_to_ws_chunk(&event, &Some("s1".into())).unwrap();
        assert_eq!(chunk["type"], "reasoning");
        assert_eq!(chunk["content"], "compaction done");
        assert_eq!(chunk["source"], "compaction");
    }

    /// Events tagged with a different session must be dropped — otherwise
    /// unrelated agents' tool calls would leak into the wrong chat.
    #[test]
    fn foreign_session_is_filtered() {
        let event = KernelEvent::ToolExecutionStarted {
            session_id: "other".into(),
            tool_name: "bash".into(),
            tool_call_id: "x".into(),
            tool_args: serde_json::Value::Null,
        };
        let chunk = kernel_event_to_ws_chunk(&event, &Some("s1".into()));
        assert!(chunk.is_none(), "foreign session should not be forwarded");
    }

    /// When no active session is set (e.g. mid-connect), the filter is
    /// effectively a no-op. Session-scoped events still pass through
    /// because we cannot distinguish "this agent is for me" from "this
    /// agent is for someone else" without a session tag. The first
    /// gateway message will populate `active_session_id`, and from that
    /// point foreign sessions are filtered correctly.
    #[test]
    fn no_active_session_passes_session_scoped_events() {
        let event = KernelEvent::TokenUsageUpdate {
            session_id: "s1".into(),
            input_tokens: 1,
            output_tokens: 1,
        };
        let chunk = kernel_event_to_ws_chunk(&event, &None);
        assert!(
            chunk.is_some(),
            "filter is inactive without an active session"
        );
        assert_eq!(chunk.unwrap()["type"], "usage");
    }

    /// Lifecycle events (AgentStarted, PhaseCompleted, …) should not be
    /// forwarded as RFC-015 chunks — the global /api/events SSE handles
    /// them. Returning None keeps the WS stream clean.
    #[test]
    fn lifecycle_events_are_skipped() {
        let event = KernelEvent::AgentStarted {
            id: AgentId::new_v4(),
        };
        let chunk = kernel_event_to_ws_chunk(&event, &None);
        assert!(chunk.is_none());
    }
}
