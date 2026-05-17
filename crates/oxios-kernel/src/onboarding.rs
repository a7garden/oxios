//! Interactive first-run setup wizard.
//!
//! Uses oxi-sdk's provider/model catalog and env key detection.
//! No hardcoded provider lists — everything comes from oxi-ai.
//!
//! UI powered by `inquire` — arrow-key navigation for all selections.

use crate::config::OxiosConfig;
use crate::credential::CredentialStore;
use inquire::{Confirm, CustomType, Select, Text};
use std::io::{self, IsTerminal};
use std::path::Path;

// ── Constants ───────────────────────────────────────────────────────────────

const TOTAL_STEPS: usize = 5;

const WORKSPACE_SUBDIRS: &[&str] = &[
    "workspace",
    "workspace/memory",
    "workspace/memory/knowledge",
    "workspace/seeds",
    "workspace/sessions",
    "workspace/skills",
    "workspace/programs",
];

/// Providers that don't need an API key (e.g., local models).
const NO_KEY_PROVIDERS: &[&str] = &[];

/// Providers to exclude from the interactive list (cloud gateways, regional variants).
/// Users who need these can set them up via config.toml directly.
const HIDDEN_PROVIDERS: &[&str] = &[
    "amazon-bedrock",         // requires AWS setup, not a simple API key
    "azure-openai-responses", // requires Azure deployment
    "cloudflare-ai-gateway",
    "cloudflare-workers-ai",
    "google-vertex", // requires ADC, not a simple API key
    "minimax-cn",
    "moonshotai-cn",
    "openai-codex", // subset of openai
    "opencode-go",
    "vercel-ai-gateway",
    "xiaomi",
];

// ── Helpers for formatted display items ─────────────────────────────────────

/// A provider entry in the selection list.
#[derive(Clone)]
struct ProviderEntry {
    id: String,
    display: String,
}

impl std::fmt::Display for ProviderEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display)
    }
}

/// A model entry in the selection list.
#[derive(Clone)]
struct ModelEntry {
    /// Full model ID: "provider/model-id"
    full_id: String,
    /// Display label
    display: String,
}

impl std::fmt::Display for ModelEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display)
    }
}

/// Manual model entry sentinel.
const MANUAL_MODEL_DISPLAY: &str = "✎ Enter model ID manually...";

// ── Public API ──────────────────────────────────────────────────────────────

/// Check if the system is fully configured (model + credentials).
/// Returns `true` when onboarding should be skipped.
pub fn has_credentials(config: &OxiosConfig) -> bool {
    let Some(provider) = CredentialStore::provider_from_model(&config.engine.default_model)
    else {
        return false;
    };
    CredentialStore::has_credential(provider, config.api_key().as_deref())
}

/// Check if stdin is an interactive terminal.
pub fn is_interactive() -> bool {
    io::stdin().is_terminal()
}

/// Run the first-time setup wizard.
///
/// Returns `Ok(true)` — onboarding completed, config written.
/// Returns `Ok(false)` — skipped (already configured, non-interactive, or cancelled).
pub fn run_onboarding(oxios_home: &Path, config: &mut OxiosConfig) -> anyhow::Result<bool> {
    // ── Re-run detection ──
    if !config.engine.default_model.is_empty() {
        if let Some(provider_id) =
            CredentialStore::provider_from_model(&config.engine.default_model)
        {
            if CredentialStore::has_credential(provider_id, config.api_key().as_deref()) {
                println!();
                println!(
                    "  Already configured as '{}'.",
                    config.engine.default_model
                );

                let ans = Select::new(
                    "  What would you like to do?",
                    vec![
                        "Keep current configuration",
                        "Modify (re-run wizard)",
                        "Reset (clear everything)",
                    ],
                )
                .with_starting_cursor(0)
                .prompt()?;

                match ans {
                    "Keep current configuration" => {
                        return Ok(false);
                    }
                    "Reset (clear everything)" => { /* fall through to wizard */ }
                    _ => { /* modify — fall through */ }
                }
            }
        }
    }

    // ── Need a terminal for interactive input ──
    if !is_interactive() {
        println!();
        println!("  Oxios requires initial setup but is not running in a terminal.");
        println!("  Please run `oxios` in an interactive shell.");
        println!();
        return Ok(false);
    }

    print_banner();

    // ── Step 0 [auto]: Check for env vars and existing auth tokens ──
    let env_providers = oxi_sdk::get_all_env_keys();
    if !env_providers.is_empty() {
        // Find the first provider that also has models in the registry
        let detected = env_providers
            .keys()
            .find(|p| !oxi_sdk::get_provider_models(p).is_empty());

        if let Some(provider) = detected {
            println!();
            let keys = oxi_sdk::find_env_keys(provider);
            let var_name = keys
                .and_then(|k| k.first().copied())
                .unwrap_or(provider);
            println!(
                "  Detected {} in environment for '{}'.",
                var_name, provider
            );
            let use_it = Confirm::new("  Use this provider?")
                .with_default(true)
                .prompt()?;
            if use_it {
                return finish_with_provider(oxios_home, config, provider);
            }
        }
    }

    // ── Step 1: Provider selection from oxi model DB ──
    let all_providers = oxi_sdk::get_providers();
    let visible: Vec<&str> = all_providers
        .iter()
        .copied()
        .filter(|p| !HIDDEN_PROVIDERS.contains(p))
        .collect();

    let provider = prompt_provider(&visible)?;
    finish_with_provider(oxios_home, config, provider)
}

