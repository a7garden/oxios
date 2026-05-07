//! Container lifecycle manager.
//!
//! Ties together the ContainerBackend, HostExecBridge, and StateStore
//! to manage isolated execution environments for agents.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context, Result};

use crate::container::{
    AppleBackend, ContainerBackend, ContainerConfig, ContainerStats, ContainerStatus, ExecResult,
};
use crate::host_exec::HostExecBridge;
use crate::state_store::StateStore;

/// Default image tag for containers.
const DEFAULT_IMAGE_TAG: &str = "oxios:latest";

/// Default Containerfile content for a new container.
const DEFAULT_CONTAINERFILE: &str = r#"# Oxios Container Containerfile
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl git ripgrep jq sqlite3 bash python3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /workspace
CMD ["/bin/bash"]
"#;

/// Rust toolchain containerfile.
const RUST_TOOLCHAIN_CONTAINERFILE: &str = r#"# Oxios Rust Dev Container
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl git ripgrep jq bash ca-certificates \
    build-essential pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
WORKDIR /workspace
CMD ["/bin/bash"]
"#;

/// Node.js / TypeScript toolchain containerfile.
const NODE_TOOLCHAIN_CONTAINERFILE: &str = r#"# Oxios Node Dev Container
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl git ripgrep jq bash ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    && npm install -g typescript ts-node \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /workspace
CMD ["/bin/bash"]
"#;

/// Python toolchain containerfile.
const PYTHON_TOOLCHAIN_CONTAINERFILE: &str = r#"# Oxios Python Dev Container
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl git ripgrep jq bash ca-certificates python3 python3-pip python3-venv \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /workspace
CMD ["/bin/bash"]
"#;

/// Select a containerfile template based on language/toolchain.
pub fn containerfile_for_toolchain(toolchain: &str) -> &'static str {
    match toolchain {
        "rust" => RUST_TOOLCHAIN_CONTAINERFILE,
        "node" | "typescript" | "ts" => NODE_TOOLCHAIN_CONTAINERFILE,
        "python" | "python3" => PYTHON_TOOLCHAIN_CONTAINERFILE,
        _ => DEFAULT_CONTAINERFILE,
    }
}

/// Status of a single tool in a container.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolStatus {
    /// Tool name.
    pub name: String,
    /// Whether the tool is available and functional.
    pub available: bool,
    /// Version string if detected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Report on the health of tools in a container.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ToolHealthReport {
    /// Per-tool status.
    pub tools: Vec<ToolStatus>,
    /// Number of healthy tools.
    pub healthy_count: usize,
    /// Total tools checked.
    pub total_checked: usize,
}

/// Container metadata stored in the state store.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContainerInfo {
    /// Container name.
    pub name: String,
    /// Image tag used.
    pub image_tag: String,
    /// Creation timestamp.
    pub created_at: String,
    /// Whether the container is currently running.
    pub running: bool,
    /// Toolchain installed in the container (e.g., "rust", "python").
    #[serde(default)]
    pub toolchain: Option<String>,
    /// Whether container tools have been verified.
    #[serde(default)]
    pub tools_verified: bool,
}

/// Container lifecycle manager.
///
/// Coordinates container operations, host exec bridge, and state
/// persistence for isolated agent execution environments.
pub struct ContainerManager {
    /// Container backend (Apple Container).
    backend: Arc<dyn ContainerBackend>,
    /// Host exec bridge for running commands on the host.
    host_exec: Arc<HostExecBridge>,
    /// State store for persisting container metadata.
    state_store: Arc<StateStore>,
    /// Base directory for container workspaces.
    containers_base: PathBuf,
}

impl ContainerManager {
    /// Create a new ContainerManager.
    pub fn new(
        backend: Arc<dyn ContainerBackend>,
        host_exec: Arc<HostExecBridge>,
        state_store: Arc<StateStore>,
        containers_base: PathBuf,
    ) -> Self {
        Self {
            backend,
            host_exec,
            state_store,
            containers_base,
        }
    }

    /// Create a ContainerManager with the default Apple backend.
    pub fn with_apple_backend(
        host_exec: Arc<HostExecBridge>,
        state_store: Arc<StateStore>,
        containers_base: PathBuf,
    ) -> Self {
        let backend = Arc::new(AppleBackend::new());
        Self::new(backend, host_exec, state_store, containers_base)
    }

    /// Get the base path for containers.
    pub fn containers_base(&self) -> &PathBuf {
        &self.containers_base
    }

