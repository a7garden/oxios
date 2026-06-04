//! `oxios update` — update binary via cargo, web UI from GitHub Releases.
//!
//! Binary update: `cargo install oxios` (optionally with `--version`)
//! Web UI:       `web-dist.zip` from GitHub Releases → `~/.oxios/web/dist/`

use anyhow::{Context, Result};
use console::style;
use std::io::Write;
use std::path::PathBuf;

/// Update oxios binary (via cargo) and/or web UI (from GitHub Releases).
pub async fn run_update(
    web_only: bool,
    binary_only: bool,
    version: Option<&str>,
    dry_run: bool,
    yes: bool,
) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");

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
        return Ok(());
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
        return Ok(());
    }

    // ── Confirmation ─────────────────────────────────────────────────────────
    if !yes {
        println!("  {} Release notes:\n", style("📋").cyan());
        for line in body.lines().take(10) {
            println!("    {line}");
        }
        if body.lines().count() > 10 {
            println!("    ... ({} more lines)", body.lines().count() - 10);
        }
        println!();

        print!("  Continue with update? [y/N] ");
        std::io::stdout().flush().ok();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("  Update cancelled.");
            return Ok(());
        }
    }

    // ── Update binary via cargo ─────────────────────────────────────────────
    if update_binary {
        let mut args = vec!["install", "oxios", "--locked"];
        if let Some(v) = version {
            args.push("--version");
            args.push(v);
        }

        print!(
            "  Running cargo install oxios{}... ",
            version
                .map(|v| format!(" --version {v}"))
                .unwrap_or_default()
        );
        std::io::stdout().flush().ok();

        let output = std::process::Command::new("cargo")
            .args(&args)
            .output()
            .context("failed to run cargo — is it installed and in PATH?")?;

        if output.status.success() {
            println!("done.");
            println!(
                "  {} Binary updated to {} via cargo.",
                style("✓").green(),
                tag_name
            );
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // cargo install outputs compilation progress to stderr
            println!();
            for line in stderr.lines().take(5) {
                println!("    {line}");
            }
            anyhow::bail!("cargo install failed (see above)");
        }

        println!();
        println!(
            "  {} Run `{}` to restart with the new binary.",
            style("ℹ").cyan(),
            style("oxios restart").bold()
        );
    }

    // ── Download and install web UI ────────────────────────────────────────
    if update_web {
        if let Some((name, url, size)) = web_asset {
            print!("  Downloading {name}... ");
            std::io::stdout().flush().ok();

            let bytes = download_file(&client, url, *size).await?;
            println!("done ({}).", format_size(bytes.len() as u64));

            let dest_dir = dest_web_dir()?;
            std::fs::create_dir_all(&dest_dir).context(format!("failed to create {dest_dir:?}"))?;

            print!("  Extracting to {dest_dir:?}... ");
            std::io::stdout().flush().ok();

            let cursor = std::io::Cursor::new(bytes);
            let mut archive = zip::ZipArchive::new(cursor).context("invalid zip file")?;

            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                let out_path = dest_dir.join(file.name());

                if file.name().ends_with('/') {
                    std::fs::create_dir_all(&out_path)?;
                } else {
                    if let Some(p) = out_path.parent() {
                        std::fs::create_dir_all(p)?;
                    }
                    let mut out_file = std::fs::File::create(&out_path)?;
                    std::io::copy(&mut file, &mut out_file)?;
                }
            }
            println!("done.");
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
    Ok(())
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
    _expected_size: u64,
) -> Result<Vec<u8>> {
    let resp = client
        .get(url)
        .send()
        .await
        .context("download request failed")?;

    if !resp.status().is_success() {
        anyhow::bail!("Download failed: {}", resp.status());
    }

    let bytes = resp.bytes().await.context("failed to read response body")?;

    Ok(bytes.to_vec())
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
