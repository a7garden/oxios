//! Tool registry API — list available agent tools for the settings UI.
//!
//! `GET /api/tools/registry` — Returns all known tool metadata
//! (name, description key, category) for the `allowed_tools` multi-select
//! widget in the frontend settings.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;

use crate::server::AppState;

/// GET /api/tools/registry — List all known tool metadata.
pub(crate) async fn handle_tools_registry(state: State<Arc<AppState>>) -> Json<serde_json::Value> {
    let tools = state.kernel.infra.list_available_tools();
    Json(serde_json::json!({ "tools": tools }))
}
