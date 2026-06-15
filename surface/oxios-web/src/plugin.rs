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
use axum::{Router, body::Body, response::Response, routing::get};
use rust_embed::Embed;
use std::sync::Arc;

use oxios_gateway::surface::{Surface, SurfaceContext, SurfaceHandle};

use crate::api_docs;
use crate::bridge::{WebBridge, WebBridgeHandle};
use crate::middleware::RateLimiter;
use crate::routes;
use crate::server::AppState;
use oxios_gateway::ReliabilityLayer;

/// Static web assets — embedded at compile time via rust-embed.
/// This is the fallback when no external web/dist/ is found.
/// Build with: cd web && npm run build
#[derive(Embed)]
#[folder = "web/dist/"]
struct EmbeddedAssets;

// ---------------------------------------------------------------------------
// Filesystem serving (RFC-024 SP3: atomic pointer + immutable cache)
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

/// Whether `path` is a content-addressed asset (hashed filename under
/// `assets/`). Such files are safe to cache as immutable because a new
/// build emits new hashes, so the URL itself is the cache key.
fn is_immutable_asset(path: &str) -> bool {
    let clean = path.trim_start_matches('/');
    clean.starts_with("assets/")
}

/// Read the active web version from `<dist>/version.json` (for the
/// `X-Web-Version` header). Returns `"dev"` when not present.
fn read_active_version(dist: &std::path::Path) -> String {
    #[derive(serde::Deserialize)]
    struct VersionFile {
        version: Option<String>,
    }
    std::fs::read(dist.join("version.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<VersionFile>(&b).ok())
        .and_then(|v| v.version)
        .unwrap_or_else(|| "dev".to_string())
}

/// Serve a static file.
///
/// **RFC-024 C3 (3-source consistency):** when an active dist is published
/// (`Some`), we serve *only* from it and never fall back to embedded assets.
/// This guarantees a request never mixes two build hashes. Embedded assets
/// are used only when no active dist exists (startup download failure, etc.).
fn serve_file(dist: Option<&std::path::Path>, path: &str) -> Response {
    let clean = path.trim_start_matches('/');

    // ── Active dist path ──
    if let Some(d) = dist {
        // Resolve the on-disk name (dist files live either at root or under assets/).
        let data = fs_read(d, clean).or_else(|| fs_read(d, &format!("assets/{clean}")));
        let Some(data) = data else {
            // Deliberately NOT falling back to embedded: the active dist is
            // self-consistent; a missing file here is a real miss, not a
            // hash mismatch to paper over.
            return Response::builder().status(404).body(Body::empty()).unwrap();
        };
        let lookup = if clean.starts_with("assets/") {
            clean.to_string()
        } else {
            format!("assets/{clean}")
        };
        let cache = if is_immutable_asset(&lookup) {
            // Content-addressed → safe to cache forever.
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        return Response::builder()
            .status(200)
            .header("Content-Type", mime_type(&lookup))
            .header("Cache-Control", cache)
            .body(Body::from(data))
            .unwrap();
    }

    // ── No active dist → embedded fallback ──
    let asset =
        EmbeddedAssets::get(clean).or_else(|| EmbeddedAssets::get(&format!("assets/{clean}")));
    match asset {
        Some(content) => {
            let lookup = if clean.starts_with("assets/") {
                clean.to_string()
            } else {
                format!("assets/{clean}")
            };
            let cache = if is_immutable_asset(&lookup) {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            let mime = mime_guess::from_path(lookup.as_str())
                .first_or_octet_stream()
                .to_string();
            Response::builder()
                .status(200)
                .header("Content-Type", mime)
                .header("Cache-Control", cache)
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
    // RFC-024 SP3: load the atomic pointer per request (O(1)).
    let dist = state.web_dist.path();
    serve_file(dist.as_deref(), &path)
}

/// SPA fallback — serves index.html for client-side routing.
async fn spa_handler(axum::extract::State(state): axum::extract::State<Arc<AppState>>) -> Response {
    // RFC-024 SP3: load the atomic pointer per request.
    let dist = state.web_dist.path();

    // Active dist: serve its index.html, annotated with the active version
    // so clients can detect a version switch (3-source consistency). index.html
    // is never cached immutably — it is the pointer to the hashed assets.
    if let Some(ref d) = dist
        && let Some(data) = fs_read(d, "index.html")
    {
        let version = read_active_version(d);
        return Response::builder()
            .status(200)
            .header("Content-Type", "text/html; charset=utf-8")
            .header("Cache-Control", "no-cache")
            .header("X-Web-Version", version)
            .body(Body::from(data))
            .unwrap();
    }

    // Fall back to embedded
    match EmbeddedAssets::get("index.html") {
        Some(content) => Response::builder()
            .status(200)
            .header("Content-Type", "text/html; charset=utf-8")
            .header("Cache-Control", "no-cache")
            .header("X-Web-Version", "embedded")
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

        // Create web channel for gateway message routing. Each web bridge
        // owns its own reliability layer (RFC-024 SP2): the gateway's
        // global layer is the source of truth, but the bridge layer is
        // what WS resume handlers query for replay.
        let web_channel = WebBridge::new(256, Arc::new(ReliabilityLayer::new(Default::default())));
        // RFC-024 SP1 / C1: pull the response timeout from config so
        // operators can tune the HTTP→gateway ceiling per environment.
        let response_timeout = std::time::Duration::from_secs(config.gateway.response_timeout_secs);
        let bridge_handle =
            WebBridgeHandle::from_bridge(&web_channel).with_response_timeout(response_timeout);

        // Build app state — all knowledge access goes through kernel.knowledge
        let state = Arc::new(AppState {
            base_url: format!("http://{host}:{port}"),
            kernel: ctx.kernel.clone(),
            bridge: bridge_handle,
            config: ctx.config.clone(),
            config_path: ctx.config_path.clone(),
            start_time: ctx.kernel.start_time(),
            rate_limiter: RateLimiter::new(rate_limit),
            memory_map_cache: routes::MemoryMapCache::default(),
            web_dist,
            readiness: ctx.kernel.readiness.clone(),
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

        // OpenAPI / Swagger UI — gated by `gateway.expose_api_docs` AND
        // a loopback bind. See `GatewayConfig::should_expose_api_docs`.
        let should_expose_docs = state.config.read().gateway.should_expose_api_docs();

        // SPA routes (defined first so we can merge them into `app`).
        let spa_routes: Router<Arc<AppState>> = Router::new()
            .route("/assets/{*path}", get(static_handler))
            .route("/favicon.svg", get(static_handler))
            .route("/icons.svg", get(static_handler))
            .route("/{*path}", get(spa_handler))
            .route("/", get(spa_handler));

        let mut app = Router::new()
            .merge(api_routes)
            .merge(spa_routes)
            .layer(cors);

        if should_expose_docs {
            let openapi = api_docs::build_openapi();
            let swagger: Router<()> = utoipa_swagger_ui::SwaggerUi::new("/api-docs")
                .url("/openapi.json", openapi)
                .into();
            app = app.nest_service("/api-docs", swagger);
            tracing::info!("API docs exposed at /api-docs and /openapi.json");
        } else {
            tracing::info!(
                "API docs disabled (set gateway.expose_api_docs=true on a loopback bind to enable)"
            );
        }

        let app = app.with_state(state);

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
