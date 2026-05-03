//! Apple Container backend for Oxios.
//!
//! Uses the `container` CLI (Apple's native macOS container runtime)
//! for garden lifecycle management. Requires macOS 15+ and Apple Silicon.

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Stdio;
use tracing::{debug, warn};

/// Path to the Apple Container CLI.
const CONTAINER_BIN: &str = "container";

/// Container name prefix for Oxios gardens.
const CONTAINER_PREFIX: &str = "oxios-";

/// Container status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerStatus {
    /// Container is being created or started.
    Starting,
    /// Container is running and healthy.
    Running,
    /// Container has been stopped.
    Stopped,
    /// Container does not exist.
    NotFound,
}

impl std::fmt::Display for ContainerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerStatus::Starting => write!(f, "starting"),
            ContainerStatus::Running => write!(f, "running"),
            ContainerStatus::Stopped => write!(f, "stopped"),
            ContainerStatus::NotFound => write!(f, "not_found"),
        }
    }
}

/// Container resource usage statistics.
#[derive(Debug, Clone)]
pub struct ContainerStats {
    /// CPU usage as a percentage.
    pub cpu_usage: f64,
    /// Memory used in MB.
    pub memory_used_mb: f64,
    /// Memory limit in MB.
    pub memory_limit_mb: f64,
}

/// Configuration for starting a garden container.
#[derive(Debug, Clone)]
pub struct GardenStartConfig {
    /// Garden name.
    pub name: String,
    /// Image tag or registry reference.
    pub image: String,
    /// Memory limit (e.g., "4g").
    pub memory_limit: Option<String>,
    /// CPU limit (number of cores).
    pub cpu_limit: Option<u64>,
    /// Workspace path to mount into the container.
    pub workspace_path: PathBuf,
    /// Optional environment file path.
    pub env_file: Option<PathBuf>,
    /// Optional API port to publish.
    pub api_port: Option<u16>,
}

/// Result of a command executed inside a container.
#[derive(Debug, Clone)]
pub struct ExecResult {
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
    /// Process exit code.
    pub exit_code: i32,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
}

/// Container backend trait.
///
/// Abstracts container operations so different runtimes can be swapped in.
#[async_trait]
pub trait ContainerBackend: Send + Sync {
    /// Name of this backend (e.g., "apple").
    fn name(&self) -> &str;

    /// Whether this backend's runtime is available on the host.
    fn is_available(&self) -> bool;

    /// Build a container image from a directory containing a Containerfile.
    async fn build_image(&self, context: &PathBuf, tag: &str) -> Result<()>;

    /// Create and start a garden container.
    async fn create_garden(&self, config: &GardenStartConfig) -> Result<()>;

    /// Start an existing garden container.
    async fn start_garden(&self, name: &str) -> Result<()>;

    /// Stop a running garden container.
    async fn stop_garden(&self, name: &str) -> Result<()>;

    /// Remove a garden container (optionally deleting files).
    async fn remove_garden(&self, name: &str, delete_files: bool) -> Result<()>;

    /// List all garden containers managed by this backend.
    async fn list_gardens(&self) -> Result<Vec<String>>;

    /// Execute a command inside a garden container.
    async fn exec_in_garden(
        &self,
        name: &str,
        cmd: &[String],
        workdir: Option<&str>,
    ) -> Result<ExecResult>;

    /// Get the status of a garden container.
    async fn garden_status(&self, name: &str) -> Result<ContainerStatus>;

    /// Get resource usage statistics for a garden container.
    async fn garden_stats(&self, name: &str) -> Result<Option<ContainerStats>>;
}

// ─── AppleBackend ──────────────────────────────────────────────────────────

/// Container backend using Apple's `container` CLI.
pub struct AppleBackend {
    /// Cached version string (empty if `container` not found).
    version: String,
    /// Whether the runtime is available on this host.
    available: bool,
}

impl AppleBackend {
    /// Create a new AppleBackend, probing for the `container` CLI.
    pub fn new() -> Self {
        let (available, version) = Self::detect_runtime();
        Self { version, available }
    }

