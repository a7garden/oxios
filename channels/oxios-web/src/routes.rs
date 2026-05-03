//! API routes for the web channel.
//!
//! Provides five route groups:
//! - **Chat**: POST /api/chat, GET /api/chat/stream (WebSocket)
//! - **Control**: GET /api/status, GET /api/agents, POST /api/agents/:id/kill
//! - **Config**: GET /api/config, PUT /api/config
//! - **Browse**: GET /api/workspace/tree, GET/PUT /api/workspace/file/*
//! - **Seeds**: GET /api/seeds, GET /api/seeds/:id
//! - **Memory**: GET /api/memory, GET /api/memory/:name
//! - **Events**: GET /api/events (SSE stream)

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, Query, State, WebSocketUpgrade,
    },
    http::StatusCode,
    response::{
        sse::{Event as SseEvent, Sse},
        IntoResponse,
    },
    routing::{get, post, put},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt as FuturesStreamExt};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as TokioStreamExt;

use crate::server::AppState;

// ---------------------------------------------------------------------------
// Route builder
// ---------------------------------------------------------------------------

/// Builds the axum router with all API routes.
pub fn build_routes() -> Router<Arc<AppState>> {
    Router::new()
        // Chat
        .route("/api/chat", post(handle_chat))
        .route("/api/chat/stream", get(handle_chat_stream))
        // Control
        .route("/api/status", get(handle_status))
        .route("/api/agents", get(handle_agents_list))
        .route("/api/agents/{id}/kill", post(handle_agent_kill))
        // Config
        .route("/api/config", get(handle_config_get))
        .route("/api/config", put(handle_config_put))
        // Workspace
        .route("/api/workspace/tree", get(handle_workspace_tree))
        .route("/api/workspace/file/{*path}", get(handle_workspace_file_get))
        .route("/api/workspace/file/{*path}", put(handle_workspace_file_put))
        // Seeds
        .route("/api/seeds", get(handle_seeds_list))
        .route("/api/seeds/{id}", get(handle_seed_get))
        // Memory
        .route("/api/memory", get(handle_memory_list))
        .route("/api/memory/{name}", get(handle_memory_get))
        // Events
        .route("/api/events", get(handle_events))
}

// ---------------------------------------------------------------------------
// Chat
// ---------------------------------------------------------------------------

/// Request body for the chat endpoint.
#[derive(Debug, Deserialize)]
struct ChatRequest {
    /// The user's message content.
    content: String,
    /// Optional user identifier (defaults to "default").
    #[serde(default = "default_user")]
    user_id: String,
}

fn default_user() -> String {
    "default".into()
}

/// Response body for the chat endpoint.
#[derive(Debug, Serialize)]
struct ChatResponse {
    /// The message ID.
    id: String,
    /// Echo of the user's message.
    echo: String,
    /// Placeholder response.
    reply: String,
}

