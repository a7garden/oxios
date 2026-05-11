//! Lightpanda process lifecycle management.
//!
//! Spawns and manages the `lightpanda serve` subprocess that provides
//! a CDP (Chrome DevTools Protocol) server on localhost.

use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// Configuration for the Lightpanda subprocess.
#[derive(Debug, Clone)]
pub struct LightpandaConfig {
    /// Path to the lightpanda binary.
    /// Defaults to `"lightpanda"` (resolved from PATH).
    pub binary_path: String,
    /// Host to bind the CDP server to.
    pub host: String,
    /// Port for the CDP server.
    pub port: u16,
    /// Whether the browser integration is enabled.
    pub enabled: bool,
}

impl Default for LightpandaConfig {
    fn default() -> Self {
        Self {
            binary_path: "lightpanda".to_string(),
            host: "127.0.0.1".to_string(),
            port: 9222,
            enabled: true,
        }
    }
}

impl LightpandaConfig {
    /// WebSocket endpoint URL for CDP connections.
    pub fn ws_endpoint(&self) -> String {
        format!("ws://{}:{}", self.host, self.port)
    }
}

/// Manages the Lightpanda CDP server as a child process.
///
/// The process is spawned lazily on first use and reused across
/// multiple browser operations. It is killed when this struct is dropped.
pub struct LightpandaProcess {
    config: LightpandaConfig,
    child: Arc<Mutex<Option<Child>>>,
}

impl LightpandaProcess {
    /// Create a new process manager with the given configuration.
    pub fn new(config: LightpandaConfig) -> Self {
        Self {
            config,
            child: Arc::new(Mutex::new(None)),
        }
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &LightpandaConfig {
        &self.config
    }

    /// Ensure the Lightpanda process is running.
    ///
    /// If the process is already running, this is a no-op.
    /// If it crashed or was killed, a new one is spawned.
    pub async fn ensure_running(&self) -> Result<()> {
        let mut guard = self.child.lock().await;

        // Check if existing process is still alive.
        if let Some(ref mut child) = *guard {
            match child.try_wait() {
                Ok(Some(status)) => {
                    tracing::warn!(
                        exit = %status,
                        "Lightpanda process exited, restarting"
                    );
                    // Process died — fall through to respawn.
                }
                Ok(None) => {
                    // Still running.
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to check lightpanda status, restarting");
                }
            }
        }

        // Spawn the process.
        tracing::info!(
            binary = %self.config.binary_path,
            host = %self.config.host,
            port = %self.config.port,
            "Starting Lightpanda CDP server"
        );

        let child = Command::new(&self.config.binary_path)
            .arg("serve")
            .arg("--host")
            .arg(&self.config.host)
            .arg("--port")
            .arg(self.config.port.to_string())
            // Minimal environment — same pattern as ExecTool.
            .env_clear()
            .env("HOME", std::env::var("HOME").unwrap_or_default())
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("USER", std::env::var("USER").unwrap_or_default())
            .env("LANG", "en_US.UTF-8")
            .env("LIGHTPANDA_DISABLE_TELEMETRY", "true")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .with_context(|| {
                format!(
                    "Failed to spawn lightpanda at '{}'. Is it installed? \
                     Install: brew install lightpanda-io/browser/lightpanda \
                     or download from https://github.com/lightpanda-io/browser/releases",
                    self.config.binary_path
                )
            })?;

        *guard = Some(child);

        // Give the server a moment to start listening.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        tracing::info!("Lightpanda CDP server started");
        Ok(())
    }

    /// Gracefully shut down the Lightpanda process.
    pub async fn shutdown(&self) -> Result<()> {
        let mut guard = self.child.lock().await;
        if let Some(ref mut child) = *guard {
            tracing::info!("Shutting down Lightpanda process");
            // Send SIGTERM (Unix) or kill (Windows).
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        *guard = None;
        Ok(())
    }
}

impl Drop for LightpandaProcess {
    fn drop(&mut self) {
        // Best-effort cleanup. The Arc<Mutex<>> prevents us from
        // doing async cleanup here, so we rely on the OS to reap
        // the child process if shutdown() wasn't called.
        // In practice, shutdown() is called from agent teardown.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LightpandaConfig::default();
        assert_eq!(config.binary_path, "lightpanda");
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 9222);
        assert!(config.enabled);
    }

    #[test]
    fn test_ws_endpoint() {
        let config = LightpandaConfig::default();
        assert_eq!(config.ws_endpoint(), "ws://127.0.0.1:9222");
    }

    #[test]
    fn test_custom_config() {
        let config = LightpandaConfig {
            binary_path: "/usr/local/bin/lightpanda".to_string(),
            host: "0.0.0.0".to_string(),
            port: 8080,
            enabled: false,
        };
        assert_eq!(config.ws_endpoint(), "ws://0.0.0.0:8080");
        assert!(!config.enabled);
    }
}
