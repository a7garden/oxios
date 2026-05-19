//! Oxios Agent OS — main binary.
//!
//! Default invocation (`oxios`) starts the daemon in the background.
//! Use `oxios --foreground` to run in the foreground (for debugging).
//! First run without credentials triggers an interactive setup wizard.

mod cmd_run;
mod kernel;
mod otel;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use console::style;
use std::net::TcpStream;
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
    about = "Oxios Agent OS — Agent Operating System",
    after_help = "Examples:\n  oxios start                  Start the daemon\n  oxios onboard                Run the setup wizard\n  oxios run \"review this code\"  Execute a single prompt\n  oxios chat                   Start interactive chat\n  oxios status                 Show system status\n  oxios doctor                 Diagnose issues\n  oxios help                   Show all commands"
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
    /// Start the daemon (default when no command is given).
    #[command(visible_alias("serve"))]
    Start,

    /// Stop the running daemon.
    Stop,

    /// Restart the daemon.
    Restart,

    /// Run the interactive setup wizard.
    #[command(visible_alias("setup"))]
    Onboard,

    /// Reset all configuration and data (with confirmation).
    Reset {
        /// Skip confirmation prompt.
        #[arg(long)]
        yes: bool,
    },

    /// Show system status (daemon, credentials, agents).
    Status,

    /// Run a single prompt through the Ouroboros flow.
    #[command(arg_required_else_help = true)]
    Run {
        /// The prompt to execute.
        prompt: String,

        /// Output result as JSON (machine-readable).
        #[arg(long)]
        json: bool,

        /// Session ID for multi-turn conversation.
        /// Omit to start a new session.
        #[arg(long)]
        session: Option<String>,

        /// File to prepend as context to the prompt.
        /// Use `-` to read from stdin.
        #[arg(long)]
        context_file: Option<String>,

        /// Set exit code: 0 = evaluation passed, 1 = failed.
        #[arg(long)]
        exit_code: bool,
    },

    /// Start an interactive CLI chat session.
    Chat,

    /// Check system health and diagnose issues.
    Doctor,

    /// List available models for the configured (or specified) provider.
    Models {
        /// Provider to list models for (default: current provider).
        #[arg(short, long)]
        provider: Option<String>,
    },

    /// Backup Oxios state.
    Backup {
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Restore Oxios state from a backup.
    Restore { input: String },

    /// Show or modify configuration (default: show).
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
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

    /// Generate shell completion script.
    Completion { shell: Shell },
}

#[derive(Debug, Clone, Subcommand)]
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

/// Read the last `n` lines from a file without external commands.
fn tail_file(path: &Path, lines: usize) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let all_lines: Vec<&str> = content.lines().collect();
    let start = all_lines.len().saturating_sub(lines);
    Ok(all_lines[start..].join("\n"))
}

