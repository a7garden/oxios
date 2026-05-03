//! API routes for the web channel.
//!
//! Provides three route groups:
//! - **Chat**: POST /api/chat for sending messages
//! - **Control**: GET /api/status for system status
//! - **Browse**: GET /api/workspace/* for browsing markdown files

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::server::AppState;

/// Builds the axum router with all API routes.
pub fn build_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/chat", post(handle_chat))
        .route("/api/status", get(handle_status))
        .route("/api/workspace/{*path}", get(handle_workspace))
}

/// Request body for the chat endpoint.
#[derive(Debug, Deserialize)]
struct ChatRequest {
    /// The user's message content.
    content: String,
}

/// Response body for the chat endpoint.
#[derive(Debug, Serialize)]
struct ChatResponse {
    /// Echo of the user's message.
    echo: String,
    /// Placeholder response.
    reply: String,
}

/// POST /api/chat — Send a message to the kernel.
async fn handle_chat(
    _state: State<Arc<AppState>>,
    Json(body): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, StatusCode> {
    tracing::info!(content = %body.content, "Chat message received");
    Ok(Json(ChatResponse {
        echo: body.content,
        reply: "Oxios received your message. Agent execution not yet connected.".into(),
    }))
}

/// Response body for the status endpoint.
#[derive(Debug, Serialize)]
struct StatusResponse {
    /// Service name.
    service: String,
    /// Current status.
    status: String,
    /// API version.
    version: String,
}

/// GET /api/status — System status.
async fn handle_status(
    _state: State<Arc<AppState>>,
) -> Json<StatusResponse> {
    Json(StatusResponse {
        service: "oxios".into(),
        status: "running".into(),
        version: "0.1.0-alpha".into(),
    })
}

/// Response for workspace browsing.
#[derive(Debug, Serialize)]
struct WorkspaceResponse {
    /// Requested path.
    path: String,
    /// Content or listing.
    content: String,
}

/// GET /api/workspace/* — Browse markdown files in the workspace.
async fn handle_workspace(
    _state: State<Arc<AppState>>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Json<WorkspaceResponse> {
    // TODO: connect to StateStore for real file browsing
    Json(WorkspaceResponse {
        path,
        content: "Workspace browsing not yet connected to state store.".into(),
    })
}
