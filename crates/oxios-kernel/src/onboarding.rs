//! Interactive first-run setup wizard.
//!
//! Inspired by @clack/prompts (OpenClaw, Vercel CLI, SvelteKit):
//!   - intro() / outro() bookends
//!   - spinner for async work
//!   - note() for information boxes
//!   - one question per screen
//!
//! Flow:
//!   Welcome → Provider (auto-detect) → API Key (auto-detect) → Model →
//!   Embedding download → Summary → Done

use crate::config::OxiosConfig;
use crate::credential::CredentialStore;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::{Confirm, CustomType, Select, Text};
use std::io::{self, IsTerminal};
use std::path::Path;

// ── Constants ───────────────────────────────────────────────────────────────

/// Subdirectories to create under the Oxios home directory during setup.
pub const WORKSPACE_SUBDIRS: &[&str] = &[
    "workspace",
    "workspace/memory",
    "workspace/memory/knowledge",
    "workspace/seeds",
    "workspace/sessions",
    "workspace/skills",
];

const NO_KEY_PROVIDERS: &[&str] = &[];

const HIDDEN_PROVIDERS: &[&str] = &[
    "amazon-bedrock",
    "azure-openai-responses",
    "cloudflare-ai-gateway",
    "cloudflare-workers-ai",
    "google-vertex",
    "minimax-cn",
    "moonshotai-cn",
    "openai-codex",
    "opencode-go",
    "vercel-ai-gateway",
    "xiaomi",
];

// ── Theme (clack-inspired) ──────────────────────────────────────────────────

mod theme {
    #![allow(dead_code)]
    use console::style;
    use std::fmt::Display;

    pub fn accent<T: Display>(s: T) -> console::StyledObject<T> {
        style(s).cyan()
    }

    pub fn success<T: Display>(s: T) -> console::StyledObject<T> {
        style(s).green()
    }

    pub fn warn<T: Display>(s: T) -> console::StyledObject<T> {
        style(s).yellow()
    }

    pub fn dim<T: Display>(s: T) -> console::StyledObject<T> {
        style(s).dim()
    }

    pub fn bold<T: Display>(s: T) -> console::StyledObject<T> {
        style(s).bold()
    }

    pub fn muted<T: Display>(s: T) -> console::StyledObject<T> {
        style(s).dim()
    }

    /// Step heading: "  ◇ Provider"
    pub fn step(name: &str) -> String {
        format!("  {} {}", style("◇").cyan(), style(name).bold())
    }

    /// Spinner frame character.
    pub fn spinner_frame() -> &'static str {
        "◯"
    }

    /// Success mark: ✓
    pub fn ok() -> &'static str {
        "✓"
    }

    /// Fail mark: ✗
    pub fn fail() -> &'static str {
        "✗"
    }
}

// ── Display helpers ─────────────────────────────────────────────────────────

#[derive(Clone)]
struct ProviderEntry {
    id: String,
    display: String,
    has_env_key: bool,
}

impl std::fmt::Display for ProviderEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display)
    }
}

#[derive(Clone)]
struct ModelEntry {
    full_id: String,
    display: String,
}

impl std::fmt::Display for ModelEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display)
    }
}

const MANUAL_MODEL_DISPLAY: &str = "✎  Enter model ID manually";

// ── Public API ──────────────────────────────────────────────────────────────

/// Check if the system is fully configured (model + credentials).
pub fn has_credentials(config: &OxiosConfig) -> bool {
    let Some(provider) = CredentialStore::provider_from_model(&config.engine.default_model) else {
        return false;
    };
    CredentialStore::has_credential(provider, config.api_key().as_deref())
}

/// Check if stdin is an interactive terminal.
pub fn is_interactive() -> bool {
    io::stdin().is_terminal()
}

/// Result of onboarding.
pub struct OnboardingResult {
    /// Config was written successfully.
    pub configured: bool,
    /// User chose to skip (cancelled / non-interactive).
    pub skipped: bool,
}