/// POST /api/chat — Send a message to the kernel via gateway.
async fn handle_chat(
    state: State<Arc<AppState>>,
    Json(body): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, StatusCode> {
    tracing::info!(content = %body.content, user = %body.user_id, "Chat message received");

    // Create an incoming message and push it into the channel
    let msg = oxios_gateway::message::IncomingMessage::new(
        "web",
        &body.user_id,
        &body.content,
    );
    let msg_id = msg.id.to_string();
    let content_echo = body.content.clone();

    if let Err(e) = state.channel.send_incoming(msg).await {
        tracing::error!(error = %e, "Failed to send message to gateway");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // TODO: In the full implementation, we would wait for a response from the
    // kernel via the outgoing channel. For now, return a placeholder.
    Ok(Json(ChatResponse {
        id: msg_id,
        echo: content_echo,
        reply: "Message received. Agent execution pipeline not yet connected.".into(),
    }))
}

/// GET /api/chat/stream — WebSocket endpoint for real-time chat streaming.
async fn handle_chat_stream(
    ws: WebSocketUpgrade,
    state: State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_chat_websocket(socket, state.0))
}

/// Handles a WebSocket connection for chat streaming.
async fn handle_chat_websocket(socket: WebSocket, state: Arc<AppState>) {
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
                        "ws_user",
                        &text.to_string(),
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

// ---------------------------------------------------------------------------
// Control
// ---------------------------------------------------------------------------

/// Response body for the status endpoint.
#[derive(Debug, Serialize)]
struct StatusResponse {
    /// Service name.
    service: String,
    /// Current status.
    status: String,
    /// API version.
    version: String,
    /// Registered channels.
    channels: Vec<String>,
    /// Uptime info.
    uptime: String,
}

/// GET /api/status — System status.
async fn handle_status(
    _state: State<Arc<AppState>>,
) -> Json<StatusResponse> {
    Json(StatusResponse {
        service: "oxios".into(),
        status: "running".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        channels: vec!["web".into()],
        uptime: "n/a".into(),
    })
}

/// Agent summary for listing.
#[derive(Debug, Serialize)]
struct AgentSummary {
    /// Agent unique ID.
    id: String,
    /// Agent name/goal.
    name: String,
    /// Current status.
    status: String,
    /// Creation timestamp.
    created_at: String,
    /// Seed ID if applicable.
    seed_id: Option<String>,
}

/// GET /api/agents — List agent instances.
async fn handle_agents_list(
    _state: State<Arc<AppState>>,
) -> Json<Vec<AgentSummary>> {
    // TODO: Query the supervisor for actual agent data
    // For now, return an empty list
    Json(Vec::new())
}

/// POST /api/agents/:id/kill — Kill an agent.
async fn handle_agent_kill(
    _state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    tracing::info!(agent_id = %id, "Kill agent requested");
    // TODO: Dispatch to supervisor.kill()
    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// GET /api/config — Get current configuration.
async fn handle_config_get(
    _state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // TODO: Load from actual config source
    let placeholder = serde_json::json!({
        "kernel": {
            "workspace": "~/.oxios/workspace",
            "event_bus_capacity": 256,
            "max_agents": 16
        },
        "gateway": {
            "host": "127.0.0.1",
            "port": 4200
        },
        "container": {
            "garden_path": "~/.oxios/gardens"
        }
    });
    Ok(Json(placeholder))
}

/// PUT /api/config — Update configuration.
async fn handle_config_put(
    _state: State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    tracing::info!("Config update requested");
    // TODO: Validate and persist config changes
    Ok(Json(body))
}

// ---------------------------------------------------------------------------
// Workspace
// ---------------------------------------------------------------------------

/// Query parameters for workspace tree.
#[derive(Debug, Deserialize)]
struct TreeQuery {
    /// Subdirectory to list (optional).
    #[serde(default)]
    pub dir: Option<String>,
}

/// File tree entry.
#[derive(Debug, Serialize)]
struct TreeEntry {
    /// File or directory name.
    name: String,
    /// Whether this is a directory.
    is_dir: bool,
    /// File size in bytes (0 for directories).
    size: u64,
}

/// GET /api/workspace/tree — File tree of workspace.
async fn handle_workspace_tree(
    state: State<Arc<AppState>>,
    Query(query): Query<TreeQuery>,
) -> Result<Json<Vec<TreeEntry>>, StatusCode> {
    let base = &state.state_store.base_path;
    let dir = match &query.dir {
        Some(d) => base.join(d),
        None => base.clone(),
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
async fn handle_workspace_file_get(
    state: State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let full_path = state.state_store.base_path.join(&path);

    // Security: ensure the path doesn't escape the workspace
    let canonical_base = state.state_store.base_path.canonicalize().unwrap_or_else(|_| state.state_store.base_path.clone());
    let canonical_file = match full_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return Err(StatusCode::NOT_FOUND),
    };

    if !canonical_file.starts_with(&canonical_base) {
        return Err(StatusCode::FORBIDDEN);
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
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

/// PUT /api/workspace/file/*path — Write/update a file.
async fn handle_workspace_file_put(
    state: State<Arc<AppState>>,
    Path(path): Path<String>,
    body: String,
) -> Result<StatusCode, StatusCode> {
    let full_path = state.state_store.base_path.join(&path);

    // Security: ensure the path doesn't escape the workspace
    let canonical_base = state.state_store.base_path.canonicalize().unwrap_or_else(|_| state.state_store.base_path.clone());
    // For new files, canonicalize the parent dir
    if let Some(parent) = full_path.parent() {
        if let Err(_) = parent.canonicalize() {
            // Try to create the parent directory
            if let Err(_) = tokio::fs::create_dir_all(parent).await {
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
        if let Ok(canonical_parent) = parent.canonicalize() {
            if !canonical_parent.starts_with(&canonical_base) {
                return Err(StatusCode::FORBIDDEN);
            }
        }
    }

    match tokio::fs::write(&full_path, &body).await {
        Ok(_) => {
            tracing::info!(path = %path, "File written");
            Ok(StatusCode::OK)
        }
        Err(e) => {
            tracing::error!(path = %path, error = %e, "Failed to write file");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
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
        Some("html") => "text/html; charset=utf-8".into(),
        Some("css") => "text/css; charset=utf-8".into(),
        Some("js") => "application/javascript".into(),
        _ => "text/plain; charset=utf-8".into(),
    }
}

// ---------------------------------------------------------------------------
// Seeds
// ---------------------------------------------------------------------------

/// Seed summary for listing.
#[derive(Debug, Serialize)]
struct SeedSummary {
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
async fn handle_seeds_list(
    state: State<Arc<AppState>>,
) -> Json<Vec<SeedSummary>> {
    let mut summaries = Vec::new();

    if let Ok(names) = state.state_store.list_category("seeds").await {
        for name in names {
            if let Ok(Some(content)) = state.state_store.load_markdown("seeds", &name).await {
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

    Json(summaries)
}

/// GET /api/seeds/:id — Get a specific seed.
async fn handle_seed_get(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Try JSON first, then markdown
    if let Ok(Some(content)) = state.state_store.load_markdown("seeds", &id).await {
        if let Ok(seed) = serde_json::from_str::<oxios_ouroboros::Seed>(&content) {
            return Ok(Json(serde_json::to_value(&seed).unwrap_or_default()));
        }
        return Ok(Json(serde_json::json!({
            "id": id,
            "content": content,
        })));
    }

    Err(StatusCode::NOT_FOUND)
}

// ---------------------------------------------------------------------------
// Memory
// ---------------------------------------------------------------------------

/// Memory entry summary.
#[derive(Debug, Serialize)]
struct MemorySummary {
    /// Entry name.
    name: String,
    /// Category (memory type).
    category: String,
}

/// GET /api/memory — List memory entries.
async fn handle_memory_list(
    state: State<Arc<AppState>>,
) -> Json<Vec<MemorySummary>> {
    let mut entries = Vec::new();

    // List daily memory files
    if let Ok(names) = state.state_store.list_category("memory").await {
        for name in names {
            entries.push(MemorySummary {
                name,
                category: "daily".into(),
            });
        }
    }

    // List knowledge base entries
    if let Ok(names) = state.state_store.list_category("memory/knowledge").await {
        for name in names {
            entries.push(MemorySummary {
                name,
                category: "knowledge".into(),
            });
        }
    }

    Json(entries)
}

/// GET /api/memory/:name — Get a specific memory entry.
async fn handle_memory_get(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    // Try memory/ first, then memory/knowledge/
    if let Ok(Some(content)) = state.state_store.load_markdown("memory", &name).await {
        return Ok(Json(serde_json::json!({
            "name": name,
            "category": "daily",
            "content": content,
        }))
        .into_response());
    }

    if let Ok(Some(content)) = state
        .state_store
        .load_markdown("memory/knowledge", &name)
        .await
    {
        return Ok(Json(serde_json::json!({
            "name": name,
            "category": "knowledge",
            "content": content,
        }))
        .into_response());
    }

    Err(StatusCode::NOT_FOUND)
}

// ---------------------------------------------------------------------------
// Events (SSE)
// ---------------------------------------------------------------------------

/// GET /api/events — SSE stream of KernelEvent.
async fn handle_events(
    state: State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<SseEvent, Infallible>>> {
    let receiver = state.event_bus.subscribe();
    let stream = BroadcastStream::new(receiver);
    let stream = TokioStreamExt::filter_map(stream, |result| {
        match result {
            Ok(event) => {
                let data = serde_json::to_string(&event).unwrap_or_default();
                Some(Ok(SseEvent::default().data(data)))
            }
            Err(_) => None, // Skip lagged messages
        }
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(30))
            .text("ping"),
    )
}
