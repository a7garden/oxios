//! Oxios Agent OS — main binary.
//!
//! Default invocation (`oxios`) starts the daemon in the background.
//! Use `oxios --foreground` to run in the foreground (for debugging).
//! First run without credentials triggers an interactive setup wizard.

mod kernel;
mod otel;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kernel::Kernel;
use oxios_kernel::{credential::CredentialStore, DaemonManager, OxiosConfig};

#[cfg(feature = "cli")]
use oxios_cli::CliPlugin;
#[cfg(feature = "telegram")]
use oxios_telegram::TelegramPlugin;
#[cfg(feature = "web")]
use oxios_web::WebPlugin;

use oxios_gateway::plugin::{ChannelContext, ChannelPlugin};

// ─── CLI ───────────────────────────────────────────────────────────────────

/// Oxios Agent OS
#[derive(Debug, Parser)]
#[command(
    name = "oxios",
    version,
    about = "Oxios Agent OS — Agent Operating System"
)]
struct Cli {
    /// Run in foreground (do not daemonize).
    #[arg(long, global = true)]
    foreground: bool,

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
    /// Stop the running daemon.
    Stop,

    /// Restart the daemon.
    Restart,

    /// Show system status (daemon, credentials, agents).
    Status,

    /// Run a single prompt through the Ouroboros flow.
    #[command(arg_required_else_help = true)]
    Run {
        /// The prompt to execute.
        prompt: String,
    },

    /// Start an interactive CLI chat session.
    Chat,

    /// Backup Oxios state.
    Backup {
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Restore Oxios state from a backup.
    Restore { input: String },

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
    Budget { agent_id: Option<String> },

    /// Manage system service (launchd/systemd).
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },

    /// Tail daemon log.
    Log {
        /// Number of lines to show.
        #[arg(short, long, default_value = "50")]
        lines: usize,
    },

    /// Show program skill file and usage.
    Program { name: String },
}

#[derive(Debug, Subcommand)]
enum ConfigAction {
    Show,
    Set { key: String, value: String },
    Get { key: String },
}

#[derive(Debug, Subcommand)]
enum PkgAction {
    Install {
        source: String,
        #[arg(short, long)]
        branch: Option<String>,
    },
    Uninstall {
        name: String,
    },
    List,
    Search,
}

#[derive(Debug, Subcommand)]
enum AgentAction {
    List,
    Kill { id: String },
}

