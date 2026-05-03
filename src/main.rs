//! Oxios Agent OS — main binary.
//!
//! Starts the kernel, creates the Ouroboros engine and Agent runtime,
//! registers the web channel, and runs the gateway event loop.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use oxios_gateway::Gateway;
use oxios_kernel::{
    AccessManager, AgentRuntime, BasicSupervisor, EventBus, GardenManager, HostExecBridge,
    HostToolValidator, Orchestrator, OxiosConfig, PersonaManager, ProgramManager, SkillStore,
    StateStore, Supervisor, AgentScheduler, InstallSource,
};
use oxios_ouroboros::OuroborosEngine;
use oxios_web::WebServer;

// ─── CLI ───────────────────────────────────────────────────────────────────

/// Oxios Agent OS
#[derive(Debug, Parser)]
#[command(name = "oxios", version, about = "Oxios Agent OS — Agent Operating System")]
struct Cli {
    /// Enable verbose logging.
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Path to config file.
    #[arg(short, long, default_value = "~/.oxios/config.toml", global = true)]
    config: String,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run a single prompt through the Ouroboros interview → seed → execute flow.
    #[command(arg_required_else_help = true)]
    Run {
        /// The prompt to execute.
        prompt: String,
    },

    /// Manage container gardens.
    Garden {
        #[command(subcommand)]
        action: GardenAction,
    },

    /// Show system status.
    Status,

    /// Show or modify configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Manage installable programs.
    Pkg {
        #[command(subcommand)]
        action: PkgAction,
    },
}

#[derive(Debug, Subcommand)]
enum GardenAction {
    /// Create a new garden workspace.
    New {
        /// Name of the garden.
        name: String,
    },
    /// Start a garden container.
    Up {
        /// Name of the garden.
        name: String,
    },
    /// Stop a garden container.
    Down {
        /// Name of the garden.
        name: String,
    },
    /// Remove a garden entirely.
    Remove {
        /// Name of the garden.
        name: String,
    },
    /// List all gardens.
    List,
    /// Execute a command inside a garden.
    Exec {
        /// Name of the garden.
        name: String,
        /// Command to execute.
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigAction {
    /// Show the current configuration.
    Show,
    /// Set a configuration value.
    Set {
        /// Configuration key (e.g., gateway.port, kernel.workspace).
        key: String,
        /// Value to set.
        value: String,
    },
    /// Get a configuration value.
    Get {
        /// Configuration key.
        key: String,
    },
}

// ─── Workspace helpers ─────────────────────────────────────────────────────

/// Subdirectories to create inside the Oxios home directory.
const WORKSPACE_SUBDIRS: &[&str] = &[
    "workspace",
    "workspace/memory",
    "workspace/memory/knowledge",
    "workspace/seeds",
    "workspace/sessions",
    "workspace/skills",
    "workspace/programs",
    "gardens",
];

/// Default configuration content written when no config file exists.
const DEFAULT_CONFIG: &str = include_str!("../channels/oxios-web/static/default-config.toml");

/// Ensures the Oxios home directory structure and default config exist.
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

/// Resolve a tilde path to an absolute path.
fn expand_path(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(path.replacen("~/", &format!("{home}/"), 1))
        } else {
            PathBuf::from(path)
        }
    } else {
        PathBuf::from(path)
    }
}

/// Get the Oxios home directory from a config path.
fn oxios_home_from_config(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            PathBuf::from(format!("{home}/.oxios"))
        })
}

// ─── Kernel initialization ──────────────────────────────────────────────────

