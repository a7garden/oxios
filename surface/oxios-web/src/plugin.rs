//! Web surface plugin.
//!
//! Factory for creating the web control surface. Implements
//! [`Surface`](crate::surface::Surface) so the main binary can
//! activate the web dashboard with full kernel access.
//!
//! The web surface is both a control plane (kernel management,
//! monitoring, configuration) and a message interface (chat via gateway).
//!
//! **Auto-update**: The web UI can be updated by placing a new build in
//! `~/.oxios/web/dist/` (checked first) or `<workspace>/web/dist/` (fallback).
//! If neither exists, the web UI is automatically downloaded from GitHub Releases.
//! The server reads from filesystem on every request — no restart needed.

use anyhow::Result;
use async_trait::async_trait;
use axum::{body::Body, response::Response, routing::get, Router};
use rust_embed::Embed;
use std::sync::Arc;

use oxios_gateway::surface::{Surface, SurfaceContext, SurfaceHandle};

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
        if let Some(data) = fs_read(d, clean) {
            let lookup = if clean.starts_with("assets/") {
                clean.to_string()
            } else {
                format!("assets/{clean}")
            };
            return Response::builder()
                .status(200)
                .header("Content-Type", mime_type(&lookup))
                .header("Cache-Control", "no-cache") // disable caching for auto-update
                .body(Body::from(data))
                .unwrap();
        }
        // Try without assets/ prefix
        if let Some(data) = fs_read(d,&format!("assets/{clean}")) {
            return Response::builder()
                .status(200)
                .header("Content-Type", mime_type(clean))
                .header("Cache-Control", "no-cache")
                .body(Body::from(data))
                .unwrap();
        }
    }

    // Fall back to embedded assets
    let asset =
        EmbeddedAssets::get(clean).or_else(|| EmbeddedAssets::get(&format!("assets/{clean}")));

    match asset {
        Some(content) => {
            let lookup = if clean.starts_with("assets/") {
                clean.to_string()
            } else {
                format!("assets/{clean}")
            };
            let mime = mime_guess::from_path(lookup.as_str())
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

/// Web surface — kernel-connected control dashboard.
pub struct WebSurface;

impl WebSurface {
    /// Create a new web surface instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WebSurface {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Surface for WebSurface {
    fn name(&self) -> &str {
        "web"
    }

    async fn start(&self, ctx: SurfaceContext) -> Result<SurfaceHandle> {
        let config = ctx.config.read().clone();
        let host = config.gateway.host.clone();
        let port = config.gateway.port;
        let rate_limit = config.security.rate_limit_per_minute;

        // Use the pre-resolved web dist path from SurfaceContext.
        // `web_dist.rs` in the binary already downloaded it before this surface starts.
        // `None` here means we'll fall back to embedded assets.
        let web_dist = ctx.web_dist;

        // Create web channel for gateway message routing
        let web_channel = WebChannel::new(256);
        let channel_handle = WebChannelHandle::from_channel(&web_channel);

        // Build app state — all knowledge access goes through kernel.knowledge
        let state = Arc::new(AppState {
            base_url: format!("http://{host}:{port}"),
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
        let addr = format!("{host}:{port}");
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

        Ok(SurfaceHandle {
            channel: Some(Box::new(web_channel)),
            tasks: vec![handle],
        })
    }
}
