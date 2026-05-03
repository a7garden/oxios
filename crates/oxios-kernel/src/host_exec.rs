//! Host Exec Bridge: secure relay for executing host commands on behalf of containerized agents.
//!
//! Adapted from clawgarden-relay. Listens on a Unix Domain Socket and executes
//! whitelisted host commands. Security model:
//! - Command whitelist (only approved commands can run)
//! - Argument validation (blocks shell metacharacters, path traversal)
//! - Timeout enforcement
//!
//! Protocol: Length-prefixed JSON (4-byte BE length + JSON payload).

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

/// Default socket name for the relay.
const RELAY_SOCK_NAME: &str = "oxios-relay.sock";

/// Shell metacharacters that are blocked in arguments.
const SHELL_METACHARS: &[char] = &[
    '|', '&', ';', '$', '`', '<', '>', '(', ')', '{', '}', '\n', '\r', '\0',
];

/// Default timeout in milliseconds.
const DEFAULT_TIMEOUT_MS: u64 = 10_000;

/// Maximum request size (1 MB).
const MAX_REQUEST_SIZE: usize = 1_048_576;

// ─── Protocol types ────────────────────────────────────────────────────────

/// Request from container to host relay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayRequest {
    /// Unique request ID.
    pub id: String,
    /// Command to execute (must be in allowlist).
    pub command: String,
    /// Command arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Timeout in milliseconds.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}

/// Response from host relay to container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayResponse {
    /// Matching request ID.
    pub id: String,
    /// Whether the command succeeded.
    pub ok: bool,
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
    /// Process exit code.
    pub exit_code: i32,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
}

impl RelayResponse {
    /// Create a success response.
    pub fn success(
        id: String,
        stdout: String,
        stderr: String,
        exit_code: i32,
        duration_ms: u64,
    ) -> Self {
        Self {
            ok: true,
            id,
            stdout,
            stderr,
            exit_code,
            duration_ms,
        }
    }

    /// Create an error response.
    pub fn error(id: String, stderr: String, duration_ms: u64) -> Self {
        Self {
            ok: false,
            id,
            stdout: String::new(),
            stderr,
            exit_code: -1,
            duration_ms,
        }
    }
}

// ─── Security validation ───────────────────────────────────────────────────

/// Check if arguments contain shell metacharacters or path traversal.
pub fn has_shell_metacharacters(args: &[String]) -> bool {
    for arg in args {
        if arg.contains("..") {
            return true;
        }
        if SHELL_METACHARS.iter().any(|&c| arg.contains(c)) {
            return true;
        }
    }
    false
}

/// Validate that a command is in the allowlist and is a bare name (not a path).
pub fn validate_command(command: &str, allowed: &HashSet<String>) -> Result<()> {
    if command.contains("..") {
        return Err(anyhow!("path traversal in command"));
    }
    if command.contains('/') {
        return Err(anyhow!("command must be bare name, not path"));
    }
    if !allowed.is_empty() && !allowed.contains(command) {
        return Err(anyhow!("command not in allowlist: {}", command));
    }
    Ok(())
}

/// Validate arguments against security rules.
pub fn validate_args(args: &[String]) -> Result<()> {
    if has_shell_metacharacters(args) {
        return Err(anyhow!("shell metacharacters or path traversal not allowed in arguments"));
    }
    Ok(())
}

// ─── UDS framing ───────────────────────────────────────────────────────────

/// Send a length-prefixed JSON response over a stream.
async fn send_response<W: AsyncWriteExt + Unpin>(
    stream: &mut W,
    resp: &RelayResponse,
) -> Result<()> {
    let data = serde_json::to_vec(resp)?;
    let len = data.len() as u32;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(&data).await?;
    Ok(())
}

/// Read a length-prefixed JSON request from a stream.
async fn read_request<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<RelayRequest> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_REQUEST_SIZE {
        anyhow::bail!("request too large: {} bytes", len);
    }
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).await?;
    let req: RelayRequest = serde_json::from_slice(&payload)?;
    Ok(req)
}

// ─── Result type (shared with container module) ────────────────────────────

/// Result of a host command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostExecResult {
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
    /// Process exit code.
    pub exit_code: i32,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
}

impl From<RelayResponse> for HostExecResult {
    fn from(resp: RelayResponse) -> Self {
        Self {
            stdout: resp.stdout,
            stderr: resp.stderr,
            exit_code: resp.exit_code,
            duration_ms: resp.duration_ms,
        }
    }
}