    /// Get the workspace path for a named container.
    pub fn workspace_path(&self, name: &str) -> std::path::PathBuf {
        self.containers_base.join(name).join("workspace")
    }

    /// Get the active container name, if any container is running.
    pub async fn active_container_name(&self) -> Option<String> {
        let containers = self.list_containers().await.ok()?;
        containers.into_iter().find(|c| c.running).map(|c| c.name)
    }

    /// Create a new container with a specific toolchain template.
    pub async fn new_container_with_toolchain(
        &self,
        name: &str,
        toolchain: &str,
    ) -> Result<()> {
        let container_dir = self.containers_base.join(name);
        if container_dir.exists() {
            bail!("Container '{}' already exists", name);
        }

        // Validate name (alphanumeric, hyphens, underscores only).
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            bail!(
                "Container name must contain only alphanumeric characters, hyphens, and underscores"
            );
        }

        // Create directory structure.
        let workspace_dir = container_dir.join("workspace");
        tokio::fs::create_dir_all(&workspace_dir)
            .await
            .with_context(|| format!("failed to create container directory for '{}'", name))?;

        // Write Containerfile for the selected toolchain.
        let containerfile = containerfile_for_toolchain(toolchain);
        tokio::fs::write(container_dir.join("Containerfile"), containerfile)
            .await
            .context("failed to write Containerfile")?;

        // Write empty .env.
        tokio::fs::write(container_dir.join(".env"), "")
            .await
            .context("failed to write .env")?;

        // Persist container metadata.
        let info = ContainerInfo {
            name: name.to_string(),
            image_tag: DEFAULT_IMAGE_TAG.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            running: false,
            toolchain: if toolchain == "default" {
                None
            } else {
                Some(toolchain.to_string())
            },
            tools_verified: false,
        };
        self.state_store
            .save_json("containers", name, &info)
            .await
            .context("failed to save container metadata")?;

