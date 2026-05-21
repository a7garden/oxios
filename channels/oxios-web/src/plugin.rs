//! Web channel plugin.
//!
//! Factory for creating the web channel and its axum HTTP/WS server.
//! Implements [`ChannelPlugin`](oxios_gateway::plugin::ChannelPlugin) so
//! the main binary can activate the web channel from configuration.

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
/// Build with: cd web && npm run build
#[derive(Embed)]
#[folder = "web/dist/"]
struct Assets;

/// SPA fallback handler — serves index.html for all non-file routes.
/// This enables client-side routing (TanStack Router).
struct SpaFallback;

impl IntoResponse for SpaFallback {
    fn into_response(self) -> Response {
        match Assets::get("index.html") {
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
}

/// Serve a static file from embedded assets.
fn serve_file(path: &str) -> Response {
    let clean_path = path.trim_start_matches('/');

    // Try exact path and assets/ prefix
    let asset = Assets::get(clean_path).or_else(|| Assets::get(&format!("assets/{}", clean_path)));

    match asset {
        Some(content) => {
            let lookup = if clean_path.starts_with("assets/") {
                clean_path
            } else {
                &format!("assets/{}", clean_path)
            };
            let mime = mime_guess::from_path(lookup)
                .first_or_octet_stream()
                .to_string();
            let body = Body::from(content.data.to_vec());
            Response::builder()
                .status(200)
                .header("Content-Type", mime)
                .body(body)
                .unwrap()
        }
        None => Response::builder().status(404).body(Body::empty()).unwrap(),
    }
}

/// Static asset handler — extracts path from URL and serves from embedded assets.
async fn static_handler(Path(path): axum::extract::Path<String>) -> Response {
    serve_file(&path)
}

/// SPA fallback — serves index.html for client-side routing.
async fn spa_handler() -> impl IntoResponse {
    SpaFallback
}

/// Web channel plugin — creates an axum HTTP/WS server.
pub struct WebPlugin;

impl WebPlugin {
    /// Create a new web plugin instance.
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

        // Create web channel
        let web_channel = WebChannel::new(256);
        let channel_handle = WebChannelHandle::from_channel(&web_channel);

        // Create KnowledgeBase for direct markdown CRUD (no kernel dependency)
        let knowledge_root = PathBuf::from(&config.kernel.workspace).join("knowledge");
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
        });

        // Build API routes (with AppState)
        let api_routes = routes::build_routes(state.clone());

        // CORS layer — origins from config
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

        // OpenAPI / Swagger UI (unit state for type compatibility)
        let openapi = api_docs::build_openapi();
        let swagger: Router<()> = utoipa_swagger_ui::SwaggerUi::new("/api-docs")
            .url("/openapi.json", openapi)
            .into();

        // SPA routes (catch-all for client-side routing)
        // Static assets (/assets/*) served from embedded files, not SPA fallback
        let spa_routes: Router<Arc<AppState>> = Router::new()
            .route("/assets/{*path}", get(static_handler))
            .route("/favicon.svg", get(static_handler))
            .route("/icons.svg", get(static_handler))
            .route("/{*path}", get(spa_handler))
            .route("/", get(spa_handler));

        // Build main app with state
        // API routes + SPA routes + CORS + Swagger (nested at /api-docs)
        // with_state() erases state type to Router<()>
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

        // Spawn server with graceful shutdown
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