/// Run the first-time setup wizard.
pub fn run_onboarding(
    oxios_home: &Path,
    config: &mut OxiosConfig,
    is_first_run: bool,
) -> anyhow::Result<OnboardingResult> {
    // ── Already configured? ──
    if !config.engine.default_model.is_empty() {
        if let Some(provider_id) =
            CredentialStore::provider_from_model(&config.engine.default_model)
        {
            if CredentialStore::has_credential(provider_id, config.api_key().as_deref()) {
                println!();
                println!(
                    "  {} {}",
                    style("✓").green(),
                    style(&config.engine.default_model).cyan(),
                );

                let ans = Select::new(
                    "  What next?",
                    vec!["Keep current configuration", "Reconfigure"],
                )
                .with_starting_cursor(0)
                .prompt()?;

                if ans == "Keep current configuration" {
                    return Ok(OnboardingResult {
                        configured: true,
                        skipped: false,
                    });
                }
            }
        }
    }

    // ── Non-interactive bail ──
    if !is_interactive() {
        println!();
        println!(
            "  {} Setup requires a terminal. Run {} interactively.",
            style("!").yellow(),
            style("oxios").cyan(),
        );
        println!();
        return Ok(OnboardingResult {
            configured: false,
            skipped: true,
        });
    }

    // ── intro ──
    print_intro(is_first_run);

    // ── Auto-detect ──
    let env_providers = oxi_sdk::get_all_env_keys();
    if !env_providers.is_empty() {
        let detected = env_providers
            .keys()
            .find(|p| !oxi_sdk::get_provider_models(p).is_empty());

        if let Some(provider) = detected {
            let keys = oxi_sdk::find_env_keys(provider);
            let var_name = keys.and_then(|k| k.first().copied()).unwrap_or(provider);
            println!(
                "  {} {} {}",
                theme::accent("◇"),
                theme::dim(format!("Found {var_name} →")),
                theme::accent(provider),
            );
            let use_it = Confirm::new("  Use this provider?")
                .with_default(true)
                .prompt()?;
            if use_it {
                return run_provider_flow(oxios_home, config, provider);
            }
        }
    }

    // ── Manual provider selection ──
    let all_providers = oxi_sdk::get_providers();
    let visible: Vec<&str> = all_providers
        .iter()
        .copied()
        .filter(|p| !HIDDEN_PROVIDERS.contains(p))
        .collect();

    let provider = prompt_provider(&visible)?;
    run_provider_flow(oxios_home, config, provider)
}

// ── Provider flow ───────────────────────────────────────────────────────────

fn run_provider_flow(
    oxios_home: &Path,
    config: &mut OxiosConfig,
    provider: &str,
) -> anyhow::Result<OnboardingResult> {
    // ── API Key ──
    let (api_key, key_source) = resolve_api_key(provider)?;

    // ── Model ──
    let model = prompt_model(provider)?;

    // ── Save config (needed for embedding download path) ──
    with_spinner("Saving configuration...", "Configuration saved", || {
        persist_config(
            oxios_home,
            config,
            provider,
            api_key.as_deref().unwrap_or(""),
            &model,
        )
    })?;

    // ── Embedding model ──
    let embed_status = setup_embedding(config)?;

    // ── Summary + outro ──
    print_summary(oxios_home, provider, &model, key_source, &embed_status);

    Ok(OnboardingResult {
        configured: true,
        skipped: false,
    })
}

// ── Step: API Key ────────────────────────────────────────────────────────────

fn resolve_api_key(provider: &str) -> anyhow::Result<(Option<String>, &'static str)> {
    if NO_KEY_PROVIDERS.contains(&provider) {
        return Ok((None, "none"));
    }

    // Try auth.json
    if let Ok(Some(token)) = oxi_sdk::load_token(provider) {
        if !token.access_token.is_empty() {
            println!();
            println!(
                "  {} Credentials found in {}",
                theme::step("API Key"),
                theme::dim("~/.oxi/auth.json"),
            );
            let use_it = Confirm::new("  Use them?").with_default(true).prompt()?;
            if use_it {
                return Ok((None, "auth.json"));
            }
        }
    }

    // Try env var
    if let Some(env_key) = oxi_sdk::get_env_api_key(provider) {
        println!();
        println!(
            "  {} {}",
            theme::step("API Key"),
            theme::dim("Using key from environment"),
        );
        return Ok((Some(env_key), "env"));
    }

    // Manual entry
    println!();
    println!("  {}", theme::step("API Key"));
    println!("  {}", theme::dim("Stored locally, never shared."),);

    let key = CustomType::<String>::new("  →")
        .with_placeholder("sk-...")
        .with_error_message("API key is required")
        .prompt()?;
    Ok((Some(key), "manual"))
}