        tracing::info!(name = %name, toolchain = %toolchain, "Container created with toolchain");
        Ok(())
    }

    /// Create a new container workspace.
    ///
    /// Creates the directory structure:
    /// - `$containers_base/<name>/workspace/` (mounted to container)
    /// - `$containers_base/<name>/Containerfile`
    /// - `$containers_base/<name>/.env` (empty)
    pub async fn new_container(&self, name: &str) -> Result<()> {
        self.new_container_with_toolchain(name, "default").await
    }

    /// Start a container.
    pub async fn start_container(&self, name: &str) -> Result<()> {
        let container_dir = self.containers_base.join(name);
        if !container_dir.exists() {
            bail!("Container '{}' does not exist", name);
        }

        let workspace_dir = container_dir.join("workspace");
        if !workspace_dir.exists() {
            bail!(
                "Container '{}' workspace directory is missing",
                name
            );
        }

        // Load container metadata.
        let info: Option<ContainerInfo> = self
            .state_store
            .load_json("containers", name)
            .await
            .context("failed to load container metadata")?;

        let image_tag = info
            .as_ref()
            .map(|i| i.image_tag.clone())
            .unwrap_or_else(|| DEFAULT_IMAGE_TAG.to_string());

        let config = ContainerConfig {
            name: name.to_string(),
            image: image_tag,
            memory_limit: Some("4g".to_string()),
            cpu_limit: Some(4),
            workspace_path: workspace_dir,
            env_file: Some(container_dir.join(".env")),
            api_port: None,
        };

        self.backend
            .create(&config)
            .await
            .with_context(|| format!("failed to start container '{}'", name))?;

        // Update metadata.
        if let Some(mut info) = info {
            info.running = true;
            self.state_store
                .save_json("containers", name, &info)
                .await?;
        }

        Ok(())
    }

    /// Stop a running container.
    pub async fn stop_container(&self, name: &str) -> Result<()> {
        self.backend
            .stop(name)
            .await
            .with_context(|| format!("failed to stop container '{}'", name))?;

        // Update metadata.
        let info: Option<ContainerInfo> = self.state_store.load_json("containers", name).await?;
        if let Some(mut info) = info {
            info.running = false;
            self.state_store
                .save_json("containers", name, &info)
                .await?;
        }

        tracing::info!(container = %name, "Container stopped");
        Ok(())
    }

    /// Remove a container entirely (stops container and deletes workspace).
    pub async fn remove_container(&self, name: &str) -> Result<()> {
        // Stop the container if running.
        let _ = self.backend.stop(name).await;

        // Remove container directory.
        let container_dir = self.containers_base.join(name);
        if container_dir.exists() {
            tokio::fs::remove_dir_all(&container_dir)
                .await
                .with_context(|| format!("failed to remove container directory for '{}'", name))?;
        }

        // Remove metadata.
        let meta_path = self
            .state_store
            .base_path
            .join("containers")
            .join(format!("{}.json", name));
        if meta_path.exists() {
            tokio::fs::remove_file(&meta_path).await?;
        }

        tracing::info!(container = %name, "Container removed");
        Ok(())
    }

    /// List all known containers.
    pub async fn list_containers(&self) -> Result<Vec<ContainerInfo>> {
        let mut containers = Vec::new();

        // List from state store.
        let names = self
            .state_store
            .list_category("containers")
            .await
            .unwrap_or_default();

        for name in names {
            if let Ok(Some(info)) = self
                .state_store
                .load_json::<ContainerInfo>("containers", &name)
                .await
            {
                containers.push(info);
            }
        }

        // Also check for containers that exist on disk but aren't in state store.
        if self.containers_base.exists() {
            let mut entries = tokio::fs::read_dir(&self.containers_base).await?;
            while let Some(entry) = entries.next_entry().await? {
                if entry.file_type().await?.is_dir() {
                    let dir_name = entry.file_name().to_string_lossy().into_owned();
                    if !containers.iter().any(|c| c.name == dir_name) {
                        containers.push(ContainerInfo {
                            name: dir_name,
                            image_tag: DEFAULT_IMAGE_TAG.to_string(),
                            created_at: String::new(),
                            running: false,
                            toolchain: None,
                            tools_verified: false,
                        });
                    }
                }
            }
        }

        // Update running status from container backend.
        if let Ok(running) = self.backend.list().await {
            for container in &mut containers {
                container.running = running.contains(&container.name);
            }
        }

        containers.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(containers)
    }

    /// Execute a command inside a container.
    pub async fn exec_in_container(
        &self,
        name: &str,
        cmd: &[String],
        workdir: Option<&str>,
    ) -> Result<ExecResult> {
        let status = self.backend.status(name).await?;
        if status != crate::container::ContainerStatus::Running {
            bail!(
                "Container '{}' is not running (status: {})",
                name,
                status
            );
        }

        self.backend
            .exec_in_container(name, cmd, workdir)
            .await
            .with_context(|| format!("failed to exec in container '{}'", name))
    }

    /// Execute a command on the host via the exec bridge.
    pub async fn host_exec(
        &self,
        command: &str,
        args: Vec<String>,
        timeout_ms: u64,
    ) -> Result<crate::host_exec::HostExecResult> {
        self.host_exec
            .exec(command, args, timeout_ms)
            .await
            .context("host exec failed")
    }

    /// Get the status of a container.
    pub async fn container_status(&self, name: &str) -> Result<ContainerStatus> {
        self.backend.status(name).await
    }

    /// Get resource usage stats for a container.
    pub async fn container_stats(&self, name: &str) -> Result<Option<ContainerStats>> {
        self.backend.stats(name).await
    }

    /// Check if the container backend is available.
    pub fn is_backend_available(&self) -> bool {
        self.backend.is_available()
    }

    /// Get the backend name.
    pub fn backend_name(&self) -> &str {
        self.backend.name()
    }

    /// Check the health of tools in a container by running version commands.
    ///
    /// Attempts to exec common tool version commands in the container
    /// and reports which tools are available.
    pub async fn check_tool_health(&self, name: &str) -> Result<ToolHealthReport> {
        let mut report = ToolHealthReport::default();

        let checks = [
            ("bash", vec!["bash".to_string(), "--version".to_string()]),
            ("git", vec!["git".to_string(), "--version".to_string()]),
            ("python3", vec!["python3".to_string(), "--version".to_string()]),
            ("curl", vec!["curl".to_string(), "--version".to_string()]),
            ("ripgrep", vec!["rg".to_string(), "--version".to_string()]),
            ("jq", vec!["jq".to_string(), "--version".to_string()]),
            ("sqlite3", vec!["sqlite3".to_string(), "--version".to_string()]),
        ];

        for (tool, cmd) in &checks {
            match self.exec_in_container(name, cmd, None).await {
                Ok(result) if result.exit_code == 0 => {
                    report.tools.push(ToolStatus {
                        name: tool.to_string(),
                        available: true,
                        version: result.stdout.lines().next().map(|s| s.to_string()),
                    });
                }
                _ => {
                    report.tools.push(ToolStatus {
                        name: tool.to_string(),
                        available: false,
                        version: None,
                    });
                }
            }
        }

        report.healthy_count = report.tools.iter().filter(|t| t.available).count();
        report.total_checked = report.tools.len();

        Ok(report)
    }
}

