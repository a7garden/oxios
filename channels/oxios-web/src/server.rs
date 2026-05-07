//! Axum HTTP server setup for the web channel.
//!
//! Starts an HTTP server on a configurable port (default 4200)
//! with CORS, WebSocket support, SSE streaming, and static file serving.
//! Supports graceful shutdown via SIGINT.

use anyhow::Result;
use axum::Router;
use parking_lot::RwLock;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tower_http::services::ServeDir;

use crate::api_docs;
use crate::channel::WebChannelHandle;
use crate::error::AppError;
use crate::middleware::RateLimiter;
use crate::routes::build_routes;
use oxios_kernel::event_bus::EventBus;
use oxios_kernel::container_manager::ContainerManager;
use oxios_kernel::host_tools::HostToolValidator;
use oxios_kernel::mcp::McpBridge;
use oxios_kernel::memory::MemoryManager;
use oxios_kernel::persona_manager::PersonaManager;
use oxios_kernel::program::ProgramManager;
use oxios_kernel::scheduler::AgentScheduler;
use oxios_kernel::access_manager::AccessManager;
use oxios_kernel::skill::SkillStore;
use oxios_kernel::state_store::StateStore;
use oxios_kernel::Supervisor;

/// Shared application state for the web server.
///
/// This is the central state accessible to all route handlers.
#[derive(Clone)]
pub struct AppState {
    /// Base URL for API responses.
    pub base_url: String,
    /// Handle to the web channel for message passing.
    pub channel: WebChannelHandle,
    /// Event bus for subscribing to kernel events.
    pub event_bus: Arc<EventBus>,
    /// State store for workspace/memory/seeds access.
    pub state_store: Arc<StateStore>,
    /// Container manager for container lifecycle.
    pub container_manager: Arc<ContainerManager>,
    /// Skill store for skill management (deprecated, use program_manager).
    pub skill_store: Arc<SkillStore>,
    /// Program manager for OS-level programs.
    pub program_manager: Arc<ProgramManager>,
    /// Host tool validator.
    pub host_tool_validator: Arc<HostToolValidator>,
    /// Agent supervisor for lifecycle management.
    pub supervisor: Arc<dyn Supervisor>,
    /// Agent scheduler for task queue management.
    pub scheduler: Arc<AgentScheduler>,
    /// Access manager for agent permissions and security.
    pub access_manager: Arc<parking_lot::Mutex<AccessManager>>,
    /// Persona manager for multi-persona support.
    pub persona_manager: Arc<PersonaManager>,
    /// Loaded configuration (hot-reloadable via RwLock).
    pub config: Arc<RwLock<oxios_kernel::OxiosConfig>>,
    /// Path to the config file (for persistence on PUT /api/config).
    pub config_path: PathBuf,
    /// Server start time (for uptime calculation).
    pub start_time: Instant,
    /// MCP bridge for tool calling (uses tokio::sync::Mutex for async-safe interior mutability).
    pub mcp_bridge: Arc<McpBridge>,
    /// Authentication manager for bearer token validation.
    pub auth_manager: Arc<parking_lot::Mutex<oxios_kernel::auth::AuthManager>>,
    /// Memory manager for cross-session agent memory.
    pub memory_manager: Arc<MemoryManager>,
    /// Rate limiter for API endpoints.
    #[allow(dead_code)]
    pub rate_limiter: RateLimiter,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("base_url", &self.base_url)
            .field("channel", &"...")
            .field("event_bus", &"...")
            .field("state_store", &self.state_store)
            .field("container_manager", &self.container_manager)
            .field("skill_store", &self.skill_store)
            .field("supervisor", &"...")
            .field("scheduler", &"...")
            .field("access_manager", &"...")
            .field("persona_manager", &"...")
            .field("config", &"...")
            .field("config_path", &self.config_path)
            .finish()
    }
}

