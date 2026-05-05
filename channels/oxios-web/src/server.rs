//! Axum HTTP server setup for the web channel.
//!
//! Starts an HTTP server on a configurable port (default 4200)
//! with CORS, WebSocket support, SSE streaming, and static file serving.
//! Supports graceful shutdown via SIGINT.

use anyhow::Result;
use axum::Router;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use crate::channel::WebChannelHandle;
use crate::routes::build_routes;
use oxios_kernel::event_bus::EventBus;
use oxios_kernel::container_manager::ContainerManager;
use oxios_kernel::host_tools::HostToolValidator;
use oxios_kernel::mcp::McpBridge;
use oxios_kernel::persona_manager::PersonaManager;
use oxios_kernel::program::ProgramManager;
use oxios_kernel::scheduler::AgentScheduler;
use oxios_kernel::access_manager::AccessManager;
use oxios_kernel::skill::SkillStore;
use oxios_kernel::state_store::StateStore;
use oxios_kernel::Supervisor;
use parking_lot::Mutex;

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
    /// Loaded configuration.
    pub config: Arc<oxios_kernel::OxiosConfig>,
    /// Path to the config file (for persistence on PUT /api/config).
    pub config_path: Option<PathBuf>,
    /// MCP bridge for tool calling (uses Mutex for interior mutability on register_server).
    pub mcp_bridge: Arc<Mutex<McpBridge>>,
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
            .field("config", &self.config)
            .field("config_path", &self.config_path)
            .finish()
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
        mcp_bridge: Arc<Mutex<McpBridge>>,
    ) -> Self {
        let addr: SocketAddr = format!("{host}:{port}")
            .parse()
            .expect("Invalid bind address");
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
            config: Arc::new(config),
            config_path,
            mcp_bridge,
        });
        Self { addr, state }
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

        let app = Router::new()
            .merge(build_routes())
            .fallback_service(
                ServeDir::new(&static_dir).append_index_html_on_directories(true),
            )
            .layer(CorsLayer::permissive())
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
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
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