impl std::fmt::Debug for ContainerManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContainerManager")
            .field("containers_base", &self.containers_base)
            .field("backend", &self.backend.name())
            .finish()
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_default_containerfile_valid() {
        // Ensure the default Containerfile is sensible.
        assert!(DEFAULT_CONTAINERFILE.contains("FROM"));
        assert!(DEFAULT_CONTAINERFILE.contains("WORKDIR /workspace"));
    }

    #[test]
    fn test_containerfile_for_toolchain_rust() {
        let cf = containerfile_for_toolchain("rust");
        assert!(cf.contains("rustup"));
        assert!(cf.contains("WORKDIR /workspace"));
        assert_eq!(cf, RUST_TOOLCHAIN_CONTAINERFILE);
    }

    #[test]
    fn test_containerfile_for_toolchain_node() {
        for alias in &["node", "typescript", "ts"] {
            let cf = containerfile_for_toolchain(alias);
            assert!(cf.contains("nodesource"));
            assert!(cf.contains("WORKDIR /workspace"));
            assert_eq!(cf, NODE_TOOLCHAIN_CONTAINERFILE);
        }
    }

    #[test]
    fn test_containerfile_for_toolchain_python() {
        for alias in &["python", "python3"] {
            let cf = containerfile_for_toolchain(alias);
            assert!(cf.contains("python3"));
            assert!(cf.contains("WORKDIR /workspace"));
            assert_eq!(cf, PYTHON_TOOLCHAIN_CONTAINERFILE);
        }
    }

    #[test]
    fn test_containerfile_for_toolchain_unknown_returns_default() {
        let cf = containerfile_for_toolchain("unknown-lang");
        assert_eq!(cf, DEFAULT_CONTAINERFILE);

        let cf2 = containerfile_for_toolchain("");
        assert_eq!(cf2, DEFAULT_CONTAINERFILE);
    }

    #[tokio::test]
    async fn test_new_container_creates_structure() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("containers");
        let state = StateStore::new(tmp.path().join("state")).unwrap();

        let host_exec = Arc::new(
            HostExecBridge::new(tmp.path().to_path_buf(), vec!["echo".to_string()])
                .expect("non-empty allowlist required"),
        );

        let manager = ContainerManager::with_apple_backend(host_exec, Arc::new(state), base.clone());

        manager.new_container("test-project").await.unwrap();

        assert!(base.join("test-project").exists());
        assert!(base.join("test-project/workspace").exists());
        assert!(base.join("test-project/Containerfile").exists());
        assert!(base.join("test-project/.env").exists());
    }

    #[tokio::test]
    async fn test_new_container_rejects_duplicate() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("containers");
        let state = StateStore::new(tmp.path().join("state")).unwrap();

        let host_exec = Arc::new(
            HostExecBridge::new(tmp.path().to_path_buf(), vec!["echo".to_string()])
                .expect("non-empty allowlist required"),
        );

        let manager = ContainerManager::with_apple_backend(host_exec, Arc::new(state), base.clone());

        manager.new_container("test").await.unwrap();
        let result = manager.new_container("test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_new_container_rejects_bad_name() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("containers");
        let state = StateStore::new(tmp.path().join("state")).unwrap();

        let host_exec = Arc::new(
            HostExecBridge::new(tmp.path().to_path_buf(), vec!["echo".to_string()])
                .expect("non-empty allowlist required"),
        );

        let manager = ContainerManager::with_apple_backend(host_exec, Arc::new(state), base);

        assert!(manager.new_container("bad name").await.is_err());
        assert!(manager.new_container("bad/name").await.is_err());
        assert!(manager.new_container("bad;name").await.is_err());
        assert!(manager.new_container("good-name").await.is_ok());
        assert!(manager.new_container("good_name").await.is_ok());
        assert!(manager.new_container("GoodName123").await.is_ok());
    }

    #[tokio::test]
    async fn test_remove_container() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("containers");
        let state = StateStore::new(tmp.path().join("state")).unwrap();

        let host_exec = Arc::new(
            HostExecBridge::new(tmp.path().to_path_buf(), vec!["echo".to_string()])
                .expect("non-empty allowlist required"),
        );

        let manager = ContainerManager::with_apple_backend(host_exec, Arc::new(state), base.clone());

        manager.new_container("to-remove").await.unwrap();
        assert!(base.join("to-remove").exists());

        manager.remove_container("to-remove").await.unwrap();
        assert!(!base.join("to-remove").exists());
    }
}