    /// Detect whether the `container` CLI is present and return its version.
    fn detect_runtime() -> (bool, String) {
        let output = std::process::Command::new(CONTAINER_BIN)
            .arg("--version")
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let v = raw
                    .split_whitespace()
                    .find(|p| p.chars().next().is_some_and(|c| c.is_ascii_digit()))
                    .unwrap_or(&raw)
                    .to_string();
                (true, v)
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                warn!("container --version failed: {}", stderr);
                (false, String::new())
            }
            Err(e) => {
                debug!("container CLI not found: {}", e);
                (false, String::new())
            }
        }
    }

    /// Check minimum platform requirements (macOS 15+, Apple Silicon).
    fn check_platform() -> Result<()> {
        let arch = std::env::consts::ARCH;
        if arch != "aarch64" {
            bail!("Apple Container requires Apple Silicon (detected: {})", arch);
        }

        let output = std::process::Command::new("sw_vers")
            .args(["-productVersion"])
            .output()
            .context("failed to run sw_vers")?;

        if !output.status.success() {
            bail!("failed to detect macOS version");
        }

        let version_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let major: u32 = version_str
            .split('.')
            .next()
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);

        if major < 15 {
            bail!(
                "Apple Container requires macOS 15+ (detected: macOS {})",
                version_str
            );
        }

        Ok(())
    }

    /// Container name used on the host.
    fn container_name(name: &str) -> String {
        format!("{}{}", CONTAINER_PREFIX, name)
    }

    /// Check if an image exists locally.
    async fn check_local_image(&self, image_ref: &str) -> bool {
        std::process::Command::new(CONTAINER_BIN)
            .args(["image", "inspect", image_ref])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Pull an image from a registry.
    async fn pull_image(&self, image_ref: &str) -> Result<()> {
        let status = std::process::Command::new(CONTAINER_BIN)
            .args(["image", "pull", image_ref])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .current_dir(".")
            .status()
            .with_context(|| format!("failed to pull image {}", image_ref))?;

        if !status.success() {
            bail!("image pull failed for '{}' (exit {:?})", image_ref, status.code());
        }
        Ok(())
    }

    /// Build a container image from the given directory.
    fn build_image_sync(&self, name: &str, containerfile_dir: &PathBuf) -> Result<()> {
        let tag = format!("oxios:{}", name);

        let status = std::process::Command::new(CONTAINER_BIN)
            .args(["build", "-t", &tag, "."])
            .current_dir(containerfile_dir)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .with_context(|| format!("failed to run `container build` for {}", name))?;

        if !status.success() {
            bail!(
                "container build failed for '{}' (exit {:?})",
                name,
                status.code()
            );
        }

        Ok(())
    }
}

impl std::fmt::Debug for AppleBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppleBackend")
            .field("version", &self.version)
            .field("available", &self.available)
            .finish()
    }
}

#[async_trait]
impl ContainerBackend for AppleBackend {
    fn name(&self) -> &str {
        "apple"
    }

    fn is_available(&self) -> bool {
        self.available
    }

    async fn build_image(&self, context: &PathBuf, tag: &str) -> Result<()> {
        if !self.available {
            bail!("Apple Container runtime is not available.");
        }

        let status = std::process::Command::new(CONTAINER_BIN)
            .args(["build", "-t", tag, "."])
            .current_dir(context)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .with_context(|| format!("failed to build image {}", tag))?;

        if !status.success() {
            bail!("container build failed for '{}' (exit {:?})", tag, status.code());
        }

        Ok(())
    }