// ── Step: Provider ───────────────────────────────────────────────────────────

fn prompt_provider<'a>(providers: &[&'a str]) -> anyhow::Result<&'a str> {
    let mut entries: Vec<ProviderEntry> = providers
        .iter()
        .map(|&p| {
            let model_count = oxi_sdk::get_provider_models(p).len();
            let has_env = oxi_sdk::has_env_key(p);
            let mut badges = vec![format!("{} models", model_count)];
            if has_env {
                badges.push("🔑 detected".into());
            }
            ProviderEntry {
                id: p.to_string(),
                display: format!(
                    "  {}  {}",
                    style(p).bold(),
                    theme::muted(badges.join(" · ")),
                ),
                has_env_key: has_env,
            }
        })
        .collect();

    // Sort: providers with detected env keys first
    entries.sort_by_key(|b| std::cmp::Reverse(b.has_env_key));

    println!();
    println!("  {}", theme::step("Provider"));
    println!("  {}", theme::dim("Which cloud hosts your LLM?"),);

    let selected = Select::new("  →", entries)
        .with_starting_cursor(0)
        .prompt()?;

    Ok(providers.iter().find(|&&p| p == selected.id).unwrap())
}

// ── Step: Model ──────────────────────────────────────────────────────────────

fn prompt_model(provider: &str) -> anyhow::Result<String> {
    let models = oxi_sdk::get_provider_models(provider);

    println!();
    println!("  {}", theme::step("Model"));

    if models.is_empty() {
        let model = Text::new("  → Model ID:").prompt()?;
        if model.is_empty() {
            anyhow::bail!("Model ID is required.");
        }
        return Ok(if model.contains('/') {
            model
        } else {
            format!("{provider}/{model}")
        });
    }

    let mut entries: Vec<ModelEntry> = Vec::new();
    for entry in models.iter() {
        if entry.name.contains("latest") {
            continue;
        }
        let full_id = format!("{}/{}", provider, entry.id);
        let ctx = if entry.context_window >= 1_000_000 {
            format!("{}M", entry.context_window / 1_000_000)
        } else {
            format!("{}K", entry.context_window / 1000)
        };
        let reasoning = if entry.reasoning {
            format!(" {}", style("reasoning").magenta())
        } else {
            String::new()
        };
        entries.push(ModelEntry {
            full_id,
            display: format!(
                "  {}  {}{}",
                style(&entry.name).bold(),
                theme::muted(format!("{ctx} ctx")),
                reasoning,
            ),
        });
        if entries.len() >= 12 {
            break;
        }
    }

    entries.push(ModelEntry {
        full_id: String::new(),
        display: format!("  {MANUAL_MODEL_DISPLAY}"),
    });

    let selected = Select::new("  →", entries)
        .with_starting_cursor(0)
        .prompt()?;

    if selected.display.contains(MANUAL_MODEL_DISPLAY) {
        let manual = Text::new("  → Model ID:").prompt()?;
        if manual.is_empty() {
            anyhow::bail!("Model ID cannot be empty.");
        }
        return Ok(if manual.contains('/') {
            manual
        } else {
            format!("{provider}/{manual}")
        });
    }

    Ok(selected.full_id.clone())
}

// ── Step: Embedding model ────────────────────────────────────────────────────

fn setup_embedding(config: &OxiosConfig) -> anyhow::Result<String> {
    let workspace = crate::config::expand_home(&config.kernel.workspace);

    #[cfg(feature = "embedding-gguf")]
    {
        let model_dir = oxios_memory::GgufModelLoader::model_dir_for_workspace(&workspace);

        if oxios_memory::GgufModelLoader::is_model_cached(&model_dir) {
            return Ok("cached".to_string());
        }

        let display_name = oxios_memory::MODEL_DISPLAY_NAME;
        let size_mb = oxios_memory::MODEL_SIZE_MB;

        println!();
        println!(
            "  {} {} model (~{} MB)",
            theme::step("Embedding"),
            display_name,
            size_mb,
        );
        println!(
            "  {}",
            theme::dim("For semantic memory search. One-time download."),
        );

        let result = with_spinner(
            &format!("Downloading {}...", display_name),
            &format!("{} Downloaded", theme::success(theme::ok()).to_string()),
            || oxios_memory::GgufModelLoader::ensure_model(&model_dir),
        );

        match result {
            Ok(path) => {
                let size_mb = path.metadata().map(|m| m.len() / 1_000_000).unwrap_or(0);
                println!(
                    "  {} {} MB",
                    theme::success(theme::ok()),
                    theme::accent(size_mb),
                );
                Ok("downloaded".to_string())
            }
            Err(e) => {
                println!("  {} {}", theme::warn(theme::fail()), e,);
                println!("  {} Will retry on first search.", theme::accent("→"),);
                Ok("failed".to_string())
            }
        }
    }

    #[cfg(not(feature = "embedding-gguf"))]
    {
        let _ = (config, workspace);
        Ok("tfidf".to_string())
    }
}