/// Initialize the Oxios kernel and all its components.
async fn init_kernel(
    config_path: &Path,
    model_id: &str,
) -> Result<(
    Arc<Orchestrator>,
    Gateway,
    EventBus,
    Arc<StateStore>,
    GardenManager,
    OxiosConfig,
    SkillStore,
    Arc<dyn Supervisor>,
    Arc<AgentScheduler>,
    Arc<parking_lot::Mutex<AccessManager>>,
    ProgramManager,
    HostToolValidator,
    PersonaManager,
)> {
    let config = if config_path.exists() {
        tracing::info!(path = %config_path.display(), "Loading config");
        oxios_kernel::config::load_config(config_path)?
    } else {
        tracing::info!("No config file found, using defaults");
        OxiosConfig {
            kernel: Default::default(),
            gateway: Default::default(),
            container: Default::default(),
            scheduler: Default::default(),
            context: Default::default(),
            security: Default::default(),
            persona: Default::default(),
        }
    };

    let event_bus = EventBus::new(config.kernel.event_bus_capacity);
    let state_store = Arc::new(StateStore::new(PathBuf::from(&config.kernel.workspace))?);

    // Create the LLM provider from the model ID.
    let (provider, model) = resolve_provider_and_model(model_id)?;

    // Create the Ouroboros engine for spec-first orchestration.
    let ouroboros: Arc<dyn oxios_ouroboros::OuroborosProtocol> =
        Arc::new(OuroborosEngine::new(Arc::clone(&provider), model));

    // Create the agent runtime for executing seeds.
    let agent_runtime = AgentRuntime::new(provider, model_id);

    // Initialize the supervisor with the agent runtime.
    let supervisor: Arc<dyn Supervisor> = Arc::new(BasicSupervisor::new(
        event_bus.clone(),
        agent_runtime,
    ));

    // Create the orchestrator to wire Ouroboros + Supervisor together.
    // Initialize the access manager and scheduler first.
    let access_manager = Arc::new(Mutex::new(AccessManager::new()));
    let scheduler = Arc::new(AgentScheduler::new(
        config.scheduler.max_concurrent,
        config.scheduler.rate_limit_per_minute,
        config.scheduler.zombie_timeout_secs,
    ));
    let orchestrator = Arc::new(Orchestrator::new(
        ouroboros,
        supervisor.clone(),
        event_bus.clone(),
        state_store.clone(),
        scheduler.clone(),
        access_manager.clone(),
        persona_manager.clone(),
    ));

    // Wire persona into OuroborosEngine for voice customization.
    if let Some(engine) = ouroboros.downcast_ref::<OuroborosEngine>() {
        if let Some(persona) = persona_manager.get_enabled().first() {
            engine.set_persona_prompt(Some(persona.system_prompt.clone()));
            tracing::info!(persona = %persona.name, "Active persona set on OuroborosEngine");
        }
    }

    // Initialize gateway with the orchestrator.
    let gateway = Gateway::new(orchestrator.clone());

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

    // Initialize the skill store.
    let skills_dir = PathBuf::from(&config.kernel.workspace).join("skills");
    let skill_store = SkillStore::new(skills_dir)?;

    // Initialize the program manager.
    let programs_dir = PathBuf::from(&config.kernel.workspace).join("programs");
    let program_manager = ProgramManager::new(programs_dir);
    program_manager.init().await?;

    // Initialize the host tool validator.
    let host_tool_validator = HostToolValidator::new(
        config.container.required_host_tools.clone(),
        config.container.optional_host_tools.clone(),
    );

    // Initialize the persona manager with default personas.
    let persona_manager = PersonaManager::new();
    Ok((orchestrator, gateway, event_bus.clone(), state_store, garden_manager, config, skill_store, supervisor, scheduler, access_manager, program_manager, host_tool_validator, persona_manager))
}

// ─── Resolve provider and model ────────────────────────────────────────────

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

    // Check if API key is available for the provider.
    let api_key_var = format!("{}_API_KEY", model.provider.to_uppercase());
    if std::env::var(&api_key_var).is_err() && std::env::var("API_KEY").is_err() {
        tracing::warn!(
            provider = %model.provider,
            key_var = %api_key_var,
            "No API key found for provider. Set {} or API_KEY environment variable.",
            api_key_var
        );
    }

    Ok((Arc::from(provider), model.clone()))
}

// ─── Commands ───────────────────────────────────────────────────────────────

