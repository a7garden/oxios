//! `oxios update` — update binary via cargo, web UI from GitHub Releases.
//!
//! Binary update: `cargo install oxios` (optionally with `--version`)
//! Web UI:       `web-dist.zip` from GitHub Releases → `~/.oxios/web/dist/`

use anyhow::{Context, Result};
use console::style;
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

/// Outcome of `oxios update` — what was actually changed on disk.
///
/// The caller (main.rs) uses this to decide whether a daemon restart is needed.
#[derive(Debug, Default, Clone, Copy)]
pub struct UpdateOutcome {
    /// The `oxios` binary was reinstalled (requires restart to take effect).
    pub binary_updated: bool,
    /// The web UI under `~/.oxios/web/dist/` was replaced.
    pub web_updated: bool,
}

impl UpdateOutcome {
    /// Nothing changed (already latest, dry run, or user cancelled).
    pub const fn unchanged() -> Self {
        Self {
            binary_updated: false,
            web_updated: false,
        }
    }

    /// Whether anything at all was updated.
    pub fn any(&self) -> bool {
        self.binary_updated || self.web_updated
    }
}

/// Update oxios binary (via cargo) and/or web UI (from GitHub Releases).
pub async fn run_update(
    web_only: bool,
    binary_only: bool,
    version: Option<&str>,
    dry_run: bool,
    yes: bool,
) -> Result<UpdateOutcome> {
    let current = env!("CARGO_PKG_VERSION");
    let mut outcome = UpdateOutcome::unchanged();

    // ── Determine what to update ────────────────────────────────────────────
    let update_binary = !web_only;
    let update_web = !binary_only;

    println!();
    println!(
        "  {} {}",
        style("⬡ Oxios Updater").bold(),
        style(format!("v{current}")).dim()
    );
    println!("  {}", "─".repeat(52));
    println!("  Current version:  {current}");
    println!(
        "  Update binary:    {}",
        if update_binary {
            "yes (cargo install)"
        } else {
            "no"
        }
    );
    println!(
        "  Update web UI:   {}",
        if update_web { "yes" } else { "no" }
    );
    if let Some(v) = version {
        println!("  Target version:  {v}");
    } else {
        println!("  Target version:  latest");
    }
    println!();

    // ── Fetch release info from GitHub (for version check + web UI) ─────────
    let owner = "a7garden";
    let repo = "oxios";
    let tag = version.map(|v| format!("v{v}"));

    let api_url = match &tag {
        Some(t) => format!("https://api.github.com/repos/{owner}/{repo}/releases/tags/{t}"),
        None => format!("https://api.github.com/repos/{owner}/{repo}/releases/latest"),
    };

    println!("  Fetching release info from GitHub...");
    let client = reqwest::Client::builder()
        .user_agent("oxios/0.3")
        .build()
        .context("failed to create HTTP client")?;

    let resp = client
        .get(&api_url)
        .send()
        .await
        .context("failed to fetch release info (check network/GITHUB_TOKEN)")?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "GitHub API error {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        );
    }

    let release: serde_json::Value = resp.json().await.context("failed to parse release JSON")?;

    let tag_name = release["tag_name"]
        .as_str()
        .unwrap_or("unknown")
        .trim_start_matches('v');
    let html_url = release["html_url"].as_str().unwrap_or("");
    let body = release["body"].as_str().unwrap_or("No release notes.");

    println!(
        "  Latest release:  {} ({})",
        style(tag_name).green().bold(),
        html_url
    );
    println!();

    if tag_name == current && !dry_run && !yes {
        println!(
            "  {} Already on latest version ({}).",
            style("✓").green(),
            current
        );
        println!("  Use `--version X.Y.Z` to force a specific version.");
        return Ok(UpdateOutcome::unchanged());
    }

    // ── Parse assets (web UI only) ──────────────────────────────────────────
    let assets: Vec<(String, String, u64)> = release["assets"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|a| {
            Some((
                a["name"].as_str()?.to_string(),
                a["browser_download_url"].as_str()?.to_string(),
                a["size"].as_u64()?,
            ))
        })
        .collect();

    let web_asset = assets.iter().find(|(name, _, _)| name == "web-dist.zip");

    // ── Dry run ──────────────────────────────────────────────────────────────
    if dry_run {
        println!("  {} Dry run — no changes made.\n", style("⚠").yellow());
        if update_web {
            if let Some((name, _, size)) = web_asset {
                println!("  Would download: {} ({})", name, format_size(*size));
                println!("  Would extract to: ~/.oxios/web/dist/");
            } else {
                println!("  {} web-dist.zip not found in release.", style("✗").red());
            }
        }
        if update_binary {
            let mut cmd = "cargo install oxios".to_string();
            if let Some(v) = version {
                cmd.push_str(&format!(" --version {v}"));
            }
            println!("  Would run: {cmd}");
        }
        return Ok(UpdateOutcome::unchanged());
    }

    // ── Confirmation ─────────────────────────────────────────────────────────
    if !yes {
        println!("  {} Release notes:\n", style("Release notes").cyan());
        for line in body.lines().take(10) {
            println!("    {line}");
        }
        if body.lines().count() > 10 {
            println!("    ... ({} more lines)", body.lines().count() - 10);
        }
        println!();

        print!("  Continue with update? ");
        std::io::stdout().flush().ok();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("  Update cancelled.");
            return Ok(UpdateOutcome::unchanged());
        }
    }

    // ── Update binary via cargo ─────────────────────────────────────────────
    if update_binary {
        let mut args = vec!["install", "oxios", "--locked"];
        if let Some(v) = version {
            args.push("--version");
            args.push(v);
        }

        // Spinner: cargo streams `Compiling X…` / `Finished` lines to stderr
        // (with carriage returns), so we parse them and update the spinner
        // message rather than dumping the raw output.
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("  {spinner} {msg}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        pb.enable_steady_tick(Duration::from_millis(100));
        pb.set_message(format!(
            "cargo install oxios{}",
            version
                .map(|v| format!(" --version {v}"))
                .unwrap_or_default()
        ));

        let mut child = std::process::Command::new("cargo")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to run cargo — is it installed and in PATH?")?;

        let stderr = child.stderr.take().expect("piped stderr");
        let pb_for_thread = pb.clone();
        let stderr_thread = std::thread::spawn(move || -> Vec<String> {
            let reader = BufReader::new(stderr);
            let mut lines = Vec::new();
            for line in reader.lines().map_while(Result::ok) {
                let t = line.trim().to_string();
                if !t.is_empty() {
                    pb_for_thread.set_message(t.clone());
                    lines.push(t);
                }
            }
            lines
        });

        let status = child.wait().context("failed to wait for cargo")?;
        let lines = stderr_thread.join().unwrap_or_default();
        pb.finish_and_clear();

        if status.success() {
            outcome.binary_updated = true;
            println!(
                "  {} Binary updated to {} via cargo.",
                style("✓").green(),
                tag_name
            );
        } else {
            println!();
            for line in lines.into_iter().take(10) {
                println!("    {line}");
            }
            anyhow::bail!("cargo install failed (see above)");
        }
    }

    // ── Download and install web UI ────────────────────────────────────────
    if update_web {
        if let Some((name, url, size)) = web_asset {
            let bytes = download_file(&client, url, *size, name).await?;

            let dest_dir = dest_web_dir()?;
            std::fs::create_dir_all(&dest_dir).context(format!("failed to create {dest_dir:?}"))?;

            let cursor = std::io::Cursor::new(bytes);
            let mut archive = zip::ZipArchive::new(cursor).context("invalid zip file")?;

            // File-count progress bar for extraction.
            let total = archive.len() as u64;
            let pb = ProgressBar::new(total);
            pb.set_style(
                ProgressStyle::with_template("  {spinner} Extracting {wide_bar} {pos}/{len} files")
                    .unwrap()
                    .progress_chars("█▓░"),
            );
            pb.enable_steady_tick(Duration::from_millis(120));

            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                // F5: Zip Slip defense — `file.name()` returns the raw
                // archive entry path (which may contain `../` or absolute
                // paths). `enclosed_name()` normalizes and rejects unsafe
                // entries, matching the pattern already used by
                // web_dist.rs and src/api/routes/system.rs::handle_update_run.
                let out_path = match file.enclosed_name() {
                    Some(p) => dest_dir.join(p),
                    None => {
                        tracing::warn!(
                            entry = file.name().to_string(),
                            "Skipping zip entry with unsafe path (traversal or absolute)"
                        );
                        continue;
                    }
                };

                if file.is_dir() {
                    std::fs::create_dir_all(&out_path)?;
                } else {
                    if let Some(p) = out_path.parent() {
                        std::fs::create_dir_all(p)?;
                    }
                    let mut out_file = std::fs::File::create(&out_path)?;
                    std::io::copy(&mut file, &mut out_file)?;
                }
                pb.inc(1);
            }
            pb.finish_with_message(format!("extracted {total} files → {dest_dir:?}"));

            outcome.web_updated = true;
            println!(
                "  {} Web UI updated to {} in {:?}.",
                style("✓").green(),
                tag_name,
                dest_dir
            );
        } else {
            println!(
                "  {} web-dist.zip not found — skipping.",
                style("⚠").yellow()
            );
        }
    }

    println!();
    Ok(outcome)
}