// ─── Subcommands ───────────────────────────────────────────────────────────

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
                "  {} '{}'",
                style("Installed").green().bold(),
                style(format!("{} v{}", program.meta.name, program.meta.version)).bold(),
            );
        }
        PkgAction::Uninstall { name } => {
            handle.extensions.uninstall_program(name).await?;
            println!("  {} '{}'", style("Uninstalled").green(), name);
        }
        PkgAction::List => {
            let programs = handle.extensions.list_programs().await;
            if programs.is_empty() {
                println!("  No programs installed.");
            } else {
                println!("{:30} {:10} {:40}", "NAME", "VERSION", "DESCRIPTION");
                println!("{}", "─".repeat(82));
                for p in &programs {
                    println!(
                        "{:30} {:10} {:40}",
                        p.meta.name, p.meta.version, p.meta.description
                    );
                }
            }
        }
        PkgAction::Search => {
            let programs = handle.extensions.list_programs().await;
            if programs.is_empty() {
                println!("  No programs installed.");
            } else {
                for p in &programs {
                    println!("{} ({})", style(&p.meta.name).bold(), p.meta.version);
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
            println!("  {} {} = {}", style("Set").green(), key, value);
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

    println!();
    println!(
        "  {} {}",
        style("⬡ Oxios Agent OS").bold(),
        style(format!("v{}", env!("CARGO_PKG_VERSION"))).dim()
    );
    println!("  {}", "─".repeat(48));
    println!("  {:<16}  {}", "Workspace:", config.kernel.workspace);
    println!(
        "  {:<16}  {}",
        "Model:",
        style(&config.engine.default_model).cyan()
    );

    let daemon_status = daemon.status();
    let is_running = matches!(daemon_status, oxios_kernel::DaemonStatus::Running { .. });
    if is_running {
        println!(
            "  {:<16}  {}",
            "Daemon:",
            style(daemon_status.to_string()).green()
        );
    } else {
        println!(
            "  {:<16}  {}",
            "Daemon:",
            style(daemon_status.to_string()).yellow()
        );
    }
    println!();

    // Credential source
    let provider = CredentialStore::provider_from_model(&config.engine.default_model);
    match provider {
        Some(provider) => match CredentialStore::resolve(provider, config.api_key().as_deref()) {
            Some((key, source)) => {
                let source_str = match source {
                    oxios_kernel::credential::CredentialSource::Config => "config.toml",
                    oxios_kernel::credential::CredentialSource::OxiAuthStore => "~/.oxi/auth.json",
                    oxios_kernel::credential::CredentialSource::EnvVar => "env var",
                };
                let preview = if key.len() > 8 {
                    format!("{}...{}", &key[..4], &key[key.len() - 4..])
                } else {
                    key.clone()
                };
                println!(
                    "  {:<16}  {} [{}]",
                    "Credentials:",
                    style(preview).green(),
                    style(source_str).dim()
                );
            }
            None => {
                println!(
                    "  {:<16}  {}",
                    "Credentials:",
                    style("✗ none (run `oxios onboard` to setup)").red()
                );
            }
        },
        None => {
            println!(
                "  {:<16}  {}",
                "Credentials:",
                style("✗ no model configured").red()
            );
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
            let status_str = format!("{:?}", agent.status);
            let styled_status = if matches!(agent.status, oxios_kernel::types::AgentStatus::Running)
            {
                style(&status_str).green()
            } else {
                style(&status_str).yellow()
            };
            println!(
                "    {}  {}  {}",
                style(&agent.id.to_string()).dim(),
                styled_status,
                agent.name
            );
        }
    }

    println!();
    Ok(())
}

// ─── Reset command ───────────────────────────────────────────────────────────

fn cmd_reset(oxios_home: &Path, skip_confirm: bool, pid_file: &Path) -> Result<()> {
    if !oxios_home.exists() {
        println!(
            "  {} does not exist — nothing to reset.",
            oxios_home.display()
        );
        return Ok(());
    }

    println!();
    println!(
        "  {}  This will delete all Oxios configuration and data:",
        style("⚠").yellow().bold()
    );
    println!("     {}", oxios_home.display());
    println!();

    if !skip_confirm {
        let confirm = inquire::Confirm::new("  Are you sure?")
            .with_default(false)
            .prompt()?;
        if !confirm {
            println!("  Reset cancelled.");
            return Ok(());
        }
    }

    // Stop daemon first if running
    if pid_file.exists() {
        let pid_str = std::fs::read_to_string(pid_file).unwrap_or_default();
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    std::fs::remove_dir_all(oxios_home)?;

    println!();
    println!(
        "  {} {} removed.",
        style("✓").green().bold(),
        oxios_home.display()
    );
    println!("  Run {} to set up again.", style("`oxios onboard`").cyan());
    println!();
    Ok(())
}

// ─── Doctor command ──────────────────────────────────────────────────────────

async fn cmd_doctor(kernel: &Kernel, config_path: &Path) -> Result<()> {
    let config = kernel.config();
    let mut issues = Vec::new();
    let mut checks = 0u32;

    println!();
    println!("  {}", style("⬡ Oxios Doctor — System Diagnostics").bold());
    println!("  {}", "─".repeat(48));

    // 1. Config file exists
    checks += 1;
    if config_path.exists() {
        println!(
            "  {} Config file present ({})",
            style("✓").green(),
            style(config_path.display()).dim()
        );
    } else {
        println!("  {} Config file missing", style("✗").red().bold());
        issues.push("Config file not found. Run `oxios onboard` to create it.".to_string());
    }

    // 2. Credentials
    checks += 1;
    let provider = CredentialStore::provider_from_model(&config.engine.default_model);
    match provider {
        Some(provider) => match CredentialStore::resolve(provider, config.api_key().as_deref()) {
            Some((key, source)) => {
                let source_str = match source {
                    oxios_kernel::credential::CredentialSource::Config => "config.toml",
                    oxios_kernel::credential::CredentialSource::OxiAuthStore => "~/.oxi/auth.json",
                    oxios_kernel::credential::CredentialSource::EnvVar => "env var",
                };
                let preview = if key.len() > 8 {
                    format!("{}...{}", &key[..4], &key[key.len() - 4..])
                } else {
                    "(set)".to_string()
                };
                println!(
                    "  {} Credentials found ({}, via {})",
                    style("✓").green(),
                    style(preview).cyan(),
                    style(source_str).dim()
                );
            }
            None => {
                println!(
                    "  {} No credentials for provider '{}'",
                    style("✗").red().bold(),
                    style(provider).cyan()
                );
                issues.push(format!(
                    "No API key for '{}'. Run `oxios onboard` to configure.",
                    provider
                ));
            }
        },
        None => {
            println!("  {} No model configured", style("✗").red().bold());
            issues.push("No model set. Run `oxios onboard` to configure.".to_string());
        }
    }

    // 3. Workspace directory
    checks += 1;
    let workspace = oxios_kernel::config::expand_home(&config.kernel.workspace);
    if workspace.exists() {
        println!(
            "  {} Workspace directory ({})",
            style("✓").green(),
            style(workspace.display()).dim()
        );
    } else {
        println!(
            "  {} Workspace directory missing ({})",
            style("✗").red().bold(),
            workspace.display()
        );
        issues.push("Workspace directory not found. It will be created on first run.".to_string());
    }

    // 4. Daemon status
    checks += 1;
    let daemon = DaemonManager::new(&config.daemon.pid_file, &config.daemon.log_dir);
    let daemon_status = daemon.status();
    let is_running = matches!(daemon_status, oxios_kernel::DaemonStatus::Running { .. });
    if is_running {
        println!("  {} Daemon is running", style("✓").green());
    } else {
        println!(
            "  {} Daemon is not running ({})",
            style("⚠").yellow().bold(),
            daemon_status
        );
        issues.push("Daemon not running. Start with `oxios start`.".to_string());
    }

    // 5. MCP servers
    checks += 1;
    let mcp_count = kernel.handle().mcp.server_count();
    if mcp_count > 0 {
        println!(
            "  {} {} MCP server(s) connected",
            style("✓").green(),
            mcp_count
        );
    } else {
        println!("  {} No MCP servers configured", style("⚠").yellow().bold());
    }

    // 6. Model is set
    checks += 1;
    if !config.engine.default_model.is_empty() {
        println!(
            "  {} Default model: {}",
            style("✓").green(),
            style(&config.engine.default_model).cyan()
        );
    } else {
        println!("  {} No default model set", style("✗").red().bold());
        issues.push("No default model configured.".to_string());
    }

    // 7. oxi CLI installed
    checks += 1;
    let oxi_auth_exists = {
        let home = std::env::var("HOME").unwrap_or_default();
        std::path::PathBuf::from(format!("{}/.oxi/auth.json", home)).exists()
    };
    let oxi_bin_exists = std::path::PathBuf::from("/usr/local/bin/oxi").exists()
        || std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default() + "/.cargo/bin/oxi")
            .exists();
    let oxi_installed = oxi_auth_exists || oxi_bin_exists;
    if oxi_installed {
        println!(
            "  {} oxi CLI available (shared auth store)",
            style("✓").green()
        );
    } else {
        println!("  {} oxi CLI not detected", style("⚠").yellow().bold());
        issues.push(
            "Install oxi CLI for shared credential management: `cargo install oxi-cli`".to_string(),
        );
    }

    // 8. Gateway port available
    checks += 1;
    let port = config.gateway.port;
    let port_in_use = TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok();
    if port_in_use && !is_running {
        println!(
            "  {} Port {} is already in use",
            style("✗").red().bold(),
            style(port).cyan()
        );
        issues.push(format!(
            "Port {} is occupied. Change with `oxios config set gateway.port <port>`.",
            port
        ));
    } else if port_in_use && is_running {
        println!(
            "  {} Port {} listening (daemon active)",
            style("✓").green(),
            style(port).cyan()
        );
    } else {
        println!(
            "  {} Port {} available",
            style("✓").green(),
            style(port).cyan()
        );
    }

    // Summary
    println!("  {}", "─".repeat(48));
    if issues.is_empty() {
        println!(
            "  {} checks passed, no issues found. {}",
            checks,
            style("All good!").green().bold()
        );
    } else {
        println!(
            "  {} checks, {} issue(s):",
            checks,
            style(issues.len()).yellow().bold()
        );
        println!();
        for (i, issue) in issues.iter().enumerate() {
            println!("    {}. {}", i + 1, issue);
        }
    }
    println!();

    Ok(())
}

// ─── Models command ──────────────────────────────────────────────────────────

fn cmd_models(provider: Option<&str>) -> Result<()> {
    // Resolve provider from arg or from config
    let provider_id = match provider {
        Some(p) => p.to_string(),
        None => {
            // Try reading from config
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            let config_path =
                oxios_kernel::config::expand_home(&format!("{}/.oxios/config.toml", home));
            if config_path.exists() {
                let config = oxios_kernel::config::load_config(&config_path)?;
                if config.engine.default_model.is_empty() {
                    anyhow::bail!(
                        "No provider configured. Run `oxios onboard` or use `--provider <name>`."
                    );
                }
                CredentialStore::provider_from_model(&config.engine.default_model)
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            } else {
                anyhow::bail!("No config found. Run `oxios onboard` or use `--provider <name>`.");
            }
        }
    };

    if provider_id.is_empty() {
        anyhow::bail!("Could not determine provider. Use `--provider <name>`.");
    }

    let models = oxi_sdk::get_provider_models(&provider_id);
    if models.is_empty() {
        println!(
            "  No models found for '{}'. Check the provider name.",
            provider_id
        );
        return Ok(());
    }

    println!();
    println!(
        "  {} for {}",
        style("Available Models").bold(),
        style(&provider_id).cyan()
    );
    println!("  {}", "─".repeat(60));

    for entry in models.iter() {
        let ctx = if entry.context_window >= 1_000_000 {
            format!("{}M", entry.context_window / 1_000_000)
        } else {
            format!("{}K", entry.context_window / 1000)
        };
        let reasoning = if entry.reasoning {
            format!(" {}", style("✦reasoning").magenta())
        } else {
            String::new()
        };
        println!(
            "  {}  {} ctx{}",
            style(&entry.name).bold(),
            style(ctx).dim(),
            reasoning,
        );
    }

    println!();
    println!(
        "  {} models total. Use full ID: {}/<model-id>",
        models.len(),
        provider_id
    );
    println!();
    Ok(())
}

// ─── Entry point ─────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!();
        eprintln!("  {} {}", style("error:").red().bold(), e);
        eprintln!(
            "  Run {} for diagnostics.\n",
            style("`oxios doctor`").cyan()
        );
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
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
        } else if let Some(ref level) = config.logging.level {
            tracing_subscriber::EnvFilter::new(level)
        } else {
            tracing_subscriber::EnvFilter::new("info")
        }
    });

    match config.logging.format.as_str() {
        "json" => {
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(env_filter)
                .with_target(true)
                .with_writer(non_blocking)
                .init();
        }
        "compact" => {
            tracing_subscriber::fmt()
                .compact()
                .with_env_filter(env_filter)
                .with_target(true)
                .with_writer(non_blocking)
                .init();
        }
        _ => {
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_target(true)
                .with_thread_ids(false)
                .with_file(false)
                .with_line_number(false)
                .compact()
                .with_writer(non_blocking)
                .init();
        }
    }

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
                println!("  No log file at {}", log_file.display());
                return Ok(());
            }
            print!("{}", tail_file(&log_file, *lines)?);
            return Ok(());
        }
        Some(Command::Config { action }) => {
            let action = action.clone().unwrap_or(ConfigAction::Show);
            return cmd_config(&action, &config_path).await;
        }
        Some(Command::Onboard) => {
            let completed = oxios_kernel::onboarding::run_onboarding(&oxios_home, &mut config)?;
            if !completed {
                println!("  Onboarding skipped or cancelled.");
            }
            return Ok(());
        }
        Some(Command::Reset { yes }) => {
            let pid_file = oxios_kernel::config::expand_home(&config.daemon.pid_file);
            return cmd_reset(&oxios_home, *yes, &pid_file);
        }
        Some(Command::Models { provider }) => {
            return cmd_models(provider.as_deref());
        }
        Some(Command::Completion { shell }) => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            generate(*shell, &mut cmd, name, &mut std::io::stdout());
            return Ok(());
        }
        _ => {}
    }

    // ── Onboarding gate ──
    // Commands that need the kernel assembled (and therefore credentials).
    let needs_kernel = matches!(
        cli.command.as_ref(),
        None | Some(Command::Start)
            | Some(Command::Run { .. })
            | Some(Command::Chat)
            | Some(Command::Status)
            | Some(Command::Doctor)
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

            // ── Onboarding → start flow ──
            let start_now = inquire::Confirm::new("  Start daemon now?")
                .with_default(true)
                .prompt()?;
            if !start_now {
                println!("  Run {} when ready.", style("`oxios start`").cyan());
                return Ok(());
            }
            // fall through to kernel assembly → daemon start
        } else {
            return Ok(());
        }
    }

    // ── Kernel assembly ──
    let term = console::Term::stderr();
    let _ = term.write_str(&format!("  {} Starting Oxios...\r", style("⠋").cyan()));
    let _ = term.flush();

    let kernel = Kernel::builder()
        .config_path(config_path.clone())
        .build()
        .await?;

    let _ = term.clear_line();

    // ── Dispatch subcommands ──
    match cli.command.as_ref() {
        // Default / start: launch daemon
        None | Some(Command::Start) => {
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

        Some(Command::Run {
            prompt,
            json,
            session,
            context_file,
            exit_code,
        }) => {
            let opts = cmd_run::RunOptions {
                json: *json,
                session_id: session.clone(),
                context_file: context_file.clone(),
                exit_code: *exit_code,
            };
            let code = cmd_run::cmd_run(&kernel, prompt, &opts).await?;
            std::process::exit(code);
        }

        Some(Command::Status) => cmd_status(&kernel).await,

        Some(Command::Doctor) => cmd_doctor(&kernel, &config_path).await,

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
                        println!("  No active agents.");
                    } else {
                        println!("{:36} {:10} {:20} CREATED", "ID", "STATUS", "NAME");
                        println!("{}", "─".repeat(90));
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
                    println!(
                        "  {} Agent {} terminated.",
                        style("✓").green(),
                        style(id).cyan()
                    );
                    Ok(())
                }
            }
        }

        Some(Command::Audit) => {
            let handle = kernel.handle();
            match handle.security.verify_chain() {
                Ok(_) => println!(
                    "  {} Audit trail verified — chain intact.",
                    style("✓").green().bold()
                ),
                Err(e) => {
                    eprintln!(
                        "  {} Audit verification failed: {:?}",
                        style("✗").red().bold(),
                        e
                    );
                    println!("  Some entries may have been tampered with.");
                }
            }
            let entries = handle.security.query_audit(0, 20);
            println!();
            if entries.is_empty() {
                println!("  No audit entries yet.");
            } else {
                println!("  Recent Audit Entries (showing last {}):", entries.len());
                println!("{:10} {:20} {:15} ACTION", "SEQ", "TIMESTAMP", "ACTOR");
                println!("{}", "─".repeat(70));
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
            println!("\n  Total entries: {}", handle.security.audit_count());
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
                        println!("  No commits yet.");
                    } else {
                        println!("{:8} {:20} {:40}", "HASH", "AUTHOR", "MESSAGE");
                        println!("{}", "─".repeat(75));
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
                    println!("  {} '{}'.", style("Tagged").green(), style(name).cyan());
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
                            style("⚠ EXHAUSTED").yellow().bold().to_string()
                        } else {
                            style("✓ OK").green().to_string()
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
                    println!(
                        "\n  {} {}",
                        style(&program.meta.name).bold(),
                        style(format!("v{}", program.meta.version)).dim()
                    );
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
                            println!(
                                "    {} {}: {}",
                                style("•").dim(),
                                tool.name,
                                tool.description
                            );
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
        | Some(Command::Config { .. })
        | Some(Command::Onboard)
        | Some(Command::Reset { .. })
        | Some(Command::Models { .. })
        | Some(Command::Completion { .. }) => unreachable!(),
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

    // Run gateway event loop on a dedicated thread (parking_lot guards are not Send,
    // so we cannot use tokio::spawn which may move futures between threads).
    // The gateway polls channels and routes messages to the kernel.
    let gateway = kernel.gateway();
    let gateway_shutdown = {
        // Signal the gateway to stop before joining its thread.
        let gw = Arc::clone(&gateway);
        move || {
            gw.signal_shutdown();
        }
    };
    let gateway_handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("gateway thread runtime");
        rt.block_on(gateway.run()).expect("gateway run error");
    });

    let config = kernel.config();
    println!();
    println!(
        "  {} {}",
        style("⬡ Oxios Agent OS").bold(),
        style(format!("v{}", env!("CARGO_PKG_VERSION"))).dim()
    );
    println!("  {}", "─".repeat(48));
    println!(
        "  Gateway:  {}",
        style(format!(
            "http://{}:{}",
            config.gateway.host, config.gateway.port
        ))
        .cyan()
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

    // Phase 1: Signal gateway to stop accepting new messages
    gateway_shutdown();

    // Phase 2: Cancel channel tasks
    for task in channel_tasks {
        task.abort();
    }

    // Phase 3: Wait for gateway thread with timeout
    let gateway_result = gateway_handle.join();
    match gateway_result {
        Ok(()) => tracing::info!("Gateway stopped cleanly"),
        Err(e) => tracing::warn!(error = ?e, "Gateway thread panicked"),
    }

    // Phase 4: Terminate running agents (parallel)
    let handle = kernel.handle();
    if let Ok(agents) = handle.agents.list().await {
        if !agents.is_empty() {
            tracing::info!(count = agents.len(), "Terminating agents...");
            let mut kill_futures = Vec::new();
            for agent in &agents {
                let agent_id = agent.id.to_string();
                let h = handle.clone();
                kill_futures.push(tokio::spawn(async move {
                    if let Err(e) = h.agents.kill(&agent_id).await {
                        tracing::warn!(agent = %agent_id, error = %e, "Failed to kill agent");
                    }
                }));
            }
            for f in kill_futures {
                let _ = f.await;
            }
            tracing::info!(count = agents.len(), "Agents terminated");
        }
    }

    if let Err(e) = handle.mcp.shutdown_all().await {
        tracing::warn!(error = %e, "MCP shutdown error");
    }

    // Flush audit trail to disk before exit
    if let Err(e) = kernel.flush_audit() {
        tracing::warn!(error = %e, "Audit trail flush error");
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