/// Run a single prompt through the Ouroboros orchestrator.
async fn cmd_run(prompt: &str, config_path: &Path, model_id: &str) -> Result<()> {
    let (orchestrator, _, _, _, _, _, _, _, _, _, _, _, persona_manager) =
        init_kernel(config_path, model_id).await?;

    tracing::info!(prompt = %prompt, "Processing prompt");
    // Keep persona_manager alive for the duration of this function.
    let _persona = persona_manager;

    let result = orchestrator.handle_message("cli", prompt, None).await?;

    println!("{}", result.response);

    if let Some(seed_id) = result.seed_id {
        println!("\nSeed: {}", seed_id);
    }

    if !result.evaluation_passed {
        println!("\n⚠️  Evaluation did not fully pass.");
        if let Some(output) = result.output {
            println!("Notes: {}", output);
        }
    }

    Ok(())
}

/// Handle garden subcommands.
async fn cmd_garden(action: GardenAction, config_path: &Path) -> Result<()> {
    let model_id = "anthropic/claude-sonnet-4-20250514"; // Dummy for garden cmds
    let (_, _, _, _, garden_manager, _, _, _, _, _, _, _, _) = init_kernel(config_path, model_id).await?;

    match action {
        GardenAction::New { name } => {
            garden_manager.new_garden(&name).await?;
            println!("Garden '{}' created.", name);
        }
        GardenAction::Up { name } => {
            if !garden_manager.is_backend_available() {
                println!("⚠️  Container runtime not available. Garden metadata recorded.");
                println!("   To start the container, install 'container' CLI from Xcode.");
            } else {
                garden_manager.start_garden(&name).await?;
                println!("Garden '{}' started.", name);
            }
        }
        GardenAction::Down { name } => {
            if !garden_manager.is_backend_available() {
                println!("⚠️  Container runtime not available.");
            } else {
                garden_manager.stop_garden(&name).await?;
                println!("Garden '{}' stopped.", name);
            }
        }
        GardenAction::Remove { name } => {
            garden_manager.remove_garden(&name).await?;
            println!("Garden '{}' removed.", name);
        }
        GardenAction::List => {
            let gardens: Vec<_> = garden_manager.list_gardens().await?;
            if gardens.is_empty() {
                println!("No gardens found.");
            } else {
                println!("{:30} {:15} IMAGE", "NAME", "STATUS");
                println!("{}", "-".repeat(70));
                for g in &gardens {
                    let status = if g.running { "running" } else { "stopped" };
                    println!("{:30} {:15} {}", g.name, status, g.image_tag);
                }
            }
        }
        GardenAction::Exec { name, command } => {
            if !garden_manager.is_backend_available() {
                bail!("Container runtime not available. Cannot exec in garden.");
            }
            let result = garden_manager.exec_in_garden(&name, &command, None).await?;
            if !result.stdout.is_empty() {
                print!("{}", result.stdout);
            }
            if !result.stderr.is_empty() {
                eprint!("{}", result.stderr);
            }
            if result.exit_code != 0 {
                std::process::exit(result.exit_code);
            }
        }
    }

    Ok(())
}

// ─── Pkg subcommands ─────────────────────────────────────────────────────────────


#[derive(Debug, Subcommand)]
enum PkgAction {
    /// Install a program from a URL or local path.
    Install {
        /// Source: a git URL, tarball URL, or local directory path.
        #[arg(value_name = "SOURCE")]
        source: String,
        /// Install from a specific git branch.
        #[arg(short, long)]
        branch: Option<String>,
    },
    /// Uninstall a program by name.
    Uninstall {
        /// Name of the program to uninstall.
        #[arg(value_name = "NAME")]
        name: String,
    },
    /// List all installed programs.
    List,
    /// Search programs (stub — lists installed programs).
    Search,
}

