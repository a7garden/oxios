//! Web channel plugin.
//!
//! Factory for creating the web channel and its axum HTTP/WS server.
//! Implements [`ChannelPlugin`](oxios_gateway::plugin::ChannelPlugin) so
//! the main binary can activate the web channel from configuration.
//!
//! **Auto-update**: The web UI can be updated by placing a new build in
//! `~/.oxios/web/dist/` (checked first) or `<workspace>/web/dist/` (fallback).
//! The server reads from filesystem on every request — no restart needed.

use anyhow::Result;
use async_trait::async_trait;
use axum::{
    body::Body,
    extract::Path,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::Embed;
use std::path::PathBuf;
use std::sync::Arc;

use oxios_gateway::plugin::{ChannelBundle, ChannelContext, ChannelPlugin};
use oxios_markdown::KnowledgeBase;

use crate::api_docs;
use crate::channel::{WebChannel, WebChannelHandle};
use crate::middleware::RateLimiter;
use crate::routes;
use crate::server::AppState;

/// Static web assets — embedded at compile time via rust-embed.
/// This is the fallback when no external web/dist/ is found.
/// Build with: cd web && npm run build
#[derive(Embed)]
#[folder = "web/dist/"]
struct EmbeddedAssets;

// ---------------------------------------------------------------------------
// Web dist path resolution
// ---------------------------------------------------------------------------

/// Returns the active web dist directory, checked in order:
/// 1. `~/.oxios/web/dist/` — user-provided or updated builds (highest priority)
/// 2. `<workspace>/web/dist/` — development / bundled builds
///
/// Returns `None` if neither exists.
fn web_dist_path(workspace: &std::path::Path) -> Option<std::path::PathBuf> {
    // 1. ~/.oxios/web/dist/ (user updates)
    let user_dist = dirs::home_dir()
        .map(|h| h.join(".oxios").join("web").join("dist"))
        .filter(|p| p.exists() && p.join("index.html").is_file());

    if user_dist.is_some() {
        tracing::info!(
            path = ?user_dist.as_ref().unwrap(),
            "Serving web UI from user override (~/.oxios/web/dist/)"
        );
        return user_dist;
    }

    // 2. workspace/web/dist/ (bundled / dev)
    let workspace_dist = workspace.join("web").join("dist");
    if workspace_dist.join("index.html").is_file() {
        tracing::info!(
            path = ?workspace_dist,
            "Serving web UI from workspace (web/dist/)"
        );
        return Some(workspace_dist);
    }

    // 3. embedded fallback
    if EmbeddedAssets::get("index.html").is_some() {
        tracing::info!("Serving web UI from embedded assets (binary built with --features web)");
        return None;
    }

    None
}

// ---------------------------------------------------------------------------
// Filesystem serving (no caching — reads fresh every request for auto-update)
// ---------------------------------------------------------------------------

/// Reads a file from the filesystem dist directory.
/// Returns `None` if the file doesn't exist.
fn fs_read(dist: &std::path::Path, path: &str) -> Option<Vec<u8>> {
    let clean = path.trim_start_matches('/');
    let file_path = dist.join(clean);
    std::fs::read(&file_path).ok()
}

/// Determines MIME type from file path.
fn mime_type(path: &str) -> axum::http::HeaderValue {
    let clean = path.trim_start_matches('/');
    mime_guess::from_path(clean)
        .first_or_octet_stream()
        .to_string()
        .parse()
        .unwrap_or_else(|_| axum::http::HeaderValue::from_static("application/octet-stream"))
}

/// Serve a static file — filesystem first, then embedded fallback.
fn serve_file(dist: Option<&std::path::Path>, path: &str) -> Response {
    let clean = path.trim_start_matches('/');

    // Try filesystem first
    if let Some(d) = dist {
        if let Some(data) = fs_read(d, &clean) {
            let lookup = if clean.starts_with("assets/") {
                clean.to_string()
            } else {
                format!("assets/{}", clean)
            };
            return Response::builder()
                .status(200)
                .header("Content-Type", mime_type(&lookup))
                .header("Cache-Control", "no-cache") // disable caching for auto-update
                .body(Body::from(data))
                .unwrap();
        }
        // Try without assets/ prefix
        if let Some(data) = fs_read(d, &format!("assets/{}", clean)) {
            return Response::builder()
                .status(200)
                .header("Content-Type", mime_type(&clean))
                .header("Cache-Control", "no-cache")
                .body(Body::from(data))
                .unwrap();
        }
    }

    // Fall back to embedded assets
    let asset =
        EmbeddedAssets::get(clean).or_else(|| EmbeddedAssets::get(&format!("assets/{}", clean)));

    match asset {
        Some(content) => {
            let lookup = if clean.starts_with("assets/") {
                clean.to_string()
            } else {
                format!("assets/{}", clean)
            };
            let mime = mime_guess::from_path(&lookup)
                .first_or_octet_stream()
                .to_string();
            Response::builder()
                .status(200)
                .header("Content-Type", mime)
                .body(Body::from(content.data.to_vec()))
                .unwrap()
        }
        None => Response::builder().status(404).body(Body::empty()).unwrap(),
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Static asset handler.
async fn static_handler(
    path: axum::extract::Path<String>,
    state: axum::extract::State<Arc<AppState>>,
) -> Response {
    let dist = state.web_dist.clone();
    serve_file(dist.as_deref(), &path)
}

/// SPA fallback — serves index.html for client-side routing.
async fn spa_handler(axum::extract::State(state): axum::extract::State<Arc<AppState>>) -> Response {
    // Try filesystem first
    if let Some(ref dist) = state.web_dist {
        if let Some(data) = fs_read(dist, "index.html") {
            return Response::builder()
                .status(200)
                .header("Content-Type", "text/html; charset=utf-8")
                .header("Cache-Control", "no-cache")
                .body(Body::from(data))
                .unwrap();
        }
    }

    // Fall back to embedded
    match EmbeddedAssets::get("index.html") {
        Some(content) => Response::builder()
            .status(200)
            .header("Content-Type", "text/html; charset=utf-8")
            .body(Body::from(content.data.to_vec()))
            .unwrap(),
        None => Response::builder()
            .status(404)
            .body(Body::from(
                "index.html not found — run `cd web && npm run build`",
            ))
            .unwrap(),
    }
}

// ---------------------------------------------------------------------------
// WebPlugin
// ---------------------------------------------------------------------------

/// Web channel plugin — creates an axum HTTP/WS server.
pub struct WebPlugin;

impl WebPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WebPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChannelPlugin for WebPlugin {
    fn name(&self) -> &str {
        "web"
    }

    async fn setup(&self, ctx: ChannelContext) -> Result<ChannelBundle> {
        let config = ctx.config.read().clone();
        let host = config.gateway.host.clone();
        let port = config.gateway.port;
        let rate_limit = config.security.rate_limit_per_minute;
        let workspace = PathBuf::from(&config.kernel.workspace);

        // Resolve web dist path (filesystem override or embedded fallback)
        let web_dist = web_dist_path(&workspace);

        // Create web channel
        let web_channel = WebChannel::new(256);
        let channel_handle = WebChannelHandle::from_channel(&web_channel);

        // Create KnowledgeBase for direct markdown CRUD (no kernel dependency)
        let knowledge_root = workspace.join("knowledge");
        let knowledge = match KnowledgeBase::new(knowledge_root) {
            Ok(kb) => Arc::new(kb),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to create KnowledgeBase at workspace, using temp dir");
                let fallback_dir = std::env::temp_dir().join("oxios-web-knowledge");
                Arc::new(
                    KnowledgeBase::new(fallback_dir)
                        .expect("Failed to create fallback KnowledgeBase"),
                )
            }
        };

        // Build app state
        let state = Arc::new(AppState {
            base_url: format!("http://{}:{}", host, port),
            knowledge,
            kernel: ctx.kernel.clone(),
            channel: channel_handle,
            config: ctx.config.clone(),
            config_path: ctx.config_path.clone(),
            start_time: ctx.kernel.start_time(),
            rate_limiter: RateLimiter::new(rate_limit),
            web_dist,
        });

        // Build API routes
        let api_routes = routes::build_routes(state.clone());

        // CORS layer
        let cors_origins: Vec<_> = config
            .security
            .cors_origins
            .iter()
            .filter_map(|o| o.parse::<axum::http::HeaderValue>().ok())
            .collect();
        let cors = tower_http::cors::CorsLayer::new()
            .allow_origin(cors_origins)
            .allow_methods(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any);

        // OpenAPI / Swagger UI
        let openapi = api_docs::build_openapi();
        let swagger: Router<()> = utoipa_swagger_ui::SwaggerUi::new("/api-docs")
            .url("/openapi.json", openapi)
            .into();

        // SPA routes
        let spa_routes: Router<Arc<AppState>> = Router::new()
            .route("/assets/{*path}", get(static_handler))
            .route("/favicon.svg", get(static_handler))
            .route("/icons.svg", get(static_handler))
            .route("/{*path}", get(spa_handler))
            .route("/", get(spa_handler));

        let app = Router::new()
            .merge(api_routes)
            .merge(spa_routes)
            .layer(cors)
            .nest_service("/api-docs", swagger)
            .with_state(state);

        // Bind listener
        let addr = format!("{}:{}", host, port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        tracing::info!(addr = %addr, "Web server listening");

        // Spawn server
        let handle = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    tokio::signal::ctrl_c().await.ok();
                    tracing::info!("Web server shutting down");
                })
                .await
            {
                tracing::error!(error = %e, "Web server error");
            }
        });

        Ok(ChannelBundle {
            channel: Box::new(web_channel),
            tasks: vec![handle],
        })
    }
}
