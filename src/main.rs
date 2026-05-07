//! Oxios Agent OS — main binary.
//!
//! Starts the kernel, creates the Ouroboros engine and Agent runtime,
//! registers the web channel, and runs the gateway event loop.

mod kernel;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

use kernel::Kernel;
use oxios_kernel::{OxiosConfig, InstallSource};
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

    /// Start an interactive CLI chat session.
    Chat,

    /// Backup Oxios state.
    Backup {
        /// Output directory for the backup (default: <workspace>/backups/<timestamp>).
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Restore Oxios state from a backup.
    Restore {
        /// Input backup directory path.
        input: String,
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
    New { name: String },
    /// Start a garden container.
    Up { name: String },
    /// Stop a garden container.
    Down { name: String },
    /// Remove a garden entirely.
    Remove { name: String },
    /// List all gardens.
    List,
    /// Execute a command inside a garden.
    Exec {
        name: String,
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigAction {
    /// Show the current configuration.
    Show,
    /// Set a configuration value.
    Set { key: String, value: String },
    /// Get a configuration value.
    Get { key: String },
}

#[derive(Debug, Subcommand)]
enum PkgAction {
    /// Install a program from a URL or local path.
    Install {
        #[arg(value_name = "SOURCE")]
        source: String,
        #[arg(short, long)]
        branch: Option<String>,
    },
    /// Uninstall a program by name.
    Uninstall { name: String },
    /// List all installed programs.
    List,
    /// Search programs (stub — lists installed programs).
    Search,
}

// ─── Workspace helpers ─────────────────────────────────────────────────────

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

const DEFAULT_CONFIG: &str = include_str!("../channels/oxios-web/static/default-config.toml");

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

fn expand_path(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(path.replacen("~/", &format!("{home}/"), 1));
        }
    }
    PathBuf::from(path)
}

fn oxios_home_from_config(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .map(|p: &Path| p.to_path_buf())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            PathBuf::from(format!("{home}/.oxios"))
        })
}

// ─── Commands ───────────────────────────────────────────────────────────────