    async fn create_garden(&self, config: &GardenStartConfig) -> Result<()> {
        if !self.available {
            bail!(
                "Apple Container runtime is not available. \
                 Install Xcode and the `container` CLI."
            );
        }

        Self::check_platform()?;

        let cname = Self::container_name(&config.name);

        // Stop existing container if running (silent, ignore errors).
        let _ = self.stop_garden(&config.name).await;

        // Resolve image reference.
        let image_ref = if config.image.contains('/') {
            config.image.clone()
        } else {
            format!("oxios:{}", config.image)
        };

        // Check if image exists locally; if not, try pull then build.
        if !self.check_local_image(&image_ref).await {
            if self.pull_image(&image_ref).await.is_err() {
                tracing::info!("Pull failed, building image locally...");
                let project_dir = std::env::current_dir()
                    .context("cannot determine current directory")?;
                self.build_image_sync(&config.name, &project_dir)?;
            }
        }

        // Run container.
        tracing::info!(garden = %config.name, "Starting garden container");

        let workspace = config
            .workspace_path
            .canonicalize()
            .with_context(|| {
                format!(
                    "workspace path '{}' does not exist",
                    config.workspace_path.display()
                )
            })?;

        let memory = config
            .memory_limit
            .as_deref()
            .unwrap_or("4g");
        let cpus = config
            .cpu_limit
            .map(|c| c.to_string())
            .unwrap_or_else(|| "4".into());

        let mut cmd = std::process::Command::new(CONTAINER_BIN);
        cmd.arg("run")
            .arg("-d")
            .arg("--init")
            .arg("--name")
            .arg(&cname)
            .arg("--cpus")
            .arg(&cpus)
            .arg("--memory")
            .arg(memory)
            .arg("--volume")
            .arg(format!("{}:/workspace", workspace.display()));

        if let Some(env_file) = &config.env_file {
            if env_file.exists() {
                cmd.arg("--env-file").arg(env_file);
            }
        }

        if let Some(port) = config.api_port {
            cmd.arg("--publish").arg(format!("{}:{}", port, port));
        }

        cmd.arg(&image_ref);

        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .with_context(|| format!("failed to run `container run` for {}", config.name))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "container run failed for '{}' (exit {:?}): {}",
                config.name,
                output.status.code(),
                stderr.trim()
            );
        }

        tracing::info!(
            garden = %config.name,
            container = %cname,
            "Garden container started"
        );

        Ok(())
    }

    async fn start_garden(&self, name: &str) -> Result<()> {
        if !self.available {
            bail!("Apple Container runtime is not available.");
        }

        let cname = Self::container_name(name);

        let status = std::process::Command::new(CONTAINER_BIN)
            .args(["start", &cname])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .status()
            .with_context(|| format!("failed to start container '{}'", name))?;

        if !status.success() {
            bail!("container start failed for '{}' (exit {:?})", name, status.code());
        }

        tracing::info!(garden = %name, "Garden started");
        Ok(())
    }

    async fn stop_garden(&self, name: &str) -> Result<()> {
        if !self.available {
            bail!("Apple Container runtime is not available.");
        }

        let cname = Self::container_name(name);

        let _ = std::process::Command::new(CONTAINER_BIN)
            .args(["stop", &cname])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        let delete_result = std::process::Command::new(CONTAINER_BIN)
            .args(["delete", &cname])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        match delete_result {
            Ok(s) if s.success() => {
                tracing::info!(garden = %name, "Garden stopped and removed");
                Ok(())
            }
            _ => {
                tracing::debug!(garden = %name, "No running container to stop");
                Ok(())
            }
        }
    }

    async fn remove_garden(&self, name: &str, delete_files: bool) -> Result<()> {
        // Stop the container first.
        let _ = self.stop_garden(name).await;

        if delete_files {
            tracing::info!(garden = %name, "Garden files deletion requested (handled by GardenManager)");
        }

        Ok(())
    }

    async fn list_gardens(&self) -> Result<Vec<String>> {
        if !self.available {
            return Ok(Vec::new());
        }

        let output = std::process::Command::new(CONTAINER_BIN)
            .args(["list", "--format", "json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .context("failed to run container list")?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut gardens = Vec::new();

        // Parse JSON array of container objects.
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&stdout) {
            if let Some(arr) = value.as_array() {
                for obj in arr {
                    if let Some(names) = obj.get("name").and_then(|n| n.as_str()) {
                        if let Some(garden_name) = names.strip_prefix(CONTAINER_PREFIX) {
                            gardens.push(garden_name.to_string());
                        }
                    }
                }
            }
        }

        // Fallback: parse tabular output if JSON fails.
        if gardens.is_empty() && !stdout.trim().is_empty() {
            for line in stdout.lines().skip(1) {
                let fields: Vec<&str> = line.split_whitespace().collect();
                if let Some(name) = fields.first() {
                    if let Some(garden_name) = name.strip_prefix(CONTAINER_PREFIX) {
                        gardens.push(garden_name.to_string());
                    }
                }
            }
        }

        Ok(gardens)
    }

    async fn exec_in_garden(
        &self,
        name: &str,
        cmd: &[String],
        workdir: Option<&str>,
    ) -> Result<ExecResult> {
        let cname = Self::container_name(name);
        let start = std::time::Instant::now();

        let mut args = vec!["exec".to_string()];
        if let Some(wd) = workdir {
            args.push("--workdir".to_string());
            args.push(wd.to_string());
        }
        args.push(cname);
        args.extend(cmd.iter().cloned());

        let output = std::process::Command::new(CONTAINER_BIN)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .with_context(|| format!("failed to exec in container '{}'", name))?;

        Ok(ExecResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn garden_status(&self, name: &str) -> Result<ContainerStatus> {
        if !self.available {
            return Ok(ContainerStatus::NotFound);
        }

        let cname = Self::container_name(name);

        let output = std::process::Command::new(CONTAINER_BIN)
            .args(["inspect", &cname])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .with_context(|| format!("failed to inspect container '{}'", name))?;

        if !output.status.success() {
            return Ok(ContainerStatus::NotFound);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let status = parse_inspect_status(&stdout);

        Ok(status)
    }

    async fn garden_stats(&self, name: &str) -> Result<Option<ContainerStats>> {
        if !self.available {
            return Ok(None);
        }

        let cname = Self::container_name(name);

        let output = std::process::Command::new(CONTAINER_BIN)
            .args(["stats", "--no-stream", &cname])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("failed to run container stats")?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(parse_stats_output(&stdout))
    }
}

// ─── Parsing helpers ───────────────────────────────────────────────────────

/// Parse `container inspect` JSON output to determine container status.
fn parse_inspect_status(raw: &str) -> ContainerStatus {
    let parsed: serde_json::Result<serde_json::Value> = serde_json::from_str(raw.trim());

    match parsed {
        Ok(serde_json::Value::Array(arr)) => {
            if let Some(obj) = arr.first() {
                let running = obj
                    .get("status")
                    .and_then(|v| v.as_str())
                    .map(|s| s == "running")
                    .unwrap_or(false);

                if running {
                    ContainerStatus::Running
                } else {
                    ContainerStatus::Stopped
                }
            } else {
                ContainerStatus::NotFound
            }
        }
        _ => ContainerStatus::NotFound,
    }
}

/// Parse `container stats --no-stream` tabular output.
fn parse_stats_output(raw: &str) -> Option<ContainerStats> {
    let data_line = raw.lines().nth(1)?;
    let fields: Vec<&str> = data_line.split_whitespace().collect();

    if fields.len() < 4 {
        return None;
    }

    let cpu_percent = fields
        .get(1)
        .and_then(|s| s.trim_end_matches('%').parse::<f64>().ok())
        .unwrap_or(0.0);

    let (mem_used, mem_limit) = if let Some(f) = fields.get(2) {
        if f.contains('/') {
            parse_mem_usage(f)
        } else if fields.get(3).map(|s| *s == "/").unwrap_or(false) {
            let combined = format!("{}/{}", fields[2], fields.get(4).unwrap_or(&"0"));
            parse_mem_usage(&combined)
        } else {
            (0.0, 0.0)
        }
    } else {
        (0.0, 0.0)
    };

    Some(ContainerStats {
        cpu_usage: cpu_percent,
        memory_used_mb: mem_used,
        memory_limit_mb: mem_limit,
    })
}

/// Parse memory usage string like "50MiB/1GiB" → (used_mb, limit_mb).
fn parse_mem_usage(s: &str) -> (f64, f64) {
    let cleaned = s.replace(' ', "");
    let parts: Vec<&str> = cleaned.split('/').collect();
    if parts.len() != 2 {
        return (0.0, 0.0);
    }
    (parse_size_mb(parts[0]), parse_size_mb(parts[1]))
}

/// Parse a size string like "50MiB", "1GiB" into megabytes.
fn parse_size_mb(s: &str) -> f64 {
    let s = s.trim();
    if s.ends_with("GiB") {
        s.trim_end_matches("GiB").parse::<f64>().unwrap_or(0.0) * 1024.0
    } else if s.ends_with("GB") {
        s.trim_end_matches("GB").parse::<f64>().unwrap_or(0.0) * 1024.0
    } else if s.ends_with("MiB") {
        s.trim_end_matches("MiB").parse::<f64>().unwrap_or(0.0)
    } else if s.ends_with("MB") {
        s.trim_end_matches("MB").parse::<f64>().unwrap_or(0.0)
    } else if s.ends_with("KiB") {
        s.trim_end_matches("KiB").parse::<f64>().unwrap_or(0.0) / 1024.0
    } else if s.ends_with("kB") {
        s.trim_end_matches("kB").parse::<f64>().unwrap_or(0.0) / 1024.0
    } else if s.ends_with("KB") {
        s.trim_end_matches("KB").parse::<f64>().unwrap_or(0.0) / 1024.0
    } else if s.ends_with('B') {
        s.trim_end_matches('B').parse::<f64>().unwrap_or(0.0) / (1024.0 * 1024.0)
    } else {
        0.0
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mem_usage() {
        assert_eq!(parse_mem_usage("50MiB/1GiB"), (50.0, 1024.0));
        assert_eq!(parse_mem_usage("512MiB/2GiB"), (512.0, 2048.0));
        assert_eq!(parse_mem_usage("1GiB/4GiB"), (1024.0, 4096.0));
    }

    #[test]
    fn test_parse_size_mb() {
        assert_eq!(parse_size_mb("50MiB"), 50.0);
        assert_eq!(parse_size_mb("1GiB"), 1024.0);
        assert_eq!(parse_size_mb("512KiB"), 0.5);
    }

    #[test]
    fn test_container_name() {
        assert_eq!(AppleBackend::container_name("mygarden"), "oxios-mygarden");
    }

    #[test]
    fn test_parse_inspect_status_running() {
        let json = r#"[{"id":"abc123","image":"oxios:test","status":"running","createdAt":"2026-05-02T12:00:00Z"}]"#;
        let status = parse_inspect_status(json);
        assert_eq!(status, ContainerStatus::Running);
    }

    #[test]
    fn test_parse_inspect_status_stopped() {
        let json = r#"[{"id":"abc123","image":"oxios:test","status":"stopped"}]"#;
        let status = parse_inspect_status(json);
        assert_eq!(status, ContainerStatus::Stopped);
    }

    #[test]
    fn test_parse_inspect_status_empty() {
        let status = parse_inspect_status("[]");
        assert_eq!(status, ContainerStatus::NotFound);
    }

    #[test]
    fn test_parse_stats_output() {
        let output = "CONTAINER   CPU %   MEM USAGE / LIMIT   MEM %   NET I/O\n\
                       abc123      0.50%   50MiB/1GiB          5%      1.2kB/0B";
        let stats = parse_stats_output(output).unwrap();
        assert_eq!(stats.cpu_usage, 0.5);
        assert_eq!(stats.memory_used_mb, 50.0);
        assert_eq!(stats.memory_limit_mb, 1024.0);
    }

    #[test]
    fn test_exec_result_default() {
        let result = ExecResult {
            stdout: "hello".into(),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: 100,
        };
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "hello");
    }
}
