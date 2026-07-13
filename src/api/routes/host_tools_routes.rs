//! Host-tools discovery API — `/api/host-tools` (RFC-041 Phase 1).
//!
//! Returns the live host-CLI inventory from the cached `HostToolsApi` scanner.
//! Optional `?names=gh,npm` restricts detection to a subset; without it a
//! sensible default set (package managers + common CLIs) is scanned.

use std::sync::Arc;

use axum::{Json, extract::Query, extract::State};
use serde::Deserialize;

use crate::api::error::AppError;
use crate::api::server::AppState;

/// `GET /api/host-tools?names=gh,npm,cargo` query.
#[derive(Debug, Default, Deserialize)]
pub struct HostToolsQuery {
    /// Comma-separated binary names to detect. Omit for the default set.
    pub names: Option<String>,
    /// Force a cache invalidation before scanning.
    #[serde(default)]
    pub rescan: bool,
}

/// Default binaries probed when `?names` is absent — the package managers the
/// provisioning layer bootstraps on, plus a few common CLIs. Cheap to extend.
const DEFAULT_NAMES: &[&str] = &[
    // package managers
    "brew", "npm", "cargo", "bun", "go", "uv", "pip", "pip3", // common CLIs
    "gh", "git", "node", "rg", "fd", "resend",
];

/// `GET /api/host-tools` — cached host-CLI inventory.
pub(crate) async fn handle_host_tools(
    state: State<Arc<AppState>>,
    Query(q): Query<HostToolsQuery>,
) -> Result<Json<Vec<oxios_kernel::DetectedTool>>, AppError> {
    if q.rescan {
        state.kernel.host_tools.invalidate();
    }
    let names: Vec<String> = match q.names {
        Some(n) => n
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        None => DEFAULT_NAMES.iter().map(|s| s.to_string()).collect(),
    };
    let tools = state.kernel.host_tools.detect_many(&names).await;
    Ok(Json(tools))
}

/// `POST /api/host-tools/detect` — force a fresh scan (alias for `?rescan=true`).
pub(crate) async fn handle_host_tools_detect(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    state.kernel.host_tools.invalidate();
    Ok(Json(
        serde_json::json!({ "status": "ok", "message": "scan cache invalidated" }),
    ))
}