async fn cmd_run(prompt: &str, config_path: &Path, model_id: &str) -> Result<()> {
    let kernel = Kernel::builder()
        .config_path(config_path.to_path_buf())
        .model_id(model_id)
        .build()
        .await?;

    tracing::info!(prompt = %prompt, "Processing prompt");

    let result = kernel.orchestrator.handle_message("cli", prompt, None).await?;

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

async fn cmd_garden(action: GardenAction, config_path: &Path) -> Result<()> {
    let kernel = Kernel::builder()
        .config_path(config_path.to_path_buf())
        .build()
        .await?;

    match action {
        GardenAction::New { name } => {
            kernel.container_manager.new_container(&name).await?;
            println!("Garden '{}' created.", name);
        }
        GardenAction::Up { name } => {
            if !kernel.container_manager.is_backend_available() {
                println!("⚠️  Container runtime not available. Garden metadata recorded.");
                println!("   To start the container, install 'container' CLI from Xcode.");
            } else {
                kernel.container_manager.start_container(&name).await?;
                println!("Garden '{}' started.", name);
            }
        }
        GardenAction::Down { name } => {
            if !kernel.container_manager.is_backend_available() {
                println!("⚠️  Container runtime not available.");
            } else {
                kernel.container_manager.stop_container(&name).await?;
                println!("Garden '{}' stopped.", name);
            }
        }
        GardenAction::Remove { name } => {
            kernel.container_manager.remove_container(&name).await?;
            println!("Garden '{}' removed.", name);
        }
        GardenAction::List => {
            let gardens: Vec<_> = kernel.container_manager.list_containers().await?;
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
            if !kernel.container_manager.is_backend_available() {
                bail!("Container runtime not available. Cannot exec in garden.");
            }
            let result = kernel.container_manager.exec_in_container(&name, &command, None).await?;
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

async fn cmd_pkg(action: PkgAction, config_path: &Path) -> Result<()> {
    let kernel = Kernel::builder()
        .config_path(config_path.to_path_buf())
        .build()
        .await?;

    match action {
        PkgAction::Install { source, branch } => {
            let install_source = if source.ends_with(".git") || source.starts_with("git@") {
                InstallSource::Git { url: source, branch }
            } else if source.starts_with("http://") || source.starts_with("https://") {
                InstallSource::Tarball { url: source }
            } else {
                InstallSource::Local(PathBuf::from(&source))
            };
            let program = kernel.program_manager.install_from(install_source).await?;
            println!("Installed '{}' v{}", program.meta.name, program.meta.version);
        }
        PkgAction::Uninstall { name } => {
            kernel.program_manager.uninstall(&name).await?;
            println!("Uninstalled '{}'", name);
        }
        PkgAction::List => {
            let programs = kernel.program_manager.list_programs().await;
            if programs.is_empty() {
                println!("No programs installed.");
            } else {
                println!("{:30} {:10} {:40}", "NAME", "VERSION", "DESCRIPTION");
                println!("{}", "-".repeat(82));
                for p in &programs {
                    println!("{:30} {:10} {:40}", p.meta.name, p.meta.version, p.meta.description);
                }
            }
        }
        PkgAction::Search => {
            let programs = kernel.program_manager.list_programs().await;
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

async fn cmd_config(action: ConfigAction, config_path: &Path) -> Result<()> {
    let config = if config_path.exists() {
        oxios_kernel::config::load_config(config_path)?
    } else {
        OxiosConfig::default()
    };
    match action {
        ConfigAction::Show => {
            let toml_str = toml::to_string_pretty(&config).context("failed to serialize config")?;
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

fn get_config_value(config: &OxiosConfig, key: &str) -> Option<String> {
    let parts: Vec<&str> = key.split('.').collect();
    match parts.as_slice() {
        ["kernel", "workspace"] => Some(config.kernel.workspace.clone()),
        ["kernel", "event_bus_capacity"] => Some(config.kernel.event_bus_capacity.to_string()),
        ["kernel", "max_agents"] => Some(config.kernel.max_agents.to_string()),
        ["gateway", "host"] => Some(config.gateway.host.clone()),
        ["gateway", "port"] => Some(config.gateway.port.to_string()),
        ["container", "container_path"] => Some(config.container.container_path.clone()),
        ["container", "image_tag"] => Some(config.container.image_tag.clone()),
        ["container", "memory_limit"] => Some(config.container.memory_limit.clone()),
        ["container", "cpu_limit"] => Some(config.container.cpu_limit.to_string()),
        _ => None,
    }
}

async fn cmd_status(config_path: &Path) -> Result<()> {
    let kernel = Kernel::builder()
        .config_path(config_path.to_path_buf())
        .build()
        .await?;

    println!("Oxios Agent OS");
    println!("{}", "=".repeat(40));
    println!("  Version: {}", env!("CARGO_PKG_VERSION"));
    println!("  Config: {}", config_path.display());
    println!("  Workspace: {}", kernel.config.kernel.workspace);
    println!("  Garden path: {}", kernel.config.container.container_path);
    println!();

    let backend_available = kernel.container_manager.is_backend_available();
    println!("Container Runtime:");
    println!("  Backend: {}", kernel.container_manager.backend_name());
    println!("  Available: {} ({})",
        if backend_available { "yes" } else { "no" },
        if backend_available { "ready" } else { "install container CLI from Xcode" });
    println!();

    let gardens: Vec<_> = kernel.container_manager.list_containers().await?;
    println!("Gardens: {} known", gardens.len());
    if !gardens.is_empty() {
        let running = gardens.iter().filter(|g| g.running).count();
        println!("  {} running, {} stopped", running, gardens.len() - running);
    }

    let api_key_vars = ["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "API_KEY"];
    let has_key = api_key_vars.iter().any(|v| std::env::var(v).is_ok());
    println!();
    println!("API Keys:");
    println!("  Configured: {} ({})",
        if has_key { "yes" } else { "no" },
        if has_key { "found" } else { "set ANTHROPIC_API_KEY or API_KEY" });

    let mcp_count = kernel.mcp_bridge.servers().len();
    println!();
    println!("MCP Servers: {} configured", mcp_count);

    Ok(())
}

// ─── Graceful shutdown ─────────────────────────────────────────────────────

fn setup_shutdown_handler() -> tokio::sync::mpsc::Sender<()> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut term = signal(SignalKind::terminate()).expect("Failed to install SIGTERM handler");
            tokio::select! {
                _ = rx.recv() => tracing::info!("Shutdown signal received"),
                _ = term.recv() => tracing::info!("SIGTERM received"),
            }
        }
        #[cfg(not(unix))]
        { rx.recv().await; }
        tracing::info!("Initiating graceful shutdown...");
    });
    tx
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config_path = expand_path(&cli.config);
    let oxios_home = oxios_home_from_config(&config_path);
    ensure_workspace(&oxios_home)?;

    // ── Tracing setup with file appender ──
    let log_dir = oxios_home.join("logs");
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, "oxios.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Leak the guard so it lives for the program duration.
    // Without this, the guard would be dropped and log flushing would stop.
    Box::leak(Box::new(_guard));

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            if cli.verbose {
                tracing_subscriber::EnvFilter::new("debug")
            } else {
                tracing_subscriber::EnvFilter::new("info")
            }
        });

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .compact()
        .with_writer(non_blocking)
        .init();

    let default_model = "anthropic/claude-sonnet-4-20250514";
    const DEFAULT_PORT: u16 = 4200;

    match cli.command {
        Some(Command::Run { prompt }) => {
            cmd_run(&prompt, &config_path, default_model).await
        }
        Some(Command::Chat) => {
            let kernel = Kernel::builder()
                .config_path(config_path.to_path_buf())
                .build()
                .await?;
            let cli_channel = oxios_cli::CliChannel::new(256);
            let handle = cli_channel.handle();
            kernel.gateway.register(Box::new(cli_channel)).await;
            let mut loop_ = oxios_cli::InteractiveLoop::new(handle);
            loop_.run().await?;
            Ok(())
        }
        Some(Command::Backup { output }) => {
            let kernel = Kernel::builder()
                .config_path(config_path.to_path_buf())
                .build()
                .await?;
            let output_path = match output {
                Some(p) => std::path::PathBuf::from(p),
                None => {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    kernel.state_store.base_path.join("backups").join(ts.to_string())
                }
            };
            oxios_kernel::backup::create_backup(&kernel.state_store, &output_path).await?;
            Ok(())
        }
        Some(Command::Restore { input }) => {
            let kernel = Kernel::builder()
                .config_path(config_path.to_path_buf())
                .build()
                .await?;
            let input_path = std::path::PathBuf::from(&input);
            oxios_kernel::backup::restore_backup(&kernel.state_store, &input_path).await?;
            Ok(())
        }
        Some(Command::Garden { action }) => {
            cmd_garden(action, &config_path).await
        }
        Some(Command::Config { action }) => {
            cmd_config(action, &config_path).await
        }
        Some(Command::Pkg { action }) => {
            cmd_pkg(action, &config_path).await
        }
        Some(Command::Status) => {
            cmd_status(&config_path).await
        }

        // ── Interactive mode (default) ──
        None => {
            if !is_port_available("127.0.0.1", DEFAULT_PORT).await {
                let (occupied, pid) = check_port_occupant("127.0.0.1", DEFAULT_PORT).await;
                eprintln!("Error: Port {} is already in use.", DEFAULT_PORT);
                if let Some(info) = occupied {
                    eprintln!("  Current binding: {}", info);
                    if let Some(p) = pid {
                        eprintln!("  Process PID: {}", p);
                    }
                }
                eprintln!("\nTo use a different port, add this to your config (~/.oxios/config.toml):");
                eprintln!("  [gateway]");
                eprintln!("  port = 4201");
                std::process::exit(1);
            }

            let kernel = Kernel::builder()
                .config_path(config_path.clone())
                .model_id(default_model)
                .build()
                .await?;

            // Initialize MCP servers
            if !kernel.config.mcp.servers.is_empty() {
                if let Err(e) = kernel.mcp_bridge.initialize_all().await {
                    tracing::warn!(error = %e, "Some MCP servers failed to initialize");
                } else {
                    tracing::info!(count = kernel.config.mcp.servers.len(), "MCP servers initialized");
                }
            }

            // Initialize default skills
            let defaults_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("channels/oxios-web/static/default-skills");
            if let Err(e) = kernel.skill_store.init_defaults(&defaults_dir).await {
                tracing::warn!(error = %e, "Failed to initialize default skills");
            }

            // Initialize default programs
            let programs_defaults_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("channels/oxios-web/static/default-programs");
            if programs_defaults_dir.exists() {
                for entry in std::fs::read_dir(&programs_defaults_dir)? {
                    let entry = entry?;
                    if entry.path().is_dir()
                        && kernel.program_manager.get_program(entry.file_name().to_str().unwrap_or("")).await.is_none()
                    {
                        if let Err(e) = kernel.program_manager.install(&entry.path()).await {
                            tracing::warn!(error = %e, program = ?entry.file_name(), "Failed to install default program");
                        }
                    }
                }
            }

            // Create web channel
            let web_channel = oxios_web::WebChannel::new(256);
            let channel_handle = oxios_web::channel::WebChannelHandle::from_channel(&web_channel);
            kernel.gateway.register(Box::new(web_channel)).await;

            // Create web server
            let _web_server = WebServer::new(
                &kernel.config.gateway.host,
                kernel.config.gateway.port,
                channel_handle,
                kernel.event_bus.clone(),
                (*kernel.state_store).clone(),
                kernel.container_manager.clone(),
                kernel.skill_store.clone(),
                kernel.program_manager.clone(),
                kernel.host_tool_validator.clone(),
                kernel.supervisor.clone(),
                kernel.scheduler.clone(),
                kernel.access_manager.clone(),
                kernel.persona_manager.clone(),
                kernel.config.clone(),
                Some(config_path),
                kernel.mcp_bridge.clone(),
                kernel.auth_manager.clone(),
                kernel.memory_manager.clone(),
            )?;

            let shutdown_tx = setup_shutdown_handler();

            // Build Axum app
            let app = _web_server.state();
            let routes = oxios_web::routes::build_routes(app.clone());
            let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("channels/oxios-web/static");
            let app = axum::Router::new()
                .merge(routes)
                .fallback_service(
                    tower_http::services::ServeDir::new(&static_dir)
                        .append_index_html_on_directories(true),
                )
                .with_state(app);

            // Spawn gateway loop
            let gateway_handle = tokio::spawn({
                let g = kernel.gateway;
                async move {
                    if let Err(e) = g.run().await {
                        tracing::error!(error = %e, "Gateway error");
                    }
                }
            });

            tracing::info!("Oxios started on http://{}:{}", kernel.config.gateway.host, kernel.config.gateway.port);

            let addr = format!("{}:{}", kernel.config.gateway.host, kernel.config.gateway.port);
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            tracing::info!(addr = %addr, "Web server listening");

            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    tokio::signal::ctrl_c().await.ok();
                    let _ = shutdown_tx.send(()).await;
                    tracing::info!("Received shutdown signal");
                })
                .await?;

            // Structured shutdown sequence
            tracing::info!("Starting graceful shutdown...");

            // 1. Stop running agents
            if let Ok(agents) = kernel.supervisor.list().await {
                for agent in &agents {
                    if let Err(e) = kernel.supervisor.kill(agent.id).await {
                        tracing::warn!(agent = %agent.id, error = %e, "Failed to kill agent");
                    }
                }
                if !agents.is_empty() {
                    tracing::info!(count = agents.len(), "Agents terminated");
                }
            }

            // 2. Stop MCP servers
            if let Err(e) = kernel.mcp_bridge.shutdown_all().await {
                tracing::warn!(error = %e, "MCP shutdown error");
            }

            // 3. Stop gateway
            gateway_handle.abort();

            tracing::info!("Oxios shut down gracefully");
            Ok(())
        }
    }
}

// ─── Port checking utilities ───────────────────────────────────────────────

async fn is_port_available(host: &str, port: u16) -> bool {
    tokio::net::TcpStream::connect(format!("{host}:{port}"))
        .await
        .is_err()
}

#[allow(unused_variables)]
async fn check_port_occupant(host: &str, port: u16) -> (Option<String>, Option<u32>) {
    let output = std::process::Command::new("lsof")
        .args(["-i", &format!(":{port}"), "-P", "-n"])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let lines: Vec<&str> = stdout.lines().collect();
            if lines.len() > 1 {
                let info = lines[1..].iter().take(2).map(|l| l.trim()).collect::<Vec<_>>().join("\n  ");
                let pid = lines.get(1).and_then(|l| {
                    l.split_whitespace().nth(1).and_then(|s| s.parse::<u32>().ok())
                });
                (Some(info), pid)
            } else {
                (None, None)
            }
        }
        _ => (None, None),
    }
}
