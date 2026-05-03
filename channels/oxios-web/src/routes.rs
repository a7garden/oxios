//! API routes for the web channel.
//!
//! Provides route groups:
//! - **Chat**: POST /api/chat, GET /api/chat/stream (WebSocket)
//! - **Control**: GET /api/status, GET /api/agents, POST /api/agents/:id/kill
//! - **Config**: GET /api/config, PUT /api/config
//! - **Browse**: GET /api/workspace/tree, GET/PUT /api/workspace/file/*
//! - **Seeds**: GET /api/seeds, GET /api/seeds/:id, GET /api/seeds/:id/evolution
//! - **Skills**: GET /api/skills, GET /api/skills/:name, POST /api/skills, DELETE /api/skills/:name
//! - **Memory**: GET /api/memory, GET /api/memory/:name
//! - **Gardens**: GET /api/gardens, POST /api/gardens, POST /api/gardens/:name/start,
//!   POST /api/gardens/:name/stop, DELETE /api/gardens/:name, POST /api/gardens/:name/exec
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
    routing::{delete, get, post, put},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt as FuturesStreamExt};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as TokioStreamExt;
use oxios_kernel::state_store::StateStore;
use oxios_gateway::message::IncomingMessage;
use oxios_kernel::{AgentId, access_manager::AuditEntry};
use uuid::Uuid;

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
        .route("/api/seeds/{id}/evolution", get(handle_seed_evolution))
        // Skills
        .route("/api/skills", get(handle_skills_list))
        .route("/api/skills/{name}", get(handle_skill_get))
        .route("/api/skills", post(handle_skill_create))
        .route("/api/skills/{name}", delete(handle_skill_delete))
        // Memory
        .route("/api/memory", get(handle_memory_list))
        .route("/api/memory/{name}", get(handle_memory_get))
        // Gardens
        .route("/api/gardens", get(handle_gardens_list))
        .route("/api/gardens", post(handle_garden_create))
        .route("/api/gardens/{name}/start", post(handle_garden_start))
        .route("/api/gardens/{name}/stop", post(handle_garden_stop))
        .route("/api/gardens/{name}", delete(handle_garden_remove))
        .route("/api/gardens/{name}/exec", post(handle_garden_exec))
        // Scheduler stats & tasks
        .route("/api/scheduler/stats", get(handle_scheduler_stats))
        .route("/api/scheduler/tasks", get(handle_scheduler_tasks))
        // Audit log & permissions
        .route("/api/audit", get(handle_audit_log))
        .route("/api/permissions/{agent}", get(handle_permissions_get))
        .route("/api/permissions/{agent}", put(handle_permissions_put))
        // Programs
        .route("/api/programs", get(handle_programs_list))
        .route("/api/programs", post(handle_program_install))
        .route("/api/programs/{name}", get(handle_program_get))
        .route("/api/programs/{name}", delete(handle_program_uninstall))
        .route("/api/programs/{name}/enable", post(handle_program_enable))
        .route("/api/programs/{name}/disable", post(handle_program_disable))
        .route("/api/programs/{name}/host-requirements", get(handle_program_host_requirements))
        // Host tools
        .route("/api/host-tools", get(handle_host_tools_check))
        // MCP
        .route("/api/mcp/servers", get(handle_mcp_servers_list))
        .route("/api/mcp/servers", post(handle_mcp_server_register))
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
    /// Optional session ID for multi-turn conversations.
    #[serde(default)]
    session_id: String,
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
async fn handle_chat(
    state: State<Arc<AppState>>,
    Json(body): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, StatusCode> {
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
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
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
    state: State<Arc<AppState>>,
) -> Json<Vec<AgentSummary>> {
    match state.supervisor.list().await {
        Ok(agents) => Json(
            agents
                .into_iter()
                .map(|a| AgentSummary {
                    id: a.id.to_string(),
                    name: a.name,
                    status: format!("{:?}", a.status),
                    created_at: a.created_at.to_rfc3339(),
                    seed_id: a.seed_id.map(|s| s.to_string()),
                })
                .collect(),
        ),
        Err(_) => Json(Vec::new()),
    }
}

/// POST /api/agents/:id/kill — Kill an agent.
async fn handle_agent_kill(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    tracing::info!(agent_id = %id, "Kill agent requested");
    let agent_id = match Uuid::parse_str(&id) {
        Ok(uuid) => AgentId::from(uuid),
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };
    match state.supervisor.kill(agent_id).await {
        Ok(_) => Ok(StatusCode::OK),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// GET /api/config — Get current configuration.
async fn handle_config_get(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Serialize the actual config from AppState.
    let config = &*state.config;
    match serde_json::to_value(config) {
        Ok(json) => Ok(Json(json)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// PUT /api/config — Update configuration.
///
/// Validates the incoming JSON against the config schema and persists
/// changes to the config file on disk.
async fn handle_config_put(
    state: State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    tracing::info!("Config update requested");


    // Validate: parse as OxiosConfig to ensure the shape is correct.
    let updated: oxios_kernel::OxiosConfig = match serde_json::from_value(body.clone()) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::warn!(error = %e, "Invalid config shape");
            return Err((StatusCode::BAD_REQUEST, format!("Invalid config: {e}")));
        }
    };

    // Persist to the config file.
    if let Some(config_path) = &state.config_path {
        let content = toml::to_string_pretty(&updated)
            .map_err(|e: toml::ser::Error| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if let Err(e) = tokio::fs::write(config_path, content).await {
            tracing::error!(error = %e, "Failed to persist config");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
        }
        tracing::info!("Config persisted to {:?}", config_path);
    }

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
        if parent.canonicalize().is_err()
            && tokio::fs::create_dir_all(parent).await.is_err()
        {
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
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

/// Evolution lineage entry for a seed.
#[derive(Debug, Serialize)]
struct EvolutionEntry {
    /// Seed ID.
    id: String,
    /// Generation number.
    generation: u32,
    /// Goal at this generation.
    goal: String,
    /// Parent seed ID (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<String>,
    /// Evaluation score (if evaluated).
    #[serde(skip_serializing_if = "Option::is_none")]
    score: Option<f64>,
    /// Whether evaluation passed.
    #[serde(skip_serializing_if = "Option::is_none")]
    passed: Option<bool>,
}

/// GET /api/seeds/:id/evolution — Get evolution lineage for a seed.
async fn handle_seed_evolution(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<EvolutionEntry>>, StatusCode> {
    use oxios_ouroboros::Seed;
    // Helper to build lineage by following parent IDs.
    // Build lineage iteratively using a work stack.
    fn build_lineage_iterative(
        state_store: Arc<StateStore>,
        seed_id: String,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<EvolutionEntry>>> + Send>> {
        Box::pin(async move {
            let mut lineage = Vec::new();
            let mut stack = vec![seed_id];

            while let Some(current_id) = stack.pop() {
                let content = match state_store.load_markdown("seeds", &current_id).await {
                    Ok(Some(c)) => c,
                    _ => continue,
                };
                let seed: Seed = match serde_json::from_str(&content) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                // Push parent first so it's processed before children (reversed order).
                if let Some(ref parent_id) = seed.parent_seed_id {
                    stack.push(parent_id.to_string());
                }

                let (score, passed) = {
                    let eval_name = format!("{}-eval", current_id);
                    if let Ok(Some(eval_content)) =
                        state_store.load_markdown("evals", &eval_name).await
                    {
                        if let Ok(eval) =
                            serde_json::from_str::<oxios_ouroboros::EvaluationResult>(&eval_content)
                        {
                            (Some(eval.score), Some(eval.all_passed()))
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    }
                };

                lineage.push(EvolutionEntry {
                    id: seed.id.to_string(),
                    generation: seed.generation,
                    goal: seed.goal,
                    parent_id: seed.parent_seed_id.map(|p| p.to_string()),
                    score,
                    passed,
                });
            }

            lineage.reverse(); // Reverse so parent comes first.
            Ok(lineage)
        })
    }

    match build_lineage_iterative(state.state_store.clone(), id).await {
        Ok(lineage) if !lineage.is_empty() => Ok(Json(lineage)),
        _ => Err(StatusCode::NOT_FOUND),
    }
}

// ---------------------------------------------------------------------------
// Skills
// ---------------------------------------------------------------------------


/// Skill summary for listing.
#[derive(Debug, Serialize)]
struct SkillSummary {
    /// Skill name.
    name: String,
    /// Skill description.
    description: String,
}

/// GET /api/skills — List all skills.
async fn handle_skills_list(
    state: State<Arc<AppState>>,
) -> Json<Vec<SkillSummary>> {
    match state.skill_store.list_skills().await {
        Ok(skills) => Json(
            skills
                .into_iter()
                .map(|s| SkillSummary {
                    name: s.name,
                    description: s.description,
                })
                .collect(),
        ),
        Err(_) => Json(Vec::new()),
    }
}

/// GET /api/skills/:name — Get skill content.
async fn handle_skill_get(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.skill_store.load_skill(&name).await {
        Ok(Some(skill)) => Ok(Json(serde_json::json!({
            "name": skill.meta.name,
            "description": skill.meta.description,
            "content": skill.content,
            "path": skill.path.to_string_lossy(),
        }))),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Request body for creating a skill.
#[derive(Debug, Deserialize)]
struct SkillCreateRequest {
    /// Skill name.
    name: String,
    /// Skill description.
    description: String,
    /// Skill markdown content.
    #[serde(default)]
    content: String,
}

/// POST /api/skills — Create a new skill.
async fn handle_skill_create(
    state: State<Arc<AppState>>,
    Json(body): Json<SkillCreateRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match state
        .skill_store
        .create_skill(&body.name, &body.description, &body.content)
        .await
    {
        Ok(_) => {
            tracing::info!(skill = %body.name, "Skill created via API");
            Ok(Json(serde_json::json!({
                "status": "created",
                "name": body.name,
            })))
        }
        Err(e) => {
            tracing::error!(error = %e, skill = %body.name, "Failed to create skill");
            Err((StatusCode::BAD_REQUEST, e.to_string()))
        }
    }
}

/// DELETE /api/skills/:name — Delete a skill.
async fn handle_skill_delete(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match state.skill_store.delete_skill(&name).await {
        Ok(_) => {
            tracing::info!(skill = %name, "Skill deleted via API");
            Ok(Json(serde_json::json!({
                "status": "deleted",
                "name": name,
            })))
        }
        Err(e) => {
            tracing::error!(error = %e, skill = %name, "Failed to delete skill");
            Err((StatusCode::BAD_REQUEST, e.to_string()))
        }
    }
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
// Gardens
// ---------------------------------------------------------------------------

/// Garden summary for listing.
#[derive(Debug, Serialize)]
struct GardenSummary {
    /// Garden name.
    name: String,
    /// Image tag used.
    image_tag: String,
    /// Whether the garden is currently running.
    running: bool,
    /// Creation timestamp.
    created_at: String,
}

/// Request body for creating a garden.
#[derive(Debug, Deserialize)]
struct GardenCreateRequest {
    /// Name for the new garden.
    name: String,
}

/// Request body for executing a command in a garden.
#[derive(Debug, Deserialize)]
struct GardenExecRequest {
    /// Command to execute.
    command: Vec<String>,
    /// Working directory (optional, defaults to /workspace).
    #[serde(default)]
    workdir: Option<String>,
}

/// Response body for a garden exec command.
#[derive(Debug, Serialize)]
struct GardenExecResponse {
    /// Standard output.
    stdout: String,
    /// Standard error.
    stderr: String,
    /// Exit code.
    exit_code: i32,
    /// Duration in milliseconds.
    duration_ms: u64,
}

/// GET /api/gardens — List gardens.
async fn handle_gardens_list(
    state: State<Arc<AppState>>,
) -> Result<Json<Vec<GardenSummary>>, StatusCode> {
    let manager = state.garden_manager.clone();
    match manager.list_gardens().await {
        Ok(gardens) => {
            let summaries = gardens
                .into_iter()
                .map(|g| GardenSummary {
                    name: g.name,
                    image_tag: g.image_tag,
                    running: g.running,
                    created_at: g.created_at,
                })
                .collect();
            Ok(Json(summaries))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list gardens");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// POST /api/gardens — Create a new garden.
async fn handle_garden_create(
    state: State<Arc<AppState>>,
    Json(body): Json<GardenCreateRequest>,
) -> Result<Json<GardenSummary>, (StatusCode, String)> {
    let manager = state.garden_manager.clone();
    match manager.new_garden(&body.name).await {
        Ok(()) => {
            tracing::info!(garden = %body.name, "Garden created via API");
            Ok(Json(GardenSummary {
                name: body.name,
                image_tag: "oxios:latest".into(),
                running: false,
                created_at: chrono::Utc::now().to_rfc3339(),
            }))
        }
        Err(e) => {
            tracing::error!(error = %e, garden = %body.name, "Failed to create garden");
            Err((StatusCode::BAD_REQUEST, e.to_string()))
        }
    }
}

/// POST /api/gardens/:name/start — Start a garden container.
async fn handle_garden_start(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let manager = state.garden_manager.clone();
    match manager.start_garden(&name).await {
        Ok(()) => {
            tracing::info!(garden = %name, "Garden started via API");
            Ok(Json(serde_json::json!({"status": "started", "name": name})))
        }
        Err(e) => {
            tracing::error!(error = %e, garden = %name, "Failed to start garden");
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// POST /api/gardens/:name/stop — Stop a garden container.
async fn handle_garden_stop(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let manager = state.garden_manager.clone();
    match manager.stop_garden(&name).await {
        Ok(()) => {
            tracing::info!(garden = %name, "Garden stopped via API");
            Ok(Json(serde_json::json!({"status": "stopped", "name": name})))
        }
        Err(e) => {
            tracing::error!(error = %e, garden = %name, "Failed to stop garden");
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// DELETE /api/gardens/:name — Remove a garden.
async fn handle_garden_remove(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let manager = state.garden_manager.clone();
    match manager.remove_garden(&name).await {
        Ok(()) => {
            tracing::info!(garden = %name, "Garden removed via API");
            Ok(Json(serde_json::json!({"status": "removed", "name": name})))
        }
        Err(e) => {
            tracing::error!(error = %e, garden = %name, "Failed to remove garden");
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// POST /api/gardens/:name/exec — Execute a command in a garden.
async fn handle_garden_exec(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<GardenExecRequest>,
) -> Result<Json<GardenExecResponse>, (StatusCode, String)> {
    let manager = state.garden_manager.clone();
    match manager
        .exec_in_garden(&name, &body.command, body.workdir.as_deref())
        .await
    {
        Ok(result) => Ok(Json(GardenExecResponse {
            stdout: result.stdout,
            stderr: result.stderr,
            exit_code: result.exit_code,
            duration_ms: result.duration_ms,
        })),
        Err(e) => {
            tracing::error!(error = %e, garden = %name, "Failed to exec in garden");
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

// ---------------------------------------------------------------------------
// Scheduler (AIOS-inspired task scheduling)
// ---------------------------------------------------------------------------

/// Scheduler statistics response.
#[derive(Debug, Serialize)]
struct SchedulerStatsResponse {
    queued: usize,
    running: usize,
    max_concurrent: usize,
    rate_limit_per_minute: u32,
    rate_remaining: u32,
}

/// GET /api/scheduler/stats — Get scheduler statistics.
async fn handle_scheduler_stats(
    state: State<Arc<AppState>>,
) -> Json<SchedulerStatsResponse> {
    let stats = state.scheduler.stats();
    let rate_remaining = state.scheduler.rate_limit_remaining();
    Json(SchedulerStatsResponse {
        queued: stats.queued,
        running: stats.running,
        max_concurrent: stats.max_concurrent,
        rate_limit_per_minute: stats.rate_limit_per_minute,
        rate_remaining,
    })
}

/// Task summary for listing.
#[derive(Debug, Serialize)]
struct TaskSummary {
    id: String,
    description: String,
    priority: String,
    status: String,
    created_at: String,
    error: Option<String>,
}

/// GET /api/scheduler/tasks — List queued and running tasks.
async fn handle_scheduler_tasks(
    state: State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let queued: Vec<TaskSummary> = state
        .scheduler
        .queued_tasks()
        .into_iter()
        .map(|t| TaskSummary {
            id: t.id.to_string(),
            description: t.description,
            priority: format!("{:?}", t.priority),
            status: format!("{:?}", t.status),
            created_at: t.created_at.to_rfc3339(),
            error: t.error,
        })
        .collect();

    let running: Vec<TaskSummary> = state
        .scheduler
        .running_tasks()
        .into_iter()
        .map(|t| TaskSummary {
            id: t.id.to_string(),
            description: t.description,
            priority: format!("{:?}", t.priority),
            status: format!("{:?}", t.status),
            created_at: t.created_at.to_rfc3339(),
            error: t.error,
        })
        .collect();

    Json(serde_json::json!({
        "queued": queued,
        "running": running,
    }))
}

// ---------------------------------------------------------------------------
// Audit & Permissions
// ---------------------------------------------------------------------------

/// Audit log entry response.
#[derive(Debug, Serialize)]
struct AuditEntryResponse {
    timestamp: String,
    agent_name: String,
    action: String,
    resource: String,
    allowed: bool,
    reason: Option<String>,
}

impl From<&AuditEntry> for AuditEntryResponse {
    fn from(entry: &AuditEntry) -> Self {
        Self {
            timestamp: entry.timestamp.to_rfc3339(),
            agent_name: entry.agent_name.clone(),
            action: entry.action.clone(),
            resource: entry.resource.clone(),
            allowed: entry.allowed,
            reason: entry.reason.clone(),
        }
    }
}

/// GET /api/audit — Get security audit log.
async fn handle_audit_log(
    state: State<Arc<AppState>>,
) -> Json<Vec<AuditEntryResponse>> {
    let access = state.access_manager.lock();
    let entries: Vec<AuditEntryResponse> = access
        .audit_log()
        .iter()
        .map(AuditEntryResponse::from)
        .collect();
    Json(entries)
}

/// Permissions response.
#[derive(Debug, Serialize)]
struct PermissionsResponse {
    agent_name: String,
    allowed_tools: Vec<String>,
    allowed_paths: Vec<String>,
    denied_paths: Vec<String>,
    network_access: bool,
    max_execution_time_secs: u64,
    max_memory_mb: u64,
    can_fork: bool,
}

/// GET /api/permissions/:agent — Get permissions for an agent.
async fn handle_permissions_get(
    state: State<Arc<AppState>>,
    Path(agent): Path<String>,
) -> Result<Json<PermissionsResponse>, StatusCode> {
    let access = state.access_manager.lock();
    match access.get_permissions(&agent) {
        Some(perms) => Ok(Json(PermissionsResponse {
            agent_name: perms.agent_name.clone(),
            allowed_tools: perms.allowed_tools.iter().cloned().collect(),
            allowed_paths: perms.allowed_paths.clone(),
            denied_paths: perms.denied_paths.clone(),
            network_access: perms.network_access,
            max_execution_time_secs: perms.max_execution_time_secs,
            max_memory_mb: perms.max_memory_mb,
            can_fork: perms.can_fork,
        })),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// PUT /api/permissions/:agent — Set permissions for an agent.
#[derive(Debug, Deserialize)]
struct PermissionsUpdate {
    allowed_tools: Option<Vec<String>>,
    allowed_paths: Option<Vec<String>>,
    denied_paths: Option<Vec<String>>,
    network_access: Option<bool>,
    max_execution_time_secs: Option<u64>,
    max_memory_mb: Option<u64>,
    can_fork: Option<bool>,
}

async fn handle_permissions_put(
    state: State<Arc<AppState>>,
    Path(agent): Path<String>,
    Json(body): Json<PermissionsUpdate>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut access = state.access_manager.lock();
    let perms = access.get_or_create_permissions(&agent);

    if let Some(tools) = body.allowed_tools {
        perms.allowed_tools = tools.into_iter().collect();
    }
    if let Some(paths) = body.allowed_paths {
        perms.allowed_paths = paths;
    }
    if let Some(paths) = body.denied_paths {
        perms.denied_paths = paths;
    }
    if let Some(v) = body.network_access {
        perms.network_access = v;
    }
    if let Some(v) = body.max_execution_time_secs {
        perms.max_execution_time_secs = v;
    }
    if let Some(v) = body.max_memory_mb {
        perms.max_memory_mb = v;
    }
    if let Some(v) = body.can_fork {
        perms.can_fork = v;
    }

    tracing::info!(agent = %agent, "Permissions updated");
    Ok(Json(serde_json::json!({
        "status": "updated",
        "agent": agent,
    })))
}

// ---------------------------------------------------------------------------
// Programs (OS-level installable applications)
// ---------------------------------------------------------------------------

/// Program summary for listing.
#[derive(Debug, Serialize)]
struct ProgramSummary {
    name: String,
    version: String,
    description: String,
    author: String,
    enabled: bool,
    tools_count: usize,
    has_skill_content: bool,
}

/// GET /api/programs — List all installed programs.
async fn handle_programs_list(
    state: State<Arc<AppState>>,
) -> Json<Vec<ProgramSummary>> {
    let programs = state.program_manager.list_programs().await;
    Json(programs
        .into_iter()
        .map(|p| ProgramSummary {
            name: p.meta.name,
            version: p.meta.version,
            description: p.meta.description,
            author: p.meta.author,
            enabled: p.enabled,
            tools_count: p.meta.tools.len(),
            has_skill_content: !p.skill_content.is_empty(),
        })
        .collect())
}

/// GET /api/programs/:name — Get program details.
async fn handle_program_get(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.program_manager.get_program(&name).await {
        Some(program) => Ok(Json(serde_json::json!({
            "name": program.meta.name,
            "version": program.meta.version,
            "description": program.meta.description,
            "author": program.meta.author,
            "enabled": program.enabled,
            "tools": program.meta.tools,
            "skill_content": program.skill_content,
            "path": program.path.to_string_lossy(),
        }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Request body for program installation.
#[derive(Debug, Deserialize)]
struct ProgramInstallRequest {
    /// Path to the program directory to install.
    path: String,
}

/// POST /api/programs — Install a program from a directory.
async fn handle_program_install(
    state: State<Arc<AppState>>,
    Json(body): Json<ProgramInstallRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let path = std::path::PathBuf::from(&body.path);
    match state.program_manager.install(&path).await {
        Ok(program) => {
            tracing::info!(program = %program.meta.name, "Program installed via API");
            Ok(Json(serde_json::json!({
                "status": "installed",
                "name": program.meta.name,
                "version": program.meta.version,
            })))
        }
        Err(e) => {
            tracing::error!(error = %e, path = %body.path, "Failed to install program");
            Err((StatusCode::BAD_REQUEST, e.to_string()))
        }
    }
}

/// DELETE /api/programs/:name — Uninstall a program.
async fn handle_program_uninstall(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match state.program_manager.uninstall(&name).await {
        Ok(()) => {
            tracing::info!(program = %name, "Program uninstalled via API");
            Ok(Json(serde_json::json!({"status": "uninstalled", "name": name})))
        }
        Err(e) => {
            tracing::error!(error = %e, program = %name, "Failed to uninstall program");
            Err((StatusCode::BAD_REQUEST, e.to_string()))
        }
    }
}

/// POST /api/programs/:name/enable — Enable a program.
async fn handle_program_enable(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match state.program_manager.set_enabled(&name, true).await {
        Ok(()) => Ok(Json(serde_json::json!({"status": "enabled", "name": name}))),
        Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
    }
}

/// POST /api/programs/:name/disable — Disable a program.
async fn handle_program_disable(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match state.program_manager.set_enabled(&name, false).await {
        Ok(()) => Ok(Json(serde_json::json!({"status": "disabled", "name": name}))),
        Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
    }
}

/// GET /api/programs/:name/host-requirements — Check host requirements for a program.
async fn handle_program_host_requirements(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match state.program_manager.check_host_requirements(&name).await {
        Ok(check) => Ok(Json(serde_json::to_value(&check).unwrap())),
        Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Host Tools
// ---------------------------------------------------------------------------

/// Host tools status response.
#[derive(Debug, Serialize)]
struct HostToolsStatusResponse {
    all_required_present: bool,
    missing_required: Vec<String>,
    optional_available: std::collections::HashMap<String, bool>,
}

/// GET /api/host-tools — Check host tool availability.
async fn handle_host_tools_check(
    state: State<Arc<AppState>>,
) -> Json<HostToolsStatusResponse> {
    let status = state.host_tool_validator.full_check();
    Json(HostToolsStatusResponse {
        all_required_present: status.all_required_present,
        missing_required: status.missing_required,
        optional_available: status.optional_available,
    })
}

// ---------------------------------------------------------------------------
// MCP (Model Context Protocol)
// ---------------------------------------------------------------------------

/// MCP server configuration response.
#[derive(Debug, Serialize)]
struct McpServerResponse {
    name: String,
    command: String,
    args: Vec<String>,
    enabled: bool,
}

/// GET /api/mcp/servers — List registered MCP servers (stub).
async fn handle_mcp_servers_list(
    _state: State<Arc<AppState>>,
) -> Json<Vec<McpServerResponse>> {
    // Stub: Return empty list until MCP server storage is implemented
    Json(Vec::new())
}

/// MCP server registration request.
#[derive(Debug, Deserialize)]
struct McpServerRegisterRequest {
    name: String,
    command: String,
    #[allow(dead_code)]
    args: Option<Vec<String>>,
}

/// POST /api/mcp/servers — Register an MCP server (stub).
async fn handle_mcp_server_register(
    _state: State<Arc<AppState>>,
    Json(body): Json<McpServerRegisterRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Stub: MCP server storage not yet implemented
    tracing::info!(server = %body.name, command = %body.command, "MCP server registration requested (stub)");
    Ok(Json(serde_json::json!({
        "status": "registered",
        "name": body.name,
        "note": "MCP integration is stubbed; full implementation requires stdio communication"
    })))
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
