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
use std::path::{Path, PathBuf};
use std::sync::Arc;

use oxios_gateway::surface::{Surface, SurfaceContext, SurfaceHandle};

use crate::api_docs;
use crate::channel::{WebChannel, WebChannelHandle};
use crate::middleware::RateLimiter;
use crate::routes;
use crate::server::AppState;

const GITHUB_REPO: &str = "a7garden/oxios";

/// Static web assets — embedded at compile time via rust-embed.
/// This is the fallback when no external web/dist/ is found.
/// Build with: cd web && npm run build
#[derive(Embed)]
#[folder = "web/dist/"]
struct EmbeddedAssets;

// ---------------------------------------------------------------------------
// Web dist auto-download from GitHub Releases
// ---------------------------------------------------------------------------

/// Returns `~/.oxios/web/dist/` path.
fn user_web_dist_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".oxios").join("web").join("dist"))
}

/// Returns `~/.oxios/web/version` path.
fn user_web_version_file() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".oxios").join("web").join("version"))
}

/// Fetches the latest release tag from GitHub API.
async fn fetch_latest_release_tag() -> Result<String> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let client = reqwest::Client::builder().user_agent("oxios-web").build()?;
    let resp: serde_json::Value = client.get(&url).send().await?.json().await?;
    let tag = resp["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("tag_name not found in GitHub response"))?;
    Ok(tag.to_string())
}

/// Downloads `web-dist.zip` from a GitHub release and extracts to `~/.oxios/web/dist/`.
async fn download_and_extract_web_dist(version_tag: &str) -> Result<PathBuf> {
    let dist_dir =
        user_web_dist_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    let version_file = user_web_version_file()
        .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;

    // Download zip
    let url =
        format!("https://github.com/{GITHUB_REPO}/releases/download/{version_tag}/web-dist.zip");
    tracing::info!(url = %url, "Downloading web UI from GitHub Releases");

    let client = reqwest::Client::builder().user_agent("oxios-web").build()?;
    let resp = client.get(&url).send().await?;

    if !resp.status().is_success() {
        anyhow::bail!("Failed to download web-dist.zip: HTTP {}", resp.status());
    }

    let bytes = resp.bytes().await?;

    // Extract zip
    let reader = std::io::Cursor::new(bytes.as_ref());
    let mut archive = zip::ZipArchive::new(reader)?;

    // Clear old dist
    if dist_dir.exists() {
        std::fs::remove_dir_all(&dist_dir)?;
    }
    std::fs::create_dir_all(&dist_dir)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => dist_dir.join(path),
            None => continue,
        };
        if file.is_dir() {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    std::fs::create_dir_all(p)?;
                }
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }

    // Write version file
    if let Some(parent) = version_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&version_file, version_tag)?;

    tracing::info!(
        path = ?dist_dir,
        version = %version_tag,
        "Web UI downloaded and extracted"
    );

    Ok(dist_dir)
}

/// Ensures web dist is available. Downloads from GitHub if not present.
/// Returns the dist directory path if available from filesystem.
async fn ensure_web_dist(workspace: &Path) -> Option<PathBuf> {
    // 1. ~/.oxios/web/dist/ (user override — always wins)
    if let Some(ref dist) = user_web_dist_dir() {
        if dist.join("index.html").is_file() {
            // Check if there's a version file — if so, it was auto-downloaded
            // and we should check for updates
            let version_file = user_web_version_file().unwrap();
            let current_version = std::fs::read_to_string(&version_file).ok();

            // Try to fetch latest tag (non-blocking, best-effort)
            match fetch_latest_release_tag().await {
                Ok(latest_tag) => {
                    let current = current_version.as_deref().unwrap_or("");
                    if current != latest_tag {
                        tracing::info!(
                            current = %current,
                            latest = %latest_tag,
                            "New web UI version available, downloading..."
                        );
                        match download_and_extract_web_dist(&latest_tag).await {
                            Ok(p) => {
                                tracing::info!(path = ?p, "Web UI updated");
                                return Some(p);
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "Failed to download update, using existing");
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!(error = %e, "Could not check for web UI updates");
                }
            }

            tracing::info!(path = ?dist, "Serving web UI from ~/.oxios/web/dist/");
            return Some(dist.clone());
        }
    }

    // 2. workspace/web/dist/ (bundled / dev)
    let workspace_dist = workspace.join("web").join("dist");
    if workspace_dist.join("index.html").is_file() {
        tracing::info!(path = ?workspace_dist, "Serving web UI from workspace (web/dist/)");
        return Some(workspace_dist);
    }

    // 3. embedded fallback
    if EmbeddedAssets::get("index.html").is_some() {
        tracing::info!("Serving web UI from embedded assets (binary built with --features web)");
        return None;
    }

    // 4. Auto-download from GitHub Releases
    tracing::info!("No web UI found locally, downloading from GitHub Releases...");
    match fetch_latest_release_tag().await {
        Ok(tag) => match download_and_extract_web_dist(&tag).await {
            Ok(p) => return Some(p),
            Err(e) => tracing::warn!(error = %e, "Failed to auto-download web UI"),
        },
        Err(e) => tracing::warn!(error = %e, "Could not fetch latest release info"),
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
        if let Some(data) = fs_read(d, &format!("assets/{clean}")) {
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
        let workspace = PathBuf::from(&config.kernel.workspace);

        // Resolve web dist path (auto-download if needed)
        let web_dist = ensure_web_dist(&workspace).await;

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