#[derive(Debug, Subcommand)]
enum GitAction {
    Log {
        limit: Option<usize>,
    },
    Tag {
        name: String,
        message: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum DaemonAction {
    /// Install as system service (launchd/systemd).
    Install,
    /// Uninstall system service.
    Uninstall,
}

// ─── Constants & helpers ───────────────────────────────────────────────────

const WORKSPACE_SUBDIRS: &[&str] = &[
    "workspace",
    "workspace/memory",
    "workspace/memory/knowledge",
    "workspace/seeds",
    "workspace/sessions",
    "workspace/skills",
    "workspace/programs",
];

const DEFAULT_CONFIG: &str = include_str!("../share/default-config.toml");

fn ensure_workspace(oxios_home: &Path) -> Result<()> {
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

fn oxios_home_from_config(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            PathBuf::from(format!("{home}/.oxios"))
        })
}

// ─── Subcommands ───────────────────────────────────────────────────────────

async fn cmd_run_async(kernel: &Kernel, prompt: &str) -> Result<()> {
    tracing::info!(prompt = %prompt, "Processing prompt");

    kernel.handle().security.audit(
        "cli",
        oxios_kernel::audit_trail::AuditAction::Other {
            detail: format!("run: {}", prompt.chars().take(100).collect::<String>()),
        },
        "cli-user",
    );

    let result = kernel.execute_prompt(prompt).await?;

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

async fn cmd_pkg(kernel: &Kernel, action: &PkgAction) -> Result<()> {
    let handle = kernel.handle();
    match action {
        PkgAction::Install { source, branch } => {
            let source = source.clone();
            let branch = branch.clone();
            let install_source = if source.ends_with(".git") || source.starts_with("git@") {
                oxios_kernel::InstallSource::Git {
                    url: source,
                    branch,
                }
            } else if source.starts_with("http://") || source.starts_with("https://") {
                oxios_kernel::InstallSource::Tarball { url: source }
            } else {
                oxios_kernel::InstallSource::Local(PathBuf::from(&source))
            };
            let program = handle.extensions.install_program(install_source).await?;
            println!(
                "Installed '{}' v{}",
                program.meta.name, program.meta.version
            );
        }
        PkgAction::Uninstall { name } => {
            handle.extensions.uninstall_program(name).await?;
            println!("Uninstalled '{}'", name);
        }
        PkgAction::List => {
            let programs = handle.extensions.list_programs().await;
            if programs.is_empty() {
                println!("No programs installed.");
            } else {
                println!("{:30} {:10} {:40}", "NAME", "VERSION", "DESCRIPTION");
                println!("{}", "-".repeat(82));
                for p in &programs {
                    println!("{:30} {:10} {:40}", p.name, p.version, p.description);
                }
            }
        }
        PkgAction::Search => {
            let programs = handle.extensions.list_programs().await;
            if programs.is_empty() {
                println!("No programs installed.");
            } else {
                for p in &programs {
                    println!("{} ({})", p.name, p.version);
                    println!("  {}", p.description);
                    if !p.tools.is_empty() {
                        let tools: Vec<_> = p.tools.iter().map(|t| t.name.clone()).collect();
                        println!("  Tools: {}", tools.join(", "));
                    }
                    println!();
                }
            }
        }
    }
    Ok(())
}

async fn cmd_config(action: &ConfigAction, config_path: &Path) -> Result<()> {
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
            let value = get_config_value(&config, key)
                .ok_or_else(|| anyhow::anyhow!("Unknown config key: {}", key))?;
            println!("{}", value);
        }
        ConfigAction::Set { key, value } => {
            let mut config = if config_path.exists() {
                let raw = std::fs::read_to_string(config_path)?;
                toml::from_str(&raw)?
            } else {
                OxiosConfig::default()
            };
            set_config_value(&mut config, key, value)
                .ok_or_else(|| anyhow::anyhow!("Unknown config key: {}", key))?;
            let toml_str = toml::to_string_pretty(&config)?;
            std::fs::write(config_path, toml_str)?;
            println!("Set {} = {}", key, value);
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
        ["engine", "default_model"] => Some(config.engine.default_model.clone()),
        ["engine", "api_key"] => Some(config.engine.api_key.clone().unwrap_or_default()),
        ["gateway", "host"] => Some(config.gateway.host.clone()),
        ["gateway", "port"] => Some(config.gateway.port.to_string()),
        ["exec", "default_timeout_secs"] => Some(config.exec.default_timeout_secs.to_string()),
        ["exec", "max_timeout_secs"] => Some(config.exec.max_timeout_secs.to_string()),
        _ => None,
    }
}

fn set_config_value(config: &mut OxiosConfig, key: &str, value: &str) -> Option<()> {
    let parts: Vec<&str> = key.split('.').collect();
    match parts.as_slice() {
        ["kernel", "workspace"] => {
            config.kernel.workspace = value.to_string();
            Some(())
        }
        ["kernel", "event_bus_capacity"] => {
            config.kernel.event_bus_capacity = value.parse().ok()?;
            Some(())
        }
        ["kernel", "max_agents"] => {
            config.kernel.max_agents = value.parse().ok()?;
            Some(())
        }
        ["engine", "default_model"] => {
            config.engine.default_model = value.to_string();
            Some(())
        }
        ["engine", "api_key"] => {
            config.engine.api_key = Some(value.to_string());
            Some(())
        }
        ["gateway", "host"] => {
            config.gateway.host = value.to_string();
            Some(())
        }
        ["gateway", "port"] => {
            config.gateway.port = value.parse().ok()?;
            Some(())
        }
        ["exec", "default_timeout_secs"] => {
            config.exec.default_timeout_secs = value.parse().ok()?;
            Some(())
        }
        ["exec", "max_timeout_secs"] => {
            config.exec.max_timeout_secs = value.parse().ok()?;
            Some(())
        }
        ["exec", "required_host_tools"] => {
            config.exec.required_host_tools = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            Some(())
        }
        ["exec", "optional_host_tools"] => {
            config.exec.optional_host_tools = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            Some(())
        }
        _ => None,
    }
}

async fn cmd_status(kernel: &Kernel) -> Result<()> {
    let config = kernel.config();
    let daemon = DaemonManager::new(&config.daemon.pid_file, &config.daemon.log_dir);

    println!("\n  ⬡ Oxios Agent OS  v{}", env!("CARGO_PKG_VERSION"));
    println!("  {}", "─".repeat(48));
    println!("  {:<16}  {}", "Workspace:", config.kernel.workspace);
    println!("  {:<16}  {}", "Model:", config.engine.default_model);
    println!("  {:<16}  {}", "Daemon:", daemon.status());
    println!();

    // Credential source
    let provider = CredentialStore::provider_from_model(&config.engine.default_model);
    match CredentialStore::resolve(provider, config.api_key().as_deref()) {
        Some((key, source)) => {
            let source_str = match source {
                oxios_kernel::credential::CredentialSource::Config => {
                    "config.toml (engine.api_key)"
                }
                oxios_kernel::credential::CredentialSource::OxiAuthStore => "~/.oxi/auth.json",
                oxios_kernel::credential::CredentialSource::EnvVar => "environment variable",
            };
            let preview = if key.len() > 8 {
                format!("{}...{}", &key[..4], &key[key.len() - 4..])
            } else {
                key.clone()
            };
            println!("  {:<16}  {} [{}]", "Credentials:", preview, source_str);
        }
        None => {
            println!("  {:<16}  ✗ none (run `oxios` to setup)", "Credentials:");
        }
    }

    // Active agents
    let mcp_count = kernel.handle().mcp.server_count();
    println!("  {:<16}  {}", "MCP Servers:", mcp_count);

    let agents = kernel
        .handle()
        .agents
        .list()
        .await
        .map_err(|e| anyhow::anyhow!("failed to list agents: {}", e))?;
    println!("  {:<16}  {}", "Active Agents:", agents.len());
    if !agents.is_empty() {
        println!();
        for agent in &agents {
            println!(
                "    • {}  {:10}  {}",
                agent.id,
                format!("{:?}", agent.status),
                agent.name
            );
        }
    }

    println!();
    Ok(())
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config_path = oxios_kernel::config::expand_home(&cli.config);
    let oxios_home = oxios_home_from_config(&config_path);
    ensure_workspace(&oxios_home)?;

    // ── Load config ──
    let mut config = if config_path.exists() {
        oxios_kernel::config::load_config(&config_path)?
    } else {
        OxiosConfig::default()
    };

    // ── Tracing setup ──
    let log_dir = oxios_kernel::config::expand_home(&config.daemon.log_dir);
    std::fs::create_dir_all(&log_dir)?;
    let file_appender = tracing_appender::rolling::daily(&log_dir, "oxios.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    Box::leak(Box::new(_guard));

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
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

    // ── OpenTelemetry ──
    let _otel_guard = otel::init_otel(&config.otel).await?;
    Box::leak(Box::new(_otel_guard));

    // ── Fast-path: commands that never need the kernel ──
    match &cli.command {
        Some(Command::Stop) => {
            let daemon = DaemonManager::new(&config.daemon.pid_file, &config.daemon.log_dir);
            return daemon.stop();
        }
        Some(Command::Daemon { action }) => {
            let daemon = DaemonManager::new(&config.daemon.pid_file, &config.daemon.log_dir);
            return match action {
                DaemonAction::Install => daemon.install_service(),
                DaemonAction::Uninstall => daemon.uninstall_service(),
            };
        }
        Some(Command::Log { lines }) => {
            let log_file = log_dir.join("oxios.log");
            if !log_file.exists() {
                println!("No log file at {}", log_file.display());
                return Ok(());
            }
            let output = std::process::Command::new("tail")
                .args(["-n", &lines.to_string()])
                .arg(&log_file)
                .output()?;
            print!("{}", String::from_utf8_lossy(&output.stdout));
            return Ok(());
        }
        Some(Command::Config { action }) => {
            return cmd_config(action, &config_path).await;
        }
        _ => {}
    }

    // ── Onboarding gate ──
    // Commands that need the kernel assembled (and therefore credentials).
    let needs_kernel = matches!(
        cli.command.as_ref(),
        None | Some(Command::Run { .. })
            | Some(Command::Chat)
            | Some(Command::Status)
            | Some(Command::Agent { .. })
            | Some(Command::Backup { .. })
            | Some(Command::Restore { .. })
            | Some(Command::Audit)
            | Some(Command::Budget { .. })
            | Some(Command::Git { .. })
            | Some(Command::Program { .. })
            | Some(Command::Pkg { .. })
    );

    if needs_kernel && !oxios_kernel::onboarding::has_credentials(&config) {
        let completed = oxios_kernel::onboarding::run_onboarding(&oxios_home, &mut config)?;
        if completed {
            if config_path.exists() {
                config = oxios_kernel::config::load_config(&config_path)?;
            }
        } else {
            return Ok(());
        }
    }

    // ── Kernel assembly (needed by most commands) ──
    let kernel = Kernel::builder()
        .config_path(config_path.clone())
        .build()
        .await?;

    // ── Dispatch subcommands ──
    match cli.command.as_ref() {
        // Default: start daemon
        None => {
            let daemon = DaemonManager::new(&config.daemon.pid_file, &config.daemon.log_dir);
            if cli.foreground {
                cmd_serve(&kernel, &config_path).await
            } else {
                daemon.start(&config_path)
            }
        }

        Some(Command::Restart) => {
            let daemon = DaemonManager::new(&config.daemon.pid_file, &config.daemon.log_dir);
            daemon.restart(&config_path)
        }

        Some(Command::Run { prompt }) => cmd_run_async(&kernel, prompt).await,

        Some(Command::Status) => cmd_status(&kernel).await,

        Some(Command::Chat) => {
            #[cfg(feature = "cli")]
            {
                let cli_channel = oxios_cli::CliChannel::new(256);
                let handle = cli_channel.handle();
                kernel.register_channel(Box::new(cli_channel)).await;
                let mut loop_ = oxios_cli::InteractiveLoop::new(handle);
                loop_.run().await?;
                Ok(())
            }
            #[cfg(not(feature = "cli"))]
            {
                anyhow::bail!("CLI channel not compiled in. Rebuild with --features cli");
            }
        }

        Some(Command::Backup { output }) => {
            let handle = kernel.handle();
            let output_path = match output {
                Some(p) => PathBuf::from(p),
                None => {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    PathBuf::from(kernel.config().kernel.workspace.clone())
                        .join("backups")
                        .join(ts.to_string())
                }
            };
            oxios_kernel::backup::create_backup(handle.state.store(), &output_path).await?;
            Ok(())
        }

        Some(Command::Restore { input }) => {
            let handle = kernel.handle();
            let input_path = PathBuf::from(&input);
            oxios_kernel::backup::restore_backup(handle.state.store(), &input_path).await?;
            Ok(())
        }

        Some(Command::Pkg { action }) => cmd_pkg(&kernel, action).await,

        Some(Command::Agent { action }) => {
            let handle = kernel.handle();
            match action {
                AgentAction::List => {
                    let agents = handle
                        .agents
                        .list()
                        .await
                        .map_err(|e| anyhow::anyhow!("failed to list agents: {}", e))?;
                    if agents.is_empty() {
                        println!("No active agents.");
                    } else {
                        println!("{:36} {:10} {:20} CREATED", "ID", "STATUS", "NAME");
                        println!("{}", "-".repeat(90));
                        for agent in &agents {
                            println!(
                                "{:36} {:10} {:20} {}",
                                agent.id,
                                format!("{:?}", agent.status),
                                agent.name,
                                agent.created_at.format("%Y-%m-%d %H:%M")
                            );
                        }
                        println!("\n{} agent(s) active.", agents.len());
                    }
                    Ok(())
                }
                AgentAction::Kill { id } => {
                    let _ = uuid::Uuid::parse_str(id)
                        .map_err(|e| anyhow::anyhow!("invalid agent id '{}': {}", id, e))?;
                    handle
                        .agents
                        .kill(id)
                        .await
                        .map_err(|e| anyhow::anyhow!("failed to kill agent {}: {}", id, e))?;
                    println!("Agent {} terminated.", id);
                    Ok(())
                }
            }
        }

        Some(Command::Audit) => {
            let handle = kernel.handle();
            match handle.security.verify_chain() {
                Ok(_) => println!("✓ Audit trail verified — chain intact."),
                Err(e) => {
                    eprintln!("✗ Audit verification failed: {:?}", e);
                    println!("  Some entries may have been tampered with.");
                }
            }
            let entries = handle.security.query_audit(0, 20);
            println!();
            if entries.is_empty() {
                println!("No audit entries yet.");
            } else {
                println!("Recent Audit Entries (showing last {}):", entries.len());
                println!("{:10} {:20} {:15} ACTION", "SEQ", "TIMESTAMP", "ACTOR");
                println!("{}", "-".repeat(70));
                for entry in &entries {
                    println!(
                        "{:10} {:20} {:15} {:?}",
                        entry.seq,
                        entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
                        entry.actor,
                        entry.action
                    );
                }
            }
            println!("\nTotal entries: {}", handle.security.audit_count());
            Ok(())
        }

        Some(Command::Git { action }) => {
            let handle = kernel.handle();
            match action {
                GitAction::Log { limit } => {
                    let limit = limit.unwrap_or(20);
                    let entries = handle
                        .infra
                        .git_log(limit)
                        .map_err(|e| anyhow::anyhow!("failed to get git log: {}", e))?;
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
                GitAction::Tag { name, message } => {
                    let msg = message.as_deref().unwrap_or("");
                    handle
                        .infra
                        .git_tag(name, msg)
                        .map_err(|e| anyhow::anyhow!("failed to create tag: {}", e))?;
                    println!("Tagged '{}'.", name);
                    if !msg.is_empty() {
                        println!("  Message: {}", msg);
                    }
                    Ok(())
                }
            }
        }

        Some(Command::Budget { agent_id }) => {
            let handle = kernel.handle();
            match agent_id {
                Some(id) => {
                    let uuid = uuid::Uuid::parse_str(id)
                        .map_err(|e| anyhow::anyhow!("invalid agent id '{}': {}", id, e))?;
                    let budget = handle.agents.check_budget(&uuid);
                    println!("\n  Agent: {}", id);
                    println!("  {}", "─".repeat(40));
                    println!("  {:<22}  {}", "Tokens remaining:", budget.tokens_remaining);
                    println!("  {:<22}  {}", "Calls remaining:", budget.calls_remaining);
                    println!(
                        "  {:<22}  {} seconds",
                        "Window remaining:", budget.window_remaining_secs
                    );
                    println!(
                        "  {:<22}  {}",
                        "Status:",
                        if budget.is_exhausted {
                            "⚠ EXHAUSTED"
                        } else {
                            "✓ OK"
                        }
                    );
                    println!();
                    Ok(())
                }
                None => {
                    println!("\n  Agent Budget Overview");
                    println!("  {}", "─".repeat(48));
                    println!("  Run `oxios agent list` to find agent IDs,");
                    println!("  then `oxios budget <agent-id>` for details.");
                    println!();
                    Ok(())
                }
            }
        }

        Some(Command::Program { name }) => {
            let handle = kernel.handle();
            match handle.extensions.get_program(name).await {
                Some(program) => {
                    println!("\n  {} v{}", program.meta.name, program.meta.version);
                    println!("  {}", "─".repeat(50));
                    if !program.meta.description.is_empty() {
                        println!("  {}", program.meta.description);
                    }
                    if !program.skill_content.is_empty() {
                        println!("\n  SKILL.md:\n{}", program.skill_content);
                    }
                    if !program.meta.tools.is_empty() {
                        println!("\n  Tools:");
                        for tool in &program.meta.tools {
                            println!("    • {}: {}", tool.name, tool.description);
                        }
                    }
                    if !program.meta.host_requirements.required.is_empty() {
                        println!(
                            "\n  Required host tools: {}",
                            program.meta.host_requirements.required.join(", ")
                        );
                    }
                    if !program.meta.host_requirements.optional.is_empty() {
                        println!(
                            "  Optional host tools:   {}",
                            program.meta.host_requirements.optional.join(", ")
                        );
                    }
                    println!();
                    Ok(())
                }
                None => Err(anyhow::anyhow!(
                    "program '{}' not found. Install with `oxios pkg install`",
                    name
                )),
            }
        }

        // Handled before kernel assembly above — unreachable here
        Some(Command::Stop)
        | Some(Command::Daemon { .. })
        | Some(Command::Log { .. })
        | Some(Command::Config { .. }) => unreachable!(),
    }
}

// ─── Server mode (foreground) ────────────────────────────────────────────────

async fn cmd_serve(kernel: &Kernel, config_path: &Path) -> Result<()> {
    // Initialize MCP servers
    if let Err(e) = kernel.init_mcp_servers().await {
        tracing::warn!(error = %e, "Some MCP servers failed to initialize");
    }

    // Initialize default skills and programs
    let share_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("share");
    if let Err(e) = kernel.init_default_skills(&share_dir).await {
        tracing::warn!(error = %e, "Failed to initialize default skills");
    }
    if let Err(e) = kernel.init_default_programs(&share_dir).await {
        tracing::warn!(error = %e, "Failed to initialize default programs");
    }

    // Activate channels
    let channel_tasks = activate_channels(kernel, config_path).await?;

    // Start guardian
    kernel.start_guardian();

    let config = kernel.config();
    println!();
    println!("  ⬡ Oxios Agent OS  v{}", env!("CARGO_PKG_VERSION"));
    println!("  {}", "─".repeat(48));
    println!(
        "  Gateway:  http://{}:{}",
        config.gateway.host, config.gateway.port
    );
    println!();
    tracing::info!(
        "Oxios started on http://{}:{}",
        config.gateway.host,
        config.gateway.port
    );

    // Wait for ctrl+c
    tokio::signal::ctrl_c().await.ok();
    tracing::info!("Received shutdown signal, starting graceful shutdown...");

    for task in channel_tasks {
        task.abort();
    }

    let handle = kernel.handle();
    if let Ok(agents) = handle.agents.list().await {
        for agent in &agents {
            if let Err(e) = handle.agents.kill(&agent.id.to_string()).await {
                tracing::warn!(agent = %agent.id, error = %e, "Failed to kill agent");
            }
        }
        if !agents.is_empty() {
            tracing::info!(count = agents.len(), "Agents terminated");
        }
    }

    if let Err(e) = handle.mcp.shutdown_all().await {
        tracing::warn!(error = %e, "MCP shutdown error");
    }

    tracing::info!("Oxios shut down gracefully");
    Ok(())
}

// ─── Channel plugin helpers ───────────────────────────────────────────────

fn build_channel_plugins() -> Vec<Box<dyn ChannelPlugin>> {
    let plugins: Vec<Box<dyn ChannelPlugin>> = vec![];
    let mut plugins = plugins;
    #[cfg(feature = "web")]
    plugins.push(Box::new(WebPlugin::new()));
    #[cfg(feature = "cli")]
    plugins.push(Box::new(CliPlugin::new()));
    #[cfg(feature = "telegram")]
    plugins.push(Box::new(TelegramPlugin::new()));
    plugins
}

async fn activate_channels(
    kernel: &Kernel,
    config_path: &Path,
) -> Result<Vec<tokio::task::JoinHandle<()>>> {
    let plugins = build_channel_plugins();
    let plugin_map: std::collections::HashMap<&str, &dyn ChannelPlugin> =
        plugins.iter().map(|p| (p.name(), p.as_ref())).collect();

    let config = kernel.config();
    let mut all_tasks = Vec::new();

    for name in &config.channels.enabled {
        match plugin_map.get(name.as_str()) {
            Some(plugin) => {
                let ctx = ChannelContext {
                    kernel: kernel.handle(),
                    config: Arc::new(parking_lot::RwLock::new(config.clone())),
                    config_path: config_path.to_path_buf(),
                };
                match plugin.setup(ctx).await {
                    Ok(bundle) => {
                        tracing::info!(channel = %name, "Channel activated");
                        kernel.register_channel(bundle.channel).await;
                        all_tasks.extend(bundle.tasks);
                    }
                    Err(e) => {
                        tracing::error!(channel = %name, error = %e, "Failed to activate channel")
                    }
                }
            }
            None => tracing::warn!(
                channel = %name,
                "Channel '{}' not available (not compiled in). Available: {}",
                name,
                plugin_map.keys().cloned().collect::<Vec<_>>().join(", ")
            ),
        }
    }

    Ok(all_tasks)
}
