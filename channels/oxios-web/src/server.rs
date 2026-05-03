//! Axum HTTP server setup for the web channel.
//!
//! Starts an HTTP server on a configurable port (default 4200)
//! and serves the Oxios web API.

use anyhow::Result;
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use crate::routes::build_routes;

/// Shared application state for the web server.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Base URL for API responses.
    pub base_url: String,
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
    pub fn new(host: &str, port: u16) -> Self {
        let addr: SocketAddr = format!("{host}:{port}")
            .parse()
            .expect("Invalid bind address");
        let state = Arc::new(AppState {
            base_url: format!("http://{host}:{port}"),
        });
        Self { addr, state }
    }

    /// Returns the shared application state.
    pub fn state(&self) -> Arc<AppState> {
        self.state.clone()
    }

    /// Starts the HTTP server. This is a blocking call.
    pub async fn serve(&self) -> Result<()> {
        let app = Router::new()
            .merge(build_routes())
            .layer(CorsLayer::permissive())
            .with_state(self.state.clone());

        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        tracing::info!(addr = %self.addr, "Web server listening");
        axum::serve(listener, app).await?;
        Ok(())
    }
}
