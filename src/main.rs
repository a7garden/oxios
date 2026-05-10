//! Oxios Agent OS — main binary.
//!
//! Starts the kernel, creates the Ouroboros engine and Agent runtime,
//! registers the web channel, and runs the gateway event loop.

mod kernel;
mod otel;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

    /// Manage running agents.
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },

    /// Verify audit trail integrity.
    Audit,

    /// Git operations on state store.
    Git {
        #[command(subcommand)]
        action: GitAction,
    },

    /// Show agent budget information.
    Budget {
        /// Show budget for a specific agent (UUID).
        agent_id: Option<String>,
    },

    /// Daemon management.
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },

    /// Show program skill file and usage.
    Program {
        /// Program name to display.
        name: String,
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

/// Agent subcommands.
#[derive(Debug, Subcommand)]
enum AgentAction {
    /// List all active agents.
    List,
    /// Kill a running agent by ID.
    Kill { id: String },
}

/// Git subcommands.
#[derive(Debug, Subcommand)]
enum GitAction {
    /// Show recent commits.
    Log { limit: Option<usize> },
    /// Create a tag.
    Tag { name: String, message: Option<String> },
}

/// Daemon subcommands.
#[derive(Debug, Subcommand)]
enum DaemonAction {
    /// Show daemon status.
    Status,
    /// Restart the guardian daemon.
    Restart,
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
    println!();

    let api_key_vars = ["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "API_KEY"];
    let has_key = api_key_vars.iter().any(|v| std::env::var(v).is_ok());
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

    // Initialize OpenTelemetry (OTLP export) after tracing setup
    // This is a no-op when otel.enabled = false
    let early_config = if config_path.exists() {
        oxios_kernel::config::load_config(&config_path).unwrap_or_default()
    } else {
        OxiosConfig::default()
    };
    let _otel_guard = otel::init_otel(&otel::OtelConfig {
        enabled: early_config.otel.enabled,
        endpoint: early_config.otel.endpoint.clone(),
        service_name: early_config.otel.service_name.clone(),
        sampling_ratio: early_config.otel.sampling_ratio,
    }).await?;
    // Keep the guard alive for program duration
    Box::leak(Box::new(_otel_guard));

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
        Some(Command::Config { action }) => {
            cmd_config(action, &config_path).await
        }
        Some(Command::Pkg { action }) => {
            cmd_pkg(action, &config_path).await
        }
        Some(Command::Status) => {
            cmd_status(&config_path).await
        }

        // ── Agent ──────────────────────────────────────────────────────────────────
        Some(Command::Agent { action }) => {
            let kernel = Kernel::builder()
                .config_path(config_path.to_path_buf())
                .build()
                .await?;

            match action {
                AgentAction::List => {
                    let agents = kernel.supervisor.list().await
                        .map_err(|e| anyhow::anyhow!("failed to list agents: {}", e))?;
                    if agents.is_empty() {
                        println!("No active agents.");
                    } else {
                        println!("{:36} {:10} {:20} {}", "ID", "STATUS", "NAME", "CREATED");
                        println!("{}", "-".repeat(90));
                        for agent in &agents {
                            println!(
                                "{:36} {:10} {:20} {}",
                                agent.id,
                                agent.status,
                                agent.name,
                                agent.created_at.format("%Y-%m-%d %H:%M")
                            );
                        }
                        println!();
                        println!("{} agent(s) active.", agents.len());
                    }
                    Ok(())
                }
                AgentAction::Kill { id } => {
                    let uuid = uuid::Uuid::parse_str(&id)
                        .map_err(|e| anyhow::anyhow!("invalid agent id '{}': {}", id, e))?;
                    kernel.supervisor.kill(uuid).await
                        .map_err(|e| anyhow::anyhow!("failed to kill agent {}: {}", id, e))?;
                    println!("Agent {} terminated.", id);
                    Ok(())
                }
            }
        }

        // ── Audit ─────────────────────────────────────────────────────────────────
        Some(Command::Audit) => {
            let kernel = Kernel::builder()
                .config_path(config_path.to_path_buf())
                .build()
                .await?;

            match kernel.audit_trail.verify() {
                Ok(_) => println!("✓ Audit trail verified — chain intact."),
                Err(e) => {
                    eprintln!("✗ Audit verification failed: {:?}", e);
                    println!("  Some entries may have been tampered with.");
                }
            }

            let entries = kernel.handle().security.query_audit(0, 20);
            println!();
            if entries.is_empty() {
                println!("No audit entries yet.");
            } else {
                println!("Recent Audit Entries (showing last {}):", entries.len());
                println!("{:10} {:20} {:15} {}", "SEQ", "TIMESTAMP", "ACTOR", "ACTION");
                println!("{}", "-".repeat(70));
                for entry in entries {
                    println!(
                        "{:10} {:20} {:15} {}",
                        entry.seq,
                        entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
                        entry.actor,
                        format!("{:?}", entry.action)
                    );
                }
            }
            println!();
            println!("Total entries: {}", kernel.handle().security.audit_count());
            Ok(())
        }

        // ── Git ────────────────────────────────────────────────────────────────────
        Some(Command::Git { action }) => {
            let kernel = Kernel::builder()
                .config_path(config_path.to_path_buf())
                .build()
                .await?;

            match action {
                GitAction::Log { limit } => {
                    let limit = limit.unwrap_or(20);
                    match kernel.handle().infra.git_log(limit) {
                        Ok(entries) => {
                            if entries.is_empty() {
                                println!("No commits yet.");
                            } else {
                                println!("{:8} {:20} {:40}", "HASH", "AUTHOR", "MESSAGE");
                                println!("{}", "-".repeat(75));
                                for entry in entries {
                                    let short_hash = &entry.hash[..8.min(entry.hash.len())];
                                    let author = entry.author.chars().take(20).collect::<String>();
                                    let msg = entry.message.chars().take(40).collect::<String>();
                                    println!("{:8} {:20} {:40}", short_hash, author, msg);
                                }
                            }
                            Ok(())
                        }
                        Err(e) => Err(anyhow::anyhow!("failed to get git log: {}", e)),
                    }
                }
                GitAction::Tag { name, message } => {
                    let msg = message.as_deref().unwrap_or("");
                    match kernel.handle().infra.git_tag(&name, msg) {
                        Ok(_) => {
                            println!("Tagged '{}'.", name);
                            if !msg.is_empty() {
                                println!("  Message: {}", msg);
                            }
                            Ok(())
                        }
                        Err(e) => Err(anyhow::anyhow!("failed to create tag: {}", e)),
                    }
                }
            }
        }

        // ── Budget ─────────────────────────────────────────────────────────────────
        Some(Command::Budget { agent_id }) => {
            let kernel = Kernel::builder()
                .config_path(config_path.to_path_buf())
                .build()
                .await?;

            match agent_id {
                Some(id) => {
                    let uuid = uuid::Uuid::parse_str(&id)
                        .map_err(|e| anyhow::anyhow!("invalid agent id '{}': {}", id, e))?;
                    let budget = kernel.handle().agents.check_budget(&uuid);
                    println!("Budget for agent {}", id);
                    println!("{}", "-".repeat(40));
                    println!("  Tokens remaining: {}", budget.tokens_remaining);
                    println!("  Calls remaining:  {}", budget.calls_remaining);
                    println!("  Window remaining:   {} seconds", budget.window_remaining_secs);
                    if budget.is_exhausted {
                        println!("  Status: EXHAUSTED");
                    } else {
                        println!("  Status: OK");
                    }
                    Ok(())
                }
                None => {
                    println!("Agent Budget Overview");
                    println!("{}", "=".repeat(50));
                    println!("(No agent metadata available without agent list)");
                    println!();
                    println!("Use 'oxios agent list' to see agent IDs,");
                    println!("then 'oxios budget <agent-id>' for details.");
                    Ok(())
                }
            }
        }

        // ── Daemon ─────────────────────────────────────────────────────────────────
        Some(Command::Daemon { action }) => {
            match action {
                DaemonAction::Status => {
                    println!("Guardian Daemon");
                    println!("{}", "-".repeat(30));
                    println!("  Status: running (background tokio task)");
                    println!("  Purpose: periodic integrity checks and cleanup");
                    println!();
                    println!("The daemon runs as a background task within the server process.");
                    println!("Start the server with 'oxios' to activate the daemon.");
                    Ok(())
                }
                DaemonAction::Restart => {
                    println!("Daemon restart not supported via CLI.");
                    println!("Restart the entire server process: pkill -f oxios && oxios");
                    Ok(())
                }
            }
        }

        // ── Program ────────────────────────────────────────────────────────────────
        Some(Command::Program { name }) => {
            let kernel = Kernel::builder()
                .config_path(config_path.to_path_buf())
                .build()
                .await?;

            match kernel.handle().extensions.get_program(&name).await {
                Some(program) => {
                    println!("Program: {} v{}", program.meta.name, program.meta.version);
                    println!("{}", "-".repeat(50));
                    println!();
                    if !program.skill_content.is_empty() {
                        println!("SKILL.md:");
                        println!("{}", program.skill_content);
                    } else {
                        println!("(No SKILL.md found in program)");
                    }
                    println!();
                    println!("Description: {}", program.meta.description);
                    if !program.meta.tools.is_empty() {
                        println!();
                        println!("Tools:");
                        for tool in &program.meta.tools {
                            println!("  - {}: {}", tool.name, tool.description);
                        }
                    }
                    if !program.meta.host_requirements.required.is_empty() {
                        println!();
                        println!("Required host tools: {}", program.meta.host_requirements.required.join(", "));
                    }
                    if !program.meta.host_requirements.optional.is_empty() {
                        println!();
                        println!("Optional host tools: {}", program.meta.host_requirements.optional.join(", "));
                    }
                    Ok(())
                }
                None => Err(anyhow::anyhow!(
                    "program '{}' not found. Install with 'oxios pkg install'",
                    name
                )),
            }
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
                kernel.handle(),
                Arc::new(parking_lot::RwLock::new(kernel.config.clone())),
                Some(config_path.clone()),
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

            // Start guardian daemon for background integrity checks
            kernel.start_guardian();

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