// ─── HostExecBridge ────────────────────────────────────────────────────────

/// Host execution bridge that runs a relay daemon on a Unix Domain Socket.
///
/// The relay listens for JSON requests from containers, validates commands
/// against an allowlist, and executes them on the host.
#[derive(Debug, Clone)]
pub struct HostExecBridge {
    /// Set of allowed command names.
    allowed_commands: Arc<HashSet<String>>,
    /// Path to the Unix Domain Socket.
    relay_sock_path: PathBuf,
    /// Whether the relay daemon is running.
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl HostExecBridge {
    /// Create a new HostExecBridge.
    ///
    /// The socket will be placed at `base_path/oxios-relay.sock`.
    /// If `allowed_commands` is empty, all bare-name commands are allowed
    /// (useful for development; lock down in production).
    pub fn new(base_path: PathBuf, allowed_commands: Vec<String>) -> Self {
        let allowed: HashSet<String> = allowed_commands.into_iter().collect();
        Self {
            allowed_commands: Arc::new(allowed),
            relay_sock_path: base_path.join(RELAY_SOCK_NAME),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Start the relay daemon (spawns a background task).
    ///
    /// The relay listens on the Unix Domain Socket and processes
    /// incoming requests in a loop.
    pub async fn start(&self) -> Result<()> {
        // Remove stale socket.
        if self.relay_sock_path.exists() {
            std::fs::remove_file(&self.relay_sock_path)?;
        }

        let listener = UnixListener::bind(&self.relay_sock_path)?;

        // Make socket accessible.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o666);
            std::fs::set_permissions(&self.relay_sock_path, perms)?;
        }

        self.running
            .store(true, std::sync::atomic::Ordering::SeqCst);

        tracing::info!(
            path = %self.relay_sock_path.display(),
            "Host exec relay listening"
        );

        let allowed = Arc::clone(&self.allowed_commands);
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            loop {
                if !running.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }

                match listener.accept().await {
                    Ok((mut stream, _)) => {
                        match read_request(&mut stream).await {
                            Ok(req) => {
                                let resp = handle_request(&req, &allowed).await;
                                if let Err(e) = send_response(&mut stream, &resp).await {
                                    tracing::error!(error = %e, "Failed to send relay response");
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "Failed to read relay request");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Relay accept error");
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop the relay daemon.
    pub async fn stop(&self) -> Result<()> {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);

        if self.relay_sock_path.exists() {
            std::fs::remove_file(&self.relay_sock_path)?;
        }

        tracing::info!("Host exec relay stopped");
        Ok(())
    }

    /// Execute a command on the host via direct execution (not through the relay).
    ///
    /// This is used by the GardenManager for commands that don't need to go
    /// through the UDS relay (i.e., when called from the host itself).
    pub async fn exec(
        &self,
        command: &str,
        args: Vec<String>,
        timeout_ms: u64,
    ) -> Result<HostExecResult> {
        // Validate command.
        validate_command(command, &self.allowed_commands)?;
        validate_args(&args)?;

        let start = std::time::Instant::now();
        let effective_timeout = timeout_ms.max(DEFAULT_TIMEOUT_MS).min(60_000);

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(effective_timeout),
            async {
                tokio::process::Command::new(command)
                    .args(&args)
                    .env_clear()
                    .env("HOME", std::env::var("HOME").unwrap_or_default())
                    .env("USER", std::env::var("USER").unwrap_or_default())
                    .env("LOGNAME", std::env::var("LOGNAME").unwrap_or_default())
                    .env("PATH", std::env::var("PATH").unwrap_or_default())
                    .env("LANG", std::env::var("LANG").unwrap_or("en_US.UTF-8".to_string()))
                    .env("TERM", "dumb")
                    .output()
                    .await
            },
        )
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => Ok(HostExecResult {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code().unwrap_or(-1),
                duration_ms,
            }),
            Ok(Err(e)) => Err(anyhow!("execution error: {}", e)),
            Err(_) => Err(anyhow!("command timed out after {}ms", effective_timeout)),
        }
    }

    /// Get the path to the relay socket.
    pub fn socket_path(&self) -> &PathBuf {
        &self.relay_sock_path
    }

    /// Check if the relay is running.
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Handle a single relay request.
async fn handle_request(req: &RelayRequest, allowed: &HashSet<String>) -> RelayResponse {
    let start = std::time::Instant::now();
    let duration_ms = || start.elapsed().as_millis() as u64;

    // 1. Command validation.
    if let Err(e) = validate_command(&req.command, allowed) {
        tracing::warn!(
            command = %req.command,
            "Relay: blocked command"
        );
        return RelayResponse::error(req.id.clone(), e.to_string(), duration_ms());
    }

    // 2. Argument validation.
    if let Err(e) = validate_args(&req.args) {
        tracing::warn!(
            command = %req.command,
            "Relay: blocked arguments"
        );
        return RelayResponse::error(req.id.clone(), e.to_string(), duration_ms());
    }

    // 3. Execute.
    tracing::info!(
        command = %req.command,
        args = ?req.args,
        "Relay: executing command"
    );

    let effective_timeout = req.timeout_ms.min(60_000);

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(effective_timeout),
        async {
            tokio::process::Command::new(&req.command)
                .args(&req.args)
                .env_clear()
                .env("HOME", std::env::var("HOME").unwrap_or_default())
                .env("USER", std::env::var("USER").unwrap_or_default())
                .env("LOGNAME", std::env::var("LOGNAME").unwrap_or_default())
                .env("PATH", std::env::var("PATH").unwrap_or_default())
                .env("LANG", std::env::var("LANG").unwrap_or("en_US.UTF-8".to_string()))
                .env("TERM", "dumb")
                .output()
                .await
        },
    )
    .await;

    match result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);
            RelayResponse::success(req.id.clone(), stdout, stderr, exit_code, duration_ms())
        }
        Ok(Err(e)) => RelayResponse::error(
            req.id.clone(),
            format!("execution error: {}", e),
            duration_ms(),
        ),
        Err(_) => RelayResponse::error(req.id.clone(), "timeout".to_string(), duration_ms()),
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_command_allowed() {
        let mut allowed = HashSet::new();
        allowed.insert("echo".to_string());
        assert!(validate_command("echo", &allowed).is_ok());
        assert!(validate_command("rm", &allowed).is_err());
    }

    #[test]
    fn test_validate_command_empty_allowlist() {
        let allowed = HashSet::new();
        // Empty allowlist means all commands are allowed.
        assert!(validate_command("anything", &allowed).is_ok());
    }

    #[test]
    fn test_validate_command_path_rejected() {
        let allowed = HashSet::new();
        assert!(validate_command("/usr/bin/echo", &allowed).is_err());
        assert!(validate_command("./script", &allowed).is_err());
    }

    #[test]
    fn test_validate_command_traversal_rejected() {
        let allowed = HashSet::new();
        assert!(validate_command("..", &allowed).is_err());
        assert!(validate_command("foo..bar", &allowed).is_err());
    }

    #[test]
    fn test_has_shell_metacharacters() {
        assert!(has_shell_metacharacters(&["foo;bar".into()]));
        assert!(has_shell_metacharacters(&["$(whoami)".into()]));
        assert!(has_shell_metacharacters(&["foo`bar`".into()]));
        assert!(has_shell_metacharacters(&["../etc/passwd".into()]));
        assert!(!has_shell_metacharacters(&["hello".into(), "world".into()]));
    }

    #[test]
    fn test_validate_args_clean() {
        assert!(validate_args(&["arg1".into(), "arg2".into()]).is_ok());
    }

    #[test]
    fn test_validate_args_blocked() {
        assert!(validate_args(&["foo;rm -rf /".into()]).is_err());
        assert!(validate_args(&["../secret".into()]).is_err());
    }

    #[tokio::test]
    async fn test_host_exec_bridge_echo() {
        let tmp = tempfile::tempdir().unwrap();
        let bridge = HostExecBridge::new(
            tmp.path().to_path_buf(),
            vec!["echo".to_string()],
        );

        // Use exec directly (not through relay).
        let result = bridge.exec("echo", vec!["hello world".into()], 5000).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("hello world"));
    }

    #[tokio::test]
    async fn test_host_exec_bridge_blocked_command() {
        let tmp = tempfile::tempdir().unwrap();
        let bridge = HostExecBridge::new(
            tmp.path().to_path_buf(),
            vec!["echo".to_string()],
        );

        let result = bridge.exec("rm", vec!["-rf".into(), "/".into()], 5000).await;
        assert!(result.is_err());
    }
}