/// Handle pkg subcommands.
async fn cmd_pkg(action: PkgAction, config_path: &Path) -> Result<()> {
    let model_id = "anthropic/claude-sonnet-4-20250514"; // Dummy for pkg cmds
    let (_, _, _, _, _, _, _, _, _, _, program_manager, _, _) =
        init_kernel(config_path, model_id).await?;

    match action {
        PkgAction::Install { source, branch } => {
            let install_source = if source.ends_with(".git") || source.starts_with("git@") {
                InstallSource::Git { url: source, branch }
            } else if source.starts_with("http://") || source.starts_with("https://") {
                InstallSource::Tarball { url: source }
            } else {
                InstallSource::Local(PathBuf::from(&source))
            };

            let program = program_manager.install_from(install_source).await?;
            println!("Installed '{}' v{}", program.meta.name, program.meta.version);
        }
        PkgAction::Uninstall { name } => {
            program_manager.uninstall(&name).await?;
            println!("Uninstalled '{}'", name);
        }
        PkgAction::List => {
            let programs = program_manager.list_programs().await;
            if programs.is_empty() {
                println!("No programs installed.");
            } else {
                println!("{:30} {:10} {:40}", "NAME", "VERSION", "DESCRIPTION");
                println!("{}", "-".repeat(82));
                for p in &programs {
                    println!(
                        "{:30} {:10} {:40}",
                        p.meta.name,
                        p.meta.version,
                        p.meta.description
                    );
                }
            }
        }
        PkgAction::Search => {
            let programs = program_manager.list_programs().await;
            if programs.is_empty() {
                println!("No programs installed.");
            } else {
                for p in &programs {
                    println!("{} ({})", p.meta.name, p.meta.version);
                    println!("  {}", p.meta.description);
                    if !p.meta.tools.is_empty() {
                        let tools: Vec<_> = p.meta.tools.iter().map(|t| t.name.clone()).collect();
                        println!("  Tools: {}", tools.join(", "));
                    }
                    println!();
                }
            }
        }
    }

    Ok(())
}

/// Handle config subcommands.
async fn cmd_config(action: ConfigAction, config_path: &Path) -> Result<()> {
    let config = if config_path.exists() {
        oxios_kernel::config::load_config(config_path)?
    } else {
        OxiosConfig {
            kernel: Default::default(),
            gateway: Default::default(),
            container: Default::default(),
            scheduler: Default::default(),
            context: Default::default(),
            security: Default::default(),
            persona: Default::default(),
        }
    };

    match action {
        ConfigAction::Show => {
            let toml_str = toml::to_string_pretty(&config)
                .context("failed to serialize config")?;
            println!("{}", toml_str);
        }
        ConfigAction::Get { key } => {
            let value = get_config_value(&config, &key)
                .ok_or_else(|| anyhow::anyhow!("Unknown config key: {}", key))?;
            println!("{}", value);
        }
        ConfigAction::Set { key: _, value: _ } => {
            bail!("Config set not yet implemented. Edit ~/.oxios/config.toml directly.");
        }
    }

    Ok(())
}

/// Get a dotted config value from OxiosConfig.
fn get_config_value(config: &OxiosConfig, key: &str) -> Option<String> {
    let parts: Vec<&str> = key.split('.').collect();
    match parts.as_slice() {
        ["kernel", "workspace"] => Some(config.kernel.workspace.clone()),
        ["kernel", "event_bus_capacity"] => Some(config.kernel.event_bus_capacity.to_string()),
        ["kernel", "max_agents"] => Some(config.kernel.max_agents.to_string()),
        ["gateway", "host"] => Some(config.gateway.host.clone()),
        ["gateway", "port"] => Some(config.gateway.port.to_string()),
        ["container", "garden_path"] => Some(config.container.garden_path.clone()),
        ["container", "image_tag"] => Some(config.container.image_tag.clone()),
        ["container", "memory_limit"] => Some(config.container.memory_limit.clone()),
        ["container", "cpu_limit"] => Some(config.container.cpu_limit.to_string()),
        _ => None,
    }
}

