//! Oxios Agent OS — main binary.
//!
//! Starts the kernel, creates the Ouroboros engine and Agent runtime,
//! registers the web channel, and runs the gateway event loop.

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;

use oxios_gateway::Gateway;
use oxios_kernel::{
    AgentRuntime, BasicSupervisor, EventBus, GardenManager, HostExecBridge, Orchestrator,
    OxiosConfig, StateStore, Supervisor,
};
use oxios_ouroboros::OuroborosEngine;
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

    /// LLM model ID to use (format: provider/model).
    #[arg(long, default_value = "anthropic/claude-sonnet-4-20250514")]
    model: String,
}

/// Subdirectories to create inside the Oxios home directory.
const WORKSPACE_SUBDIRS: &[&str] = &[
    "workspace",
    "workspace/memory",
    "workspace/memory/knowledge",
    "workspace/seeds",
    "workspace/sessions",
    "workspace/skills",
    "gardens",
];

/// Default configuration content written when no config file exists.
const DEFAULT_CONFIG: &str = include_str!("../channels/oxios-web/static/default-config.toml");

/// Ensures the Oxios home directory structure and default config exist.
///
/// Creates `~/.oxios/` and all required subdirectories on first run.
/// Writes a default `config.toml` if none is found.
fn ensure_workspace(oxios_home: &std::path::Path) -> Result<()> {
    if !oxios_home.exists() {
        tracing::info!(path = %oxios_home.display(), "Creating Oxios home directory");
        std::fs::create_dir_all(oxios_home)?;
    }

    for subdir in WORKSPACE_SUBDIRS {
        let dir = oxios_home.join(subdir);
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }
    }

    let config_path = oxios_home.join("config.toml");
    if !config_path.exists() {
        tracing::info!(path = %config_path.display(), "Writing default config");
        std::fs::write(&config_path, DEFAULT_CONFIG)?;
    }

    Ok(())
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

    // Determine the Oxios home directory (parent of config file).
    let oxios_home = config_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            PathBuf::from(format!("{home}/.oxios"))
        });

    // Ensure workspace structure exists (first-run initialization).
    ensure_workspace(&oxios_home)?;

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
        model = %cli.model,
        "Oxios initializing"
    );

    // Initialize kernel components.
    let event_bus = EventBus::new(config.kernel.event_bus_capacity);
    let state_store = Arc::new(StateStore::new(PathBuf::from(&config.kernel.workspace))?);

    // Create the LLM provider from the model ID.
    let (provider, model) = resolve_provider_and_model(&cli.model)?;

    // Create the Ouroboros engine for spec-first orchestration.
    let ouroboros: Arc<dyn oxios_ouroboros::OuroborosProtocol> =
        Arc::new(OuroborosEngine::new(Arc::clone(&provider), model));

    // Create the agent runtime for executing seeds.
    let agent_runtime = AgentRuntime::new(provider, &cli.model);

    // Initialize the supervisor with the agent runtime.
    let supervisor: Arc<dyn Supervisor> = Arc::new(BasicSupervisor::new(
        event_bus.clone(),
        agent_runtime,
    ));

    // Create the orchestrator to wire Ouroboros + Supervisor together.
    let orchestrator = Arc::new(Orchestrator::new(
        ouroboros,
        supervisor.clone(),
        event_bus.clone(),
        state_store.clone(),
    ));

    // Initialize gateway with the orchestrator.
    let gateway = Gateway::new(orchestrator.clone());

    // Create the web channel and extract a handle for the HTTP server.
    let web_channel = oxios_web::WebChannel::new(256);
    let channel_handle = oxios_web::channel::WebChannelHandle::from_channel(&web_channel);

    // Register the web channel with the gateway.
    gateway.register(Box::new(web_channel)).await;

    // Initialize the garden manager.
    let gardens_base = PathBuf::from(&config.container.garden_path);
    let host_exec = Arc::new(HostExecBridge::new(
        gardens_base.clone(),
        config.container.allowed_host_commands.clone(),
    ));
    let state_store_for_gardens = StateStore::new(PathBuf::from(&config.kernel.workspace))?;
    let garden_manager = GardenManager::with_apple_backend(
        host_exec,
        Arc::new(state_store_for_gardens),
        gardens_base,
    );

    // Create the web server with the channel handle, event bus, state store, and garden manager.
    let web_server = WebServer::new(
        &config.gateway.host,
        config.gateway.port,
        channel_handle,
        event_bus.clone(),
        (*state_store).clone(),
        garden_manager,
    );

    tracing::info!(
        "Oxios started on http://{}:{}",
        config.gateway.host,
        config.gateway.port
    );

    // If a prompt was given, run it through the Ouroboros orchestrator.
    if let Some(prompt) = &cli.prompt {
        tracing::info!(prompt = %prompt, "Processing direct prompt");

        match orchestrator
            .handle_message("cli", prompt, None)
            .await
        {
            Ok(result) => {
                tracing::info!(response = %result.response, "Prompt processed");
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to process prompt");
            }
        }
    }

    // Run the web server and gateway concurrently.
    // The gateway's run() loop handles channel message polling.
    tokio::spawn(async move {
        if let Err(e) = gateway.run().await {
            tracing::error!(error = %e, "Gateway error");
        }
    });

    if let Err(e) = web_server.serve().await {
        tracing::error!(error = %e, "Web server error");
    }

    Ok(())
}

/// Resolve a model ID string into a Provider and Model.
fn resolve_provider_and_model(
    model_id: &str,
) -> Result<(Arc<dyn oxi_ai::Provider>, oxi_ai::Model)> {
    let parts: Vec<&str> = model_id.split('/').collect();
    if parts.len() < 2 {
        anyhow::bail!(
            "Invalid model ID '{}'. Expected format: provider/model",
            model_id
        );
    }

    let model = oxi_ai::get_model(parts[0], &parts[1..].join("/"))
        .ok_or_else(|| anyhow::anyhow!("Model '{}' not found in registry", model_id))?;

    let provider = oxi_ai::get_provider(&model.provider)
        .ok_or_else(|| anyhow::anyhow!("Provider '{}' not available", model.provider))?;

    Ok((Arc::from(provider), model.clone()))
}