// ── Core flow for a chosen provider ─────────────────────────────────────────

/// Complete onboarding for a given provider: key → model → workspace → write.
fn finish_with_provider(
    oxios_home: &Path,
    config: &mut OxiosConfig,
    provider: &str,
) -> anyhow::Result<bool> {
    let mut api_key: Option<String> = None;
    let mut skip_key = false;

    // ── Check existing auth.json ──
    if let Ok(Some(token)) = oxi_sdk::load_token(provider) {
        if !token.access_token.is_empty() {
            println!();
            println!(
                "  Found existing credentials for '{}' in ~/.oxi/auth.json.",
                provider
            );
            let use_it = Confirm::new("  Use them?")
                .with_default(true)
                .prompt()?;
            if use_it {
                skip_key = true;
            }
        }
    }

    // ── Step 2: API key ──
    if !skip_key && !NO_KEY_PROVIDERS.contains(&provider) {
        // Check if env var already has the key
        if let Some(env_key) = oxi_sdk::get_env_api_key(provider) {
            println!();
            println!("  Using {} key from environment.", provider);
            api_key = Some(env_key);
            skip_key = true;
        }

        if !skip_key {
            api_key = Some(prompt_api_key(provider)?);
        }
    }

    // ── Step 3: Model selection from oxi model DB ──
    let model = prompt_model(provider)?;

    // ── Step 4: Workspace ──
    let workspace = prompt_workspace()?;

    // ── Summary ──
    let key_preview = if skip_key {
        let key = api_key.as_deref().unwrap_or("(from auth store)");
        mask_key(key)
    } else {
        mask_key(api_key.as_deref().unwrap_or("(none)"))
    };

    if !confirm_summary(provider, &model, &key_preview, &workspace)? {
        println!();
        println!("  Setup cancelled.");
        return Ok(false);
    }

    // ── Step 5: Write ──
    persist_config(
        oxios_home,
        config,
        provider,
        api_key.as_deref().unwrap_or(""),
        &model,
        &workspace,
    )?;
    print_success(oxios_home, &model);
    Ok(true)
}

// ── Prompt steps ─────────────────────────────────────────────────────────────

/// Step 1: Show providers from oxi model DB — arrow-key selection.
fn prompt_provider<'a>(providers: &[&'a str]) -> anyhow::Result<&'a str> {
    let entries: Vec<ProviderEntry> = providers
        .iter()
        .map(|&p| {
            let mut suffix = String::new();
            if oxi_sdk::has_env_key(p) {
                suffix = " 🔑".to_string();
            }
            let model_count = oxi_sdk::get_provider_models(p).len();
            ProviderEntry {
                id: p.to_string(),
                display: format!("{} [{} models]{}", p, model_count, suffix),
            }
        })
        .collect();

    println!();
    println!("  [1/{}] Select an LLM provider:", TOTAL_STEPS);

    let selected = Select::new("  Provider:", entries)
        .with_starting_cursor(0)
        .prompt()?;

    // Find the original &str by matching the id
    Ok(providers.iter().find(|&&p| p == selected.id).unwrap())
}

/// Step 2: Prompt for API key (masked input).
fn prompt_api_key(provider: &str) -> anyhow::Result<String> {
    println!();
    println!("  [2/{}] Enter your {} API key:", TOTAL_STEPS, provider);

    let key = CustomType::<String>::new("  API key:")
        .with_placeholder("sk-...")
        .with_error_message("API key is required")
        .prompt()?;
    Ok(key)
}

