//! Oxios Agent OS — main binary.
//!
//! Starts the kernel, registers channels, and runs the gateway event loop.

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use oxios_gateway::Gateway;
use oxios_kernel::{EventBus, OxiosConfig};
use oxios_web::WebServer;

/// Oxios Agent OS
#[derive(Debug, Parser)]
#[command(name = "oxios", version, about = "Oxios Agent OS")]
struct Cli {
    /// Optional prompt to process immediately.
    prompt: Option<String>,

    /// Path to config file.
    #[arg(long, default_value = "~/.oxios/config.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    // Expand tilde in config path.
    let config_path = if cli.config.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            cli.config.replacen("~/", &format!("{home}/"), 1)
        } else {
            cli.config.clone()
        }
    } else {
        cli.config.clone()
    };
    let config_path = PathBuf::from(&config_path);

    // Load config, falling back to defaults.
    let config = if config_path.exists() {
        tracing::info!(path = %config_path.display(), "Loading config");
        oxios_kernel::config::load_config(&config_path)?
    } else {
        tracing::info!("No config file found, using defaults");
        OxiosConfig {
            kernel: Default::default(),
            gateway: Default::default(),
            container: Default::default(),
        }
    };

    tracing::info!(
        host = %config.gateway.host,
        port = %config.gateway.port,
        workspace = %config.kernel.workspace,
        "Oxios initializing"
    );

    // Initialize kernel components.
    let _event_bus = EventBus::new(config.kernel.event_bus_capacity);

    // Initialize gateway.
    let gateway = Gateway::new();

    // Register the web channel.
    let web_server = WebServer::new(&config.gateway.host, config.gateway.port);

    tracing::info!("Oxios started on http://{}:{}", config.gateway.host, config.gateway.port);

    // If a prompt was given, log it.
    if let Some(prompt) = &cli.prompt {
        tracing::info!(prompt = %prompt, "Processing direct prompt");
        // TODO: dispatch prompt through ouroboros protocol
    }

    // Run the web server concurrently with the gateway.
    tokio::select! {
        result = web_server.serve() => {
            if let Err(e) = result {
                tracing::error!(error = %e, "Web server error");
            }
        }
        result = gateway.run() => {
            if let Err(e) = result {
                tracing::error!(error = %e, "Gateway error");
            }
        }
    }

    Ok(())
}
