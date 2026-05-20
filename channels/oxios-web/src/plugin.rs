//! Web channel plugin.
//!
//! Factory for creating the web channel and its axum HTTP/WS server.
//! Implements [`ChannelPlugin`](oxios_gateway::plugin::ChannelPlugin) so
//! the main binary can activate the web channel from configuration.

use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

use oxios_gateway::plugin::{ChannelBundle, ChannelContext, ChannelPlugin};
use oxios_markdown::KnowledgeBase;

use crate::api_docs;
use crate::channel::{WebChannel, WebChannelHandle};
use crate::middleware::RateLimiter;
use crate::routes;
use crate::server::AppState;

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
                Arc::new(KnowledgeBase::new(fallback_dir).expect("Failed to create fallback KnowledgeBase"))
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

        // Build API routes
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

        // Static file serving
        // 1) web/dist/ — TypeScript SPA frontend (Vite build output)
        // 2) static/   — Oxios runtime assets (knowledge, default-programs, etc.)
        let web_dist_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("web/dist");
        let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static");
        let spa_index = web_dist_dir.join("index.html");

        // OpenAPI / Swagger UI
        let openapi = api_docs::build_openapi();
        let swagger: axum::Router<()> = utoipa_swagger_ui::SwaggerUi::new("/swagger-ui")
            .url("/api-docs/openapi.json", openapi)
            .into();

        // Compose final app
        //
        // Serve web/dist/ first (SPA frontend), fall back to static/ (runtime assets).
        // For SPA routing, non-file paths that don't match any route serve index.html.
        let app = axum::Router::new()
            .merge(api_routes)
            .fallback_service(
                tower_http::services::ServeDir::new(&web_dist_dir)
                    .fallback(
                        tower_http::services::ServeDir::new(&static_dir)
                            .append_index_html_on_directories(true)
                            .fallback(
                                tower_http::services::ServeFile::new(&spa_index),
                            ),
                    ),
            )
            .layer(cors)
            .with_state(state);

        let app = axum::Router::new().merge(swagger).merge(app);

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