/// Step 3: Model selection — arrow-key selection with manual entry option.
fn prompt_model(provider: &str) -> anyhow::Result<String> {
    let models = oxi_sdk::get_provider_models(provider);

    println!();
    println!("  [3/{}] Select a model for {}:", TOTAL_STEPS, provider);

    if models.is_empty() {
        // Unknown provider — manual entry
        let model = Text::new("  Enter model ID:")
            .prompt()?;
        if model.is_empty() {
            anyhow::bail!("Model ID is required.");
        }
        return Ok(if model.contains('/') {
            model
        } else {
            format!("{}/{}", provider, model)
        });
    }

    // Build model entries (skip "latest" aliases, up to 8)
    let mut entries: Vec<ModelEntry> = Vec::new();
    for entry in models.iter() {
        if entry.name.contains("latest") {
            continue;
        }
        let ctx = if entry.context_window >= 1_000_000 {
            format!("{}M ctx", entry.context_window / 1_000_000)
        } else {
            format!("{}K ctx", entry.context_window / 1000)
        };
        let reasoning = if entry.reasoning { " ✦reasoning" } else { "" };
        entries.push(ModelEntry {
            full_id: format!("{}/{}", provider, entry.id),
            display: format!("{:<40} {:>10}{}", entry.name, ctx, reasoning),
        });
        if entries.len() >= 8 {
            break;
        }
    }

    // Add manual entry option
    let manual_entry = ModelEntry {
        full_id: String::new(), // sentinel
        display: MANUAL_MODEL_DISPLAY.to_string(),
    };
    entries.push(manual_entry);

    let selected = Select::new("  Model:", entries)
        .with_starting_cursor(0)
        .prompt()?;

    if selected.display == MANUAL_MODEL_DISPLAY {
        let manual = Text::new("  Model ID:").prompt()?;
        if manual.is_empty() {
            anyhow::bail!("Model ID cannot be empty.");
        }
        return Ok(if manual.contains('/') {
            manual
        } else {
            format!("{}/{}", provider, manual)
        });
    }

    Ok(selected.full_id.clone())
}

/// Step 4: Workspace path with default.
fn prompt_workspace() -> anyhow::Result<String> {
    let default_workspace = dirs::home_dir()
        .map(|h| format!("{}/.oxios/workspace", h.display()))
        .unwrap_or_else(|| "~/.oxios/workspace".to_string());

    println!();
    println!("  [4/{}] Workspace path:", TOTAL_STEPS);

    let workspace = Text::new("  Workspace:")
        .with_default(&default_workspace)
        .prompt()?;

    Ok(crate::config::expand_home(&workspace)
        .to_string_lossy()
        .to_string())
}

/// Step 5: Summary confirmation.
fn confirm_summary(
    provider: &str,
    model: &str,
    key_preview: &str,
    workspace: &str,
) -> anyhow::Result<bool> {
    println!();
    println!("  ┌─────────────────────────────────────────────┐");
    println!("  │            Configuration Summary             │");
    println!("  ├─────────────────────────────────────────────┤");
    println!("  │  Provider:  {:<32}│", provider);
    println!("  │  Model:     {:<32}│", model);
    println!("  │  Key:       {:<32}│", key_preview);
    println!("  │  Workspace: {:<32}│", truncate_str(workspace, 32));
    println!("  └─────────────────────────────────────────────┘");
    println!();
    println!("  [5/{}] Write configuration?", TOTAL_STEPS);

    Confirm::new("  Save this configuration?")
        .with_default(true)
        .prompt()
        .map_err(Into::into)
}

// ── Persistence ──────────────────────────────────────────────────────────────

/// Write everything to disk.
fn persist_config(
    oxios_home: &Path,
    config: &mut OxiosConfig,
    provider: &str,
    api_key: &str,
    model: &str,
    workspace: &str,
) -> anyhow::Result<()> {
    print!("\n  Saving configuration... ");
    std::io::stdout().flush()?;

    if !api_key.is_empty() {
        CredentialStore::store(provider, api_key)?;
    }

    std::fs::create_dir_all(workspace)?;
    for subdir in WORKSPACE_SUBDIRS {
        std::fs::create_dir_all(Path::new(workspace).join(subdir))?;
    }

    config.engine.default_model = model.to_string();
    config.kernel.workspace = workspace.to_string();
    write_config(oxios_home, config)?;

    println!("done");
    Ok(())
}

fn write_config(oxios_home: &Path, config: &OxiosConfig) -> anyhow::Result<()> {
    std::fs::create_dir_all(oxios_home)?;
    let toml_str = toml::to_string_pretty(config)
        .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
    std::fs::write(oxios_home.join("config.toml"), &toml_str)?;
    Ok(())
}

// ── UI ───────────────────────────────────────────────────────────────────────

fn print_banner() {
    println!();
    println!("  ╔═══════════════════════════════════════════╗");
    println!("  ║       ⬡  Oxios — First-time Setup        ║");
    println!("  ╚═══════════════════════════════════════════╝");
    println!();
    println!("  This wizard will configure your API credentials.");
    println!("  Use ↑↓ arrow keys to navigate, Enter to confirm.");
    println!("  Press Ctrl+C at any time to cancel.");
}

fn print_success(oxios_home: &Path, model: &str) {
    println!();
    println!("  ╔═══════════════════════════════════════════╗");
    println!("  ║             Setup Complete!               ║");
    println!("  ╚═══════════════════════════════════════════╝");
    println!();
    println!(
        "    Config:   {}",
        oxios_home.join("config.toml").display()
    );
    println!("    Model:    {}", model);
    println!();
    println!("  Next steps:");
    println!("    oxios               → start the daemon");
    println!("    oxios daemon install → register as system service");
    println!("    open http://127.0.0.1:4200");
    println!();
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return key.to_string();
    }
    format!("{}...{}", &key[..4], &key[key.len() - 4..])
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