/// Show changelog / release notes for a given version (or latest).
pub async fn run_changelog(version: Option<&str>) -> Result<()> {
    let owner = "a7garden";
    let repo = "oxios";
    let api_url = match version {
        Some(v) => format!("https://api.github.com/repos/{owner}/{repo}/releases/tags/v{v}"),
        None => format!("https://api.github.com/repos/{owner}/{repo}/releases/latest"),
    };

    let client = reqwest::Client::builder()
        .user_agent("oxios/0.3")
        .build()
        .context("failed to create HTTP client")?;

    let resp = client
        .get(&api_url)
        .send()
        .await
        .context("failed to fetch release info")?;

    if !resp.status().is_success() {
        anyhow::bail!("Release not found: {}", resp.status());
    }

    let release: serde_json::Value = resp.json().await.context("failed to parse release JSON")?;
    let tag = release["tag_name"]
        .as_str()
        .unwrap_or("?")
        .trim_start_matches('v');
    let body = release["body"].as_str().unwrap_or("(no release notes)");
    let date = release["published_at"].as_str().unwrap_or("?");

    println!();
    println!(
        "  {} v{}  ({})",
        style("⬡ Oxios").bold(),
        style(tag).green().bold(),
        date
    );
    println!("  {}", "─".repeat(55));
    println!();
    println!("{body}");
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn dest_web_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("cannot determine home directory")?;
    Ok(home.join(".oxios").join("web").join("dist"))
}

async fn download_file(
    client: &reqwest::Client,
    url: &str,
    expected_size: u64,
    name: &str,
) -> Result<Vec<u8>> {
    let resp = client
        .get(url)
        .send()
        .await
        .context("download request failed")?;

    if !resp.status().is_success() {
        anyhow::bail!("Download failed: {}", resp.status());
    }

    // Prefer Content-Length; fall back to the asset size from the API.
    let total = resp.content_length().unwrap_or(expected_size);
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::with_template(
            "  {spinner} {prefix} {wide_bar} {bytes}/{total_bytes} ({bytes_per_sec}, ETA {eta})",
        )
        .unwrap()
        .progress_chars("█▓░"),
    );
    pb.set_prefix(format!("Downloading {name}"));
    pb.enable_steady_tick(Duration::from_millis(120));

    // Stream chunks so the bar reflects real progress instead of buffering.
    let mut bytes = Vec::with_capacity(total as usize);
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("failed to read chunk")?;
        pb.inc(chunk.len() as u64);
        bytes.extend_from_slice(&chunk);
    }

    pb.finish_with_message(format!("downloaded {}", format_size(bytes.len() as u64)));
    Ok(bytes)
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