/// Show system status.
async fn cmd_status(config_path: &Path) -> Result<()> {
    let model_id = "anthropic/claude-sonnet-4-20250514";
    let (_, _, _, _, garden_manager, config, _, _, _, _, _, _, _) =
        init_kernel(config_path, model_id).await?;

    println!("Oxios Agent OS");
    println!("{}", "=".repeat(40));
    println!("  Version: {}", env!("CARGO_PKG_VERSION"));
    println!("  Config: {}", config_path.display());
    println!("  Workspace: {}", config.kernel.workspace);
    println!("  Garden path: {}", config.container.garden_path);
    println!();

    // Container backend status.
    let backend_available = garden_manager.is_backend_available();
    println!("Container Runtime:");
    println!("  Backend: {}", garden_manager.backend_name());
    println!("  Available: {} ({})",
        if backend_available { "yes" } else { "no" },
        if backend_available { "ready" } else { "install container CLI from Xcode" }
    );
    println!();

    // Gardens status.
    let gardens: Vec<_> = garden_manager.list_gardens().await?;
    println!("Gardens: {} known", gardens.len());
    if !gardens.is_empty() {
        let running = gardens.iter().filter(|g| g.running).count();
        println!("  {} running, {} stopped", running, gardens.len() - running);
    }

    // API key check.
    let api_key_vars = ["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "API_KEY"];
    let has_key = api_key_vars.iter().any(|v| std::env::var(v).is_ok());
    println!();
    println!("API Keys:");
    println!("  Configured: {} ({})",
        if has_key { "yes" } else { "no" },
        if has_key { "found" } else { "set ANTHROPIC_API_KEY or API_KEY" }
    );

    Ok(())
}

// ─── Graceful shutdown ─────────────────────────────────────────────────────

/// Set up Ctrl+C and SIGTERM handlers for graceful shutdown.
fn setup_shutdown_handler() -> tokio::sync::mpsc::Sender<()> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);

    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut term = signal(SignalKind::terminate()).unwrap();
            tokio::select! {
                _ = rx.recv() => tracing::info!("Shutdown signal received"),
                _ = term.recv() => tracing::info!("SIGTERM received"),
            }
        }

        #[cfg(not(unix))]
        {
            rx.recv().await;
        }

        tracing::info!("Initiating graceful shutdown...");
    });

    tx
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing with env var support and compact format.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    if cli.verbose {
                        tracing_subscriber::EnvFilter::new("debug")
                    } else {
                        tracing_subscriber::EnvFilter::new("info")
                    }
                }),
        )
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .compact()
        .init();

    // Expand and resolve config path.
    let config_path = expand_path(&cli.config);
    let oxios_home = oxios_home_from_config(&config_path);

    // Ensure workspace exists.
    ensure_workspace(&oxios_home)?;

    // Default model ID (used for web server when no subcommand given).
    let default_model = "anthropic/claude-sonnet-4-20250514";

    // Default port to check (used for web server).
    const DEFAULT_PORT: u16 = 4200;

    match cli.command {
        // ── Single prompt execution ───────────────────────
        Some(Command::Run { prompt }) => {
            cmd_run(&prompt, &config_path, default_model).await?;
            return Ok(());
        }

        // ── Garden management ──────────────────────────────
        Some(Command::Garden { action }) => {
            cmd_garden(action, &config_path).await?;
            return Ok(());
        }

        // ── Config commands ───────────────────────────────
        Some(Command::Config { action }) => {
            cmd_config(action, &config_path).await?;
            return Ok(());
        }

        // ── Pkg commands ──────────────────────────────────
        Some(Command::Pkg { action }) => {
            cmd_pkg(action, &config_path).await?;
            return Ok(());
        }

        // ── Status ────────────────────────────────────────
        Some(Command::Status) => {
            cmd_status(&config_path).await?;
            return Ok(());
        }

        // ── Interactive mode (default) ────────────────────
        None => {
            // Check that the port is available before initializing everything.
            let port = DEFAULT_PORT;
            if !is_port_available("127.0.0.1", port).await {
                // Try to detect what's using the port.
                let (occupied, pid) = check_port_occupant("127.0.0.1", port).await;
                eprintln!("Error: Port {} is already in use.", port);
                if let Some(info) = occupied {
                    eprintln!("  Current binding: {}", info);
                    if let Some(p) = pid {
                        eprintln!("  Process PID: {}", p);
                    }
                }
                eprintln!("\nTo use a different port, add this to your config (~/.oxios/config.toml):");
                eprintln!("  [gateway]");
                eprintln!("  port = 4201  # or any available port");
                std::process::exit(1);
            }

            // Initialize kernel components.
            let (_orchestrator, gateway, event_bus, state_store, garden_manager, config, skill_store, supervisor, scheduler, access_manager, program_manager, host_tool_validator, persona_manager) =
                init_kernel(&config_path, default_model).await?;

            // Initialize default skills on first run.
            let defaults_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("channels/oxios-web/static/default-skills");
            if let Err(e) = skill_store.init_defaults(&defaults_dir).await {
                tracing::warn!(error = %e, "Failed to initialize default skills");
            }

            // Initialize default programs on first run.
            let programs_defaults_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("channels/oxios-web/static/default-programs");
            if programs_defaults_dir.exists() {
                for entry in std::fs::read_dir(&programs_defaults_dir)? {
                    let entry = entry?;
                    if entry.path().is_dir()
                        && program_manager.get_program(entry.file_name().to_str().unwrap_or("")).await.is_none()
                    {
                        if let Err(e) = program_manager.install(&entry.path()).await {
                            tracing::warn!(error = %e, program = ?entry.file_name(), "Failed to install default program");
                        }
                    }
                }
            }

            // Initialize default persona (set during init_kernel).

            // Create the web channel.
            let web_channel = oxios_web::WebChannel::new(256);
            let channel_handle = oxios_web::channel::WebChannelHandle::from_channel(&web_channel);
            gateway.register(Box::new(web_channel)).await;

            // Start the host exec bridge if available.
            // (The bridge is created in GardenManager but start() is optional)

            // Create the web server.
            let _web_server = WebServer::new(
                &config.gateway.host,
                config.gateway.port,
                channel_handle,
                event_bus.clone(),
                (*state_store).clone(),
                garden_manager,
                skill_store.clone(),
                program_manager,
                host_tool_validator,
                supervisor.clone(),
                scheduler.clone(),
                access_manager.clone(),
                persona_manager,
                config.clone(),
                Some(config_path.clone()),
            );

            // Set up graceful shutdown.
            let shutdown_tx = setup_shutdown_handler();

            // Build the Axum app using the web server's state.
            let app = _web_server.state();
            let routes = oxios_web::routes::build_routes();
            let static_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("channels/oxios-web/static");
            let app = axum::Router::new()
                .merge(routes)
                .fallback_service(
                    tower_http::services::ServeDir::new(&static_dir)
                        .append_index_html_on_directories(true),
                )
                .layer(tower_http::cors::CorsLayer::permissive())
                .with_state(app);

            // Spawn the gateway loop.
            let gateway_handle = tokio::spawn({
                let g = gateway;
                async move {
                    if let Err(e) = g.run().await {
                        tracing::error!(error = %e, "Gateway error");
                    }
                }
            });

            tracing::info!(
                "Oxios started on http://{}:{}",
                config.gateway.host,
                config.gateway.port
            );

            // Start the web server with graceful shutdown.
            let addr = format!("{}:{}", config.gateway.host, config.gateway.port);
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            tracing::info!(addr = %addr, "Web server listening");

            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    tokio::signal::ctrl_c().await.ok();
                    let _ = shutdown_tx.send(()).await;
                    tracing::info!("Received shutdown signal");
                })
                .await?;

            gateway_handle.abort();
            tracing::info!("Oxios shut down gracefully");
        }
    }

    Ok(())
}

// ─── Port checking utilities ───────────────────────────────────────────────

/// Check if a port is available on the given host.
async fn is_port_available(host: &str, port: u16) -> bool {
    tokio::net::TcpStream::connect(format!("{host}:{port}"))
        .await
        .is_err()
}

/// Get info about what's using a port.
#[allow(unused_variables)]
async fn check_port_occupant(host: &str, port: u16) -> (Option<String>, Option<u32>) {
    use std::process::Command;

    let output = Command::new("lsof")
        .args(["-i", &format!(":{port}"), "-P", "-n"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let lines: Vec<&str> = stdout.lines().collect();
            if lines.len() > 1 {
                // First line is header, remaining lines are data
                let info = lines[1..]
                    .iter()
                    .take(2)
                    .map(|l| l.trim())
                    .collect::<Vec<_>>()
                    .join("\n  ");
                // Extract PID from first line
                let pid = lines.get(1).and_then(|l| {
                    l.split_whitespace()
                        .nth(1)
                        .and_then(|s| s.parse::<u32>().ok())
                });
                (Some(info), pid)
            } else {
                (None, None)
            }
        }
        _ => (None, None),
    }
}