impl AppState {
    /// Reload config from disk and update in-memory state.
    pub async fn reload_config(&self) -> Result<(), AppError> {
        let config = oxios_kernel::config::load_config(&self.config_path)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        *self.config.write() = config;

        tracing::info!("Config hot-reloaded from {}", self.config_path.display());
        Ok(())
    }
}

/// The web HTTP server.
pub struct WebServer {
    /// Address to bind the server to.
    addr: SocketAddr,
    /// Shared application state.
    state: Arc<AppState>,
}

impl WebServer {
    /// Creates a new web server bound to the given address.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        host: &str,
        port: u16,
        channel: WebChannelHandle,
        event_bus: EventBus,
        state_store: StateStore,
        container_manager: Arc<ContainerManager>,
        skill_store: SkillStore,
        program_manager: Arc<ProgramManager>,
        host_tool_validator: HostToolValidator,
        supervisor: Arc<dyn Supervisor>,
        scheduler: Arc<AgentScheduler>,
        access_manager: Arc<parking_lot::Mutex<AccessManager>>,
        persona_manager: PersonaManager,
        config: oxios_kernel::OxiosConfig,
        config_path: Option<PathBuf>,
        mcp_bridge: Arc<McpBridge>,
        auth_manager: Arc<parking_lot::Mutex<oxios_kernel::auth::AuthManager>>,
        memory_manager: Arc<MemoryManager>,
    ) -> Result<Self, anyhow::Error> {
        let addr: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid bind address '{host}:{port}': {e}"))?;
        let rate_limit = config.security.rate_limit_per_minute;
        let state = Arc::new(AppState {
            base_url: format!("http://{host}:{port}"),
            channel,
            event_bus: Arc::new(event_bus),
            state_store: Arc::new(state_store),
            container_manager,
            skill_store: Arc::new(skill_store),
            program_manager,
            host_tool_validator: Arc::new(host_tool_validator),
            supervisor,
            scheduler,
            access_manager,
            persona_manager: Arc::new(persona_manager),
            config: Arc::new(RwLock::new(config)),
            config_path: config_path.clone().unwrap_or_default(),
            start_time: Instant::now(),
            mcp_bridge,
            auth_manager,
            memory_manager,
            rate_limiter: RateLimiter::new(rate_limit),
        });
        Ok(Self { addr, state })
    }

    /// Returns the shared application state.
    pub fn state(&self) -> Arc<AppState> {
        self.state.clone()
    }

    /// Starts the HTTP server with graceful shutdown.
    ///
    /// This method blocks until the server is shut down via SIGINT.
    pub async fn serve(&self) -> Result<()> {
        // Locate the static directory (relative to the crate root)
        let static_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("static");

        let cors = tower_http::cors::CorsLayer::new()
            .allow_origin(
                ["http://localhost:4200".parse::<axum::http::HeaderValue>()
                    .expect("hardcoded valid origin")]
            )
            .allow_methods(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any);

        // Build OpenAPI spec and Swagger UI
        let openapi = api_docs::build_openapi();
        let swagger = utoipa_swagger_ui::SwaggerUi::new("/swagger-ui")
            .url("/api-docs/openapi.json", openapi);

        let app = Router::new()
            .merge(swagger)
            .merge(build_routes(self.state.clone()))
            .fallback_service(
                ServeDir::new(&static_dir).append_index_html_on_directories(true),
            )
            .layer(cors)
            .with_state(self.state.clone());

        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        tracing::info!(addr = %self.addr, "Web server listening");

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await?;

        tracing::info!("Web server shut down");
        Ok(())
    }
}

/// Waits for Ctrl+C or SIGTERM to trigger graceful shutdown.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .unwrap_or_else(|e| tracing::error!(error = %e, "Ctrl+C handler failed"));
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .unwrap_or_else(|e| {
                tracing::error!(error = %e, "SIGTERM handler failed");
                // Return a signal stream that never fires
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::user_defined1())
                    .expect("fallback signal")
            })
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Received Ctrl+C"),
        _ = terminate => tracing::info!("Received SIGTERM"),
    }
}
