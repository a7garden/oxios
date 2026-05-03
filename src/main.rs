//! Oxios Agent OS — main binary.
//!
//! Starts the kernel, creates the Ouroboros engine and Agent runtime,
//! registers the web channel, and runs the gateway event loop.

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;

use oxios_gateway::Gateway;
use oxios_kernel::{AgentRuntime, EventBus, OxiosConfig, StateStore};
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
        model = %cli.model,
        "Oxios initializing"
    );

    // Initialize kernel components.
    let event_bus = EventBus::new(config.kernel.event_bus_capacity);
    let state_store = StateStore::new(PathBuf::from(&config.kernel.workspace))?;

    // Create the LLM provider from the model ID.
    let (provider, model) = resolve_provider_and_model(&cli.model)?;

    // Create the Ouroboros engine for spec-first orchestration.
    let ouroboros = OuroborosEngine::new(Arc::clone(&provider), model);

    // Create the agent runtime for executing seeds.
    let agent_runtime = AgentRuntime::new(provider, &cli.model);

    // Initialize the supervisor with the agent runtime.
    let _supervisor =
        oxios_kernel::supervisor::BasicSupervisor::new(event_bus.clone(), agent_runtime);

    // Initialize gateway.
    let gateway = Gateway::new();

    // Create the web channel and extract a handle for the HTTP server.
    let web_channel = oxios_web::WebChannel::new(256);
    let channel_handle = oxios_web::channel::WebChannelHandle::from_channel(&web_channel);

    // Register the web channel with the gateway.
    gateway.register(Box::new(web_channel)).await;

    // Create the web server with the channel handle, event bus, and state store.
    let web_server = WebServer::new(
        &config.gateway.host,
        config.gateway.port,
        channel_handle,
        event_bus,
        state_store,
    );

    tracing::info!(
        "Oxios started on http://{}:{}",
        config.gateway.host,
        config.gateway.port
    );

    // If a prompt was given, run it through the Ouroboros protocol.
    if let Some(prompt) = &cli.prompt {
        tracing::info!(prompt = %prompt, "Processing direct prompt");

        match process_prompt(&ouroboros, prompt).await {
            Ok(result) => {
                tracing::info!(output = %result.output, "Prompt processed");
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to process prompt");
            }
        }
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

/// Process a direct prompt through the Ouroboros interview → seed pipeline.
///
/// For interactive prompts, we run the interview phase. If the ambiguity is
/// low enough, we also generate a seed. Execution is deferred to the agent
/// runtime (would need the full supervisor integration).
async fn process_prompt(
    ouroboros: &OuroborosEngine,
    prompt: &str,
) -> Result<oxios_ouroboros::ExecutionResult> {
    use oxios_ouroboros::OuroborosProtocol;

    // Phase 1: Interview.
    let interview_result = ouroboros.interview(prompt).await?;

    if !interview_result.ready_for_seed {
        tracing::info!(
            ambiguity = ?interview_result.ambiguity,
            "Ambiguity too high for automatic execution. Questions:"
        );
        for (i, q) in interview_result.questions.iter().enumerate() {
            tracing::info!(question = %i, "{}", q);
        }
        return Ok(oxios_ouroboros::ExecutionResult {
            output: format!(
                "Ambiguity too high ({:.2}). Please clarify:\n{}",
                interview_result.ambiguity.ambiguity(),
                interview_result
                    .questions
                    .iter()
                    .enumerate()
                    .map(|(i, q)| format!("{}. {}", i + 1, q))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            steps_completed: 0,
            success: false,
        });
    }

    // Phase 2: Generate seed.
    let seed = ouroboros.generate_seed(&interview_result).await?;
    tracing::info!(seed_id = %seed.id, goal = %seed.goal, "Seed generated from prompt");

    // Phases 3-5 (execute, evaluate, evolve) would be run by the supervisor
    // via run_with_seed(). For the direct prompt path, we return a result
    // indicating the seed was created.
    Ok(oxios_ouroboros::ExecutionResult {
        output: format!("Seed created: {}", seed.goal),
        steps_completed: 0,
        success: true,
    })
}
