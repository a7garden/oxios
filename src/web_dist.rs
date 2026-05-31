//! Web UI dist directory resolution and auto-download.
//!
//! This module runs before surfaces start to ensure the web UI is available.
//! If `~/.oxios/web/dist/index.html` doesn't exist, it downloads from GitHub Releases.
//!
//! The resolved path is passed to surfaces via [`SurfaceContext.web_dist`].
//! This avoids the race condition where the server starts listening before
//! the web UI is downloaded.

use anyhow::{Context, Result};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};

const GITHUB_REPO: &str = "a7garden/oxios";

/// Returns `~/.oxios/web/dist/` path.
fn user_web_dist_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".oxios").join("web").join("dist"))
}

/// Returns `~/.oxios/web/version` path.
fn user_web_version_file() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".oxios").join("web").join("version"))
}

/// Result of ensuring web UI availability.
#[derive(Debug)]
pub enum WebDistResult {
    /// Web UI found in `~/.oxios/web/dist/`.
    UserDir(PathBuf),
    /// Web UI found in `workspace/web/dist/`.
    WorkspaceDir(PathBuf),
    /// Downloaded from GitHub Releases.
    Downloaded { path: PathBuf, version: String },
    /// No filesystem web UI — embedded assets will be used.
    Embedded,
    /// Download failed — embedded assets will be used as fallback.
    DownloadFailed { reason: String },
}

impl WebDistResult {
    /// Returns the version tag without the leading 'v' prefix (for display).
    pub fn version_display(&self) -> Option<&str> {
        match self {
            WebDistResult::Downloaded { version, .. } => {
                Some(version.trim_start_matches('v'))
            }
            _ => None,
        }
    }
}

/// Format bytes into human-readable string.
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Fetches the latest release tag from GitHub API.
async fn fetch_latest_release_tag() -> Result<String> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let client = reqwest::Client::builder()
        .user_agent("oxios-web")
        .build()
        .context("failed to create HTTP client")?;
    let resp: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .context("failed to fetch GitHub release info")?
        .json()
        .await
        .context("failed to parse GitHub response")?;
    let tag = resp["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("tag_name not found in GitHub response"))?;
    Ok(tag.to_string())
}

/// Downloads `web-dist.zip` from a GitHub release and extracts to `~/.oxios/web/dist/`.
async fn download_and_extract_web_dist(version_tag: &str) -> Result<PathBuf> {
    let dist_dir = user_web_dist_dir()
        .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    let version_file = user_web_version_file()
        .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;

    let url = format!(
        "https://github.com/{GITHUB_REPO}/releases/download/{version_tag}/web-dist.zip"
    );

    let client = reqwest::Client::builder()
        .user_agent("oxios-web")
        .build()
        .context("failed to create HTTP client")?;

    // ── Download with progress bar ─────────────────────────────────────────
    let resp = client.get(&url).send().await.context("download request failed")?;

    if !resp.status().is_success() {
        anyhow::bail!("Failed to download web-dist.zip: HTTP {}", resp.status());
    }

    let total_size = resp.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("  {spinner} {msg}  [{bar:>.dim}] {bytes}/{total_bytes} ({bytes_per_sec})")
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  "),
    );
    let tag_label = style(version_tag).cyan().to_string();
    pb.set_message(format!("Downloading web UI {tag_label}"));

    let bytes = resp
        .bytes()
        .await
        .context("failed to read response body")?;

    let ok = style("✓").green().to_string();
    let downloaded = style("Downloaded").green().to_string();
    let done_msg = format!(
        "  {} {} ({})",
        ok,
        downloaded,
        format_size(bytes.len() as u64)
    );
    pb.finish_with_message(done_msg);

    // ── Extract with progress ─────────────────────────────────────────────
    let reader = std::io::Cursor::new(bytes.as_ref());
    let mut archive = zip::ZipArchive::new(reader).context("invalid zip file")?;
    let file_count = archive.len();

    let extract_pb = ProgressBar::new(file_count as u64);
    extract_pb.set_style(
        ProgressStyle::default_bar()
            .template("  {spinner} {msg}  [{bar:>.dim}] {pos}/{len}")
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  "),
    );
    extract_pb.set_message("Extracting files".to_string());

    // Clear old dist
    if dist_dir.exists() {
        std::fs::remove_dir_all(&dist_dir)?;
    }
    std::fs::create_dir_all(&dist_dir)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => dist_dir.join(path),
            None => continue,
        };
        if file.is_dir() {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    std::fs::create_dir_all(p)?;
                }
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
        extract_pb.inc(1);
    }

    let ok = style("✓").green().to_string();
    let done_msg = format!("  {} {} files extracted", ok, file_count);
    extract_pb.finish_with_message(done_msg);

    // Write version file
    if let Some(parent) = version_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&version_file, version_tag)?;

    tracing::info!(
        path = ?dist_dir,
        version = %version_tag,
        "Web UI downloaded and extracted"
    );

    Ok(dist_dir)
}

/// Ensures the web UI is available, downloading from GitHub if needed.
///
/// Priority order:
///  1. `~/.oxios/web/dist/index.html` — user override, always wins
///  2. `workspace/web/dist/index.html` — bundled / dev mode
///  3. Download from GitHub Releases (latest tag)
///  4. Embedded fallback (`rust-embed`, only if binary was built with web assets)
///
/// Returns a [`WebDistResult`] describing what happened.
pub async fn ensure_web_dist(workspace: &Path) -> WebDistResult {
    // 1. ~/.oxios/web/dist/ (user override — always wins)
    if let Some(ref dist) = user_web_dist_dir() {
        if dist.join("index.html").is_file() {
            tracing::info!(path = ?dist, "Serving web UI from ~/.oxios/web/dist/");
            return WebDistResult::UserDir(dist.clone());
        }
    }

    // 2. workspace/web/dist/ (bundled / dev)
    let workspace_dist = workspace.join("web").join("dist");
    if workspace_dist.join("index.html").is_file() {
        tracing::info!(path = ?workspace_dist, "Serving web UI from workspace (web/dist/)");
        return WebDistResult::WorkspaceDir(workspace_dist);
    }

    // 3. Auto-download from GitHub Releases
    tracing::info!("No web UI found locally, downloading from GitHub Releases...");
    match fetch_latest_release_tag().await {
        Ok(tag) => match download_and_extract_web_dist(&tag).await {
            Ok(p) => return WebDistResult::Downloaded { path: p, version: tag },
            Err(e) => {
                tracing::warn!(error = %e, "Failed to auto-download web UI");
                return WebDistResult::DownloadFailed {
                    reason: e.to_string(),
                };
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "Could not fetch latest release info");
            return WebDistResult::DownloadFailed {
                reason: e.to_string(),
            };
        }
    }
}