// ── Spinner helper ───────────────────────────────────────────────────────────

/// Run a closure with a spinner. Shows `message` while running,
/// replaces with `done` on success.
fn with_spinner<T, F>(message: &str, done: &str, f: F) -> T
where
    F: FnOnce() -> T,
{
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
            .template("  {spinner} {msg}")
            .unwrap(),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));

    let result = f();

    pb.finish_with_message(done.to_string());
    result
}

// ── Persistence ──────────────────────────────────────────────────────────────

fn persist_config(
    oxios_home: &Path,
    config: &mut OxiosConfig,
    provider: &str,
    api_key: &str,
    model: &str,
) -> anyhow::Result<()> {
    if !api_key.is_empty() {
        CredentialStore::store(provider, api_key)?;
    }

    let workspace = crate::config::expand_home(&config.kernel.workspace);
    std::fs::create_dir_all(&workspace)?;
    for subdir in WORKSPACE_SUBDIRS {
        std::fs::create_dir_all(Path::new(&workspace).join(subdir))?;
    }

    config.engine.default_model = model.to_string();

    std::fs::create_dir_all(oxios_home)?;
    let toml_str = toml::to_string_pretty(config)
        .map_err(|e| anyhow::anyhow!("Failed to serialize config: {e}"))?;
    std::fs::write(oxios_home.join("config.toml"), &toml_str)?;

    Ok(())
}

// ── UI: intro / outro ───────────────────────────────────────────────────────

fn print_intro(is_first_run: bool) {
    println!();

    if is_first_run {
        println!("  {}", style("⬡ Oxios Agent OS").bold().cyan(),);
        println!("  {}", theme::dim("Your AI agents, organized."),);
        println!();
        println!("  Let's get you set up. About 30 seconds.");
    } else {
        println!("  {}", style("⬡ Oxios Setup").bold());
    }

    println!(
        "  {}",
        theme::dim("↑↓ navigate · Enter confirm · Ctrl+C skip"),
    );
    println!();
}

fn print_summary(
    oxios_home: &Path,
    provider: &str,
    model: &str,
    key_source: &str,
    embed_status: &str,
) {
    println!();
    println!(
        "  {}",
        theme::dim("─────────────────────────────────────────")
    );

    println!("  {:<14} {}", theme::dim("LLM:"), theme::accent(model),);
    println!(
        "  {:<14} {}",
        theme::dim("Provider:"),
        theme::muted(provider),
    );
    println!("  {:<14} {}", theme::dim("Key:"), theme::muted(key_source),);

    let embed_label = match embed_status {
        "cached" | "downloaded" => {
            #[cfg(feature = "embedding-gguf")]
            {
                let name = oxios_memory::MODEL_DISPLAY_NAME;
                Some(if embed_status == "downloaded" {
                    format!("{} ✓", name)
                } else {
                    format!("{} ✓ (cached)", name)
                })
            }
            #[cfg(not(feature = "embedding-gguf"))]
            {
                None
            }
        }
        "failed" => Some("will download on first search".to_string()),
        _ => None,
    };

    if let Some(ref label) = embed_label {
        let styled = if embed_status == "failed" {
            theme::warn(label).to_string()
        } else {
            theme::accent(label).to_string()
        };
        println!("  {:<14} {}", theme::dim("Embedding:"), styled);
    }

    println!(
        "  {:<14} {}",
        theme::dim("Home:"),
        theme::muted(oxios_home.display()),
    );

    println!(
        "  {}",
        theme::dim("─────────────────────────────────────────")
    );
    println!();
}
