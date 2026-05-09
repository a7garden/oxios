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
use oxios_kernel::KernelHandle;

/// Shared application state for the web server.
///
/// This is the central state accessible to all route handlers.
/// Built from KernelHandle for clean dependency injection.
#[derive(Clone)]
pub struct AppState {
    /// Base URL for API responses.
    pub base_url: String,
    /// Handle to the kernel subsystem (provides access to all kernel components).
    pub kernel: Arc<KernelHandle>,
    /// Handle to the web channel for message passing.
    pub channel: WebChannelHandle,
    /// Event bus for subscribing to kernel events.
    pub event_bus: Arc<oxios_kernel::EventBus>,
    /// State store for workspace/memory/seeds access.
    pub state_store: Arc<oxios_kernel::StateStore>,
    /// Container manager for container lifecycle.
    pub container_manager: Arc<oxios_kernel::ContainerManager>,
    /// Resource monitor for system metrics.
    pub resource_monitor: Arc<oxios_kernel::ResourceMonitor>,
    /// Audit trail for tamper-evident logging.
    pub audit_trail: Arc<oxios_kernel::AuditTrail>,
    /// Budget manager for agent-level token/call budgets.
    pub budget_manager: Arc<oxios_kernel::BudgetManager>,
    /// Skill store for skill management.
    pub skill_store: Arc<oxios_kernel::SkillStore>,
    /// Program manager for OS-level programs.
    pub program_manager: Arc<oxios_kernel::ProgramManager>,
    /// Host tool validator.
    pub host_tool_validator: Arc<oxios_kernel::HostToolValidator>,
    /// Agent supervisor for lifecycle management.
    pub supervisor: Arc<dyn oxios_kernel::Supervisor>,
    /// Agent scheduler for task queue management.
    pub scheduler: Arc<oxios_kernel::AgentScheduler>,
    /// Access manager for agent permissions and security.
    pub access_manager: Arc<parking_lot::Mutex<oxios_kernel::AccessManager>>,
    /// Persona manager for multi-persona support.
    pub persona_manager: Arc<oxios_kernel::PersonaManager>,
    /// Loaded configuration (hot-reloadable via RwLock).
    pub config: Arc<RwLock<oxios_kernel::OxiosConfig>>,
    /// Path to the config file (for persistence on PUT /api/config).
    pub config_path: PathBuf,
    /// Server start time (for uptime calculation).
    pub start_time: Instant,
    /// MCP bridge for tool calling.
    pub mcp_bridge: Arc<oxios_kernel::McpBridge>,
    /// Authentication manager for bearer token validation.
    pub auth_manager: Arc<parking_lot::Mutex<oxios_kernel::auth::AuthManager>>,
    /// Memory manager for cross-session agent memory.
    pub memory_manager: Arc<oxios_kernel::MemoryManager>,
    /// Rate limiter for API endpoints.
    #[allow(dead_code)]
    pub rate_limiter: RateLimiter,
    /// Cron scheduler for time-based job execution.
    pub cron_scheduler: Arc<oxios_kernel::CronScheduler>,
    /// Git version control layer.
    pub git_layer: Arc<oxios_kernel::GitLayer>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("base_url", &self.base_url)
            .field("kernel", &"...")
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
    ///
    /// # Arguments
    /// * `host` - Host address to bind to
    /// * `port` - Port to listen on
    /// * `channel` - Web channel handle for message passing
    /// * `kernel` - Arc<KernelHandle> containing all kernel subsystems
    /// * `config` - Arc<RwLock<OxiosConfig>> for hot-reloadable config
    /// * `config_path` - Optional path to config file for persistence
    pub fn new(
        host: &str,
        port: u16,
        channel: WebChannelHandle,
        kernel: Arc<KernelHandle>,
        config: Arc<RwLock<oxios_kernel::OxiosConfig>>,
        config_path: Option<PathBuf>,
    ) -> Result<Self, anyhow::Error> {
        let addr: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid bind address '{host}:{port}': {e}"))?;

        let rate_limit = kernel.get_config().security.rate_limit_per_minute;

        let state = Arc::new(AppState {
            base_url: format!("http://{host}:{port}"),
            kernel: kernel.clone(),
            channel,
            event_bus: Arc::new(kernel.event_bus().clone()),
            state_store: kernel.state_store().clone(),
            container_manager: kernel.container_manager().clone(),
            resource_monitor: kernel.resource_monitor().clone(),
            audit_trail: kernel.audit_trail().clone(),
            budget_manager: kernel.budget_manager().clone(),
            skill_store: kernel.skill_store().clone(),
            program_manager: kernel.program_manager().clone(),
            host_tool_validator: kernel.host_tool_validator().clone(),
            supervisor: kernel.supervisor().clone(),
            scheduler: kernel.scheduler().clone(),
            access_manager: kernel.access_manager().clone(),
            persona_manager: kernel.persona_manager().clone(),
            config,
            config_path: config_path.clone().unwrap_or_default(),
            start_time: kernel.start_time(),
            mcp_bridge: kernel.mcp_bridge().clone(),
            auth_manager: kernel.auth_manager().clone(),
            memory_manager: kernel.memory_manager().clone(),
            rate_limiter: RateLimiter::new(rate_limit),
            cron_scheduler: kernel.cron_scheduler().clone(),
            git_layer: kernel.git_layer().clone(),
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
        let swagger: Router<()> = utoipa_swagger_ui::SwaggerUi::new("/swagger-ui")
            .url("/api-docs/openapi.json", openapi)
            .into();

        let app = Router::new()
            .merge(build_routes(self.state.clone()))
            .fallback_service(
                ServeDir::new(&static_dir).append_index_html_on_directories(true),
            )
            .layer(cors)
            .with_state(self.state.clone());

        let app = Router::new().merge(swagger).merge(app);

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