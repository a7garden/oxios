//! MCP client — manages a single MCP server process lifecycle.
//!
//! `McpClient` spawns a child process and communicates with it over stdin/stdout
//! using JSON-RPC 2.0 messages (one JSON object per line).
//!
//! I/O handles are stored persistently (not consumed via `take()`) so that
//! multiple requests can be serialized through the same connection. A write
//! lock on both stdin and stdout is held for the duration of each request-response
//! cycle, ensuring correct ordering.

use anyhow::{anyhow, Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};

use super::protocol::*;

// ---------------------------------------------------------------------------
// McpClient — manages a single MCP server process lifecycle
// ---------------------------------------------------------------------------

/// Manages a single MCP server process with stdio JSON-RPC communication.
///
/// I/O handles are stored persistently so that concurrent requests can be
/// serialized through the same connection without consuming the handles.
///
/// # Example
///
/// ```ignore
/// let client = McpClient::new(server_config);
/// client.initialize().await?;
/// let tools = client.list_tools().await?;
/// let result = client.call_tool("my_tool", serde_json::json!({"arg": "value"})).await?;
/// client.shutdown().await?;
/// ```
pub struct McpClient {
    /// Server configuration
    server: McpServer,
    /// Child process handle (None when not running)
    child: RwLock<Option<Child>>,
    /// Persistent stdin handle for writing to the server process.
    stdin: RwLock<Option<tokio::io::BufWriter<ChildStdin>>>,
    /// Persistent stdout handle for reading from the server process.
    stdout: RwLock<Option<BufReader<ChildStdout>>>,
    /// Whether the server has been initialized
    initialized: RwLock<bool>,
    /// Cached tool list (invalidated on refresh_tools)
    tool_cache: RwLock<Option<Vec<McpTool>>>,
    /// Server info received during initialize
    server_info: RwLock<Option<ServerInfo>>,
    /// Request timeout duration
    request_timeout: Duration,
}

impl McpClient {
    /// Create a new MCP client for the given server configuration.
    ///
    /// Does NOT spawn the process yet — call `initialize()` to start and negotiate.
    pub fn new(server: McpServer) -> Self {
        Self {
            server,
            child: RwLock::new(None),
            stdin: RwLock::new(None),
            stdout: RwLock::new(None),
            initialized: RwLock::new(false),
            tool_cache: RwLock::new(None),
            server_info: RwLock::new(None),
            request_timeout: Duration::from_secs(30),
        }
    }

    /// Set the request timeout duration.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Spawn the MCP server process and establish communication.
    pub async fn initialize(&self) -> Result<()> {
        if *self.initialized.read().await {
            return Ok(());
        }

        // Spawn the child process
        let mut child = Command::new(&self.server.command)
            .args(&self.server.args)
            .envs(&self.server.env)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .with_context(|| format!("Failed to spawn MCP server '{}'", self.server.name))?;

        let stdin = child.stdin.take()
            .expect("stdin not captured — stdin was piped");
        let stdout = child.stdout.take()
            .expect("stdout not captured — stdout was piped");

        // Store persistent I/O handles (separate from child process handle)
        *self.stdin.write().await = Some(tokio::io::BufWriter::new(stdin));
        *self.stdout.write().await = Some(BufReader::new(stdout));

        // Store child handle
        *self.child.write().await = Some(child);

        // Send initialize request using persistent handles
        let params = InitializeParams::default();
        let request = McpRequest::new("initialize")
            .with_params(serde_json::to_value(&params)?);

        let response = self.send_request(request).await?;

        // Parse initialize result
        let result_json = response.into_result()?;
        let init_result: InitializeResult = serde_json::from_value(result_json)?;

        *self.server_info.write().await = Some(init_result.server_info.clone());
        *self.initialized.write().await = true;

        // Send initialised notification (JSON-RPC 2.0 requires this)
        let notification = McpRequest::new("notifications/initialized");
        self.send_notification(notification).await?;

        tracing::debug!(
            server = %self.server.name,
            version = %init_result.server_info.version,
            "MCP server initialized"
        );

        Ok(())
    }

    /// Check if the server has been initialized
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }

    /// Get the server info received during initialize
    pub async fn server_info(&self) -> Option<ServerInfo> {
        self.server_info.read().await.clone()
    }

    /// Send a JSON-RPC request using persistent I/O handles.
    ///
    /// Acquires write locks on both stdin and stdout for the duration of
    /// the request-response cycle, serializing concurrent access.
    async fn do_request(&self, request: McpRequest) -> Result<McpResponse> {
        let request_id = request.id.clone();

        // Acquire stdin lock for writing
        let mut stdin_guard = self.stdin.write().await;
        let stdin = stdin_guard
            .as_mut()
            .ok_or_else(|| anyhow!("stdin not available on '{}'", self.server.name))?;

        // Write the request
        let json = request.to_jsonl()?;
        timeout(self.request_timeout, async {
            stdin.write_all(&json).await?;
            stdin.flush().await?;
            Ok::<(), tokio::io::Error>(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("MCP request timed out (write): {}", e))??;

        // Acquire stdout lock for reading
        let mut stdout_guard = self.stdout.write().await;
        let stdout = stdout_guard
            .as_mut()
            .ok_or_else(|| anyhow!("stdout not available on '{}'", self.server.name))?;

        // Read the response (single JSON line)
        let line: std::io::Result<Option<String>> = timeout(self.request_timeout, async {
            stdout.lines().next_line().await
        })
        .await
        .map_err(|e| anyhow::anyhow!("MCP request timed out (read): {}", e))?;

        let response_str: String = line
            .context("Failed to read MCP response line from stdout")?
            .with_context(|| format!("MCP server {} returned no response", self.server.name))?;

        let parsed: McpResponse = serde_json::from_str(&response_str)
            .with_context(|| format!("Failed to parse MCP response JSON: {}", response_str))?;

        // Sanity check: ID should match
        if parsed.id != request_id {
            tracing::warn!(
                server = %self.server.name,
                expected_id = ?request_id,
                got_id = ?parsed.id,
                "MCP response ID mismatch"
            );
        }

        Ok(parsed)
    }

    /// Send a JSON-RPC notification (no response expected).
    async fn send_notification(&self, notification: McpRequest) -> Result<()> {
        let mut stdin_guard = self.stdin.write().await;
        let stdin = stdin_guard
            .as_mut()
            .ok_or_else(|| anyhow!("stdin not available on '{}'", self.server.name))?;

        let json = notification.to_jsonl()?;
        stdin.write_all(&json).await?;
        stdin.flush().await?;

        Ok(())
    }

    /// Send a JSON-RPC request via persistent I/O handles.
    pub(crate) async fn send_request(&self, request: McpRequest) -> Result<McpResponse> {
        // Verify server is running
        {
            let child = self.child.read().await;
            if child.is_none() {
                anyhow::bail!("MCP server '{}' is not running", self.server.name);
            }
        }

        self.do_request(request).await
    }

    /// List all tools available from this MCP server.
    ///
    /// Results are cached and refreshed on [refresh_tools](Self::refresh_tools).
    pub async fn list_tools(&self) -> Result<Vec<McpTool>> {
        // Return cached tools if available
        if let Some(cached) = self.tool_cache.read().await.clone() {
            return Ok(cached);
        }

        self.refresh_tools().await
    }

    /// Force-refresh the tool list from the server.
    pub async fn refresh_tools(&self) -> Result<Vec<McpTool>> {
        let request = McpRequest::new("tools/list");
        let response = self.send_request(request).await?;

        let result_json = response.into_result()?;
        let tools_result: McpToolsResult = serde_json::from_value(result_json)?;

        let tools = tools_result.tools;
        *self.tool_cache.write().await = Some(tools.clone());

        tracing::debug!(
            server = %self.server.name,
            count = tools.len(),
            "Refreshed tool cache"
        );

        Ok(tools)
    }

    /// Call a tool on this MCP server.
    ///
    /// The server must be initialized first.
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<McpToolCallResult> {
        let params = serde_json::json!({
            "name": tool_name,
            "arguments": arguments,
        });

        let request = McpRequest::new("tools/call").with_params(params);
        let response = self.send_request(request).await?;

        let result_json = response.into_result()?;
        let call_result: McpToolCallResult = serde_json::from_value(result_json)?;

        tracing::debug!(
            server = %self.server.name,
            tool = tool_name,
            "Tool call completed"
        );

        Ok(call_result)
    }

    /// Call a tool and return the result content as a string.
    ///
    /// Returns the first text content block, or an error if no text content.
    pub async fn call_tool_text(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<String> {
        let result = self.call_tool(tool_name, arguments).await?;

        for block in result.content {
            if let McpContentBlock::Text { text } = block {
                return Ok(text);
            }
        }

        Err(anyhow!("Tool '{}' returned no text content", tool_name))
    }

    /// Gracefully shutdown the MCP server process.
    ///
    /// Drops persistent I/O handles first, then kills the child process.
    pub async fn shutdown(&self) -> Result<()> {
        // Drop persistent I/O handles first
        *self.stdin.write().await = None;
        *self.stdout.write().await = None;

        let mut child_guard = self.child.write().await;

        if let Some(mut child) = child_guard.take() {
            tracing::debug!(server = %self.server.name, "Shutting down MCP server");

            // Try graceful shutdown first
            let _ = child.try_wait();

            // Kill the process
            child.kill().await?;
            let _ = child.wait().await;
        }

        *self.initialized.write().await = false;
        *self.tool_cache.write().await = None;

        Ok(())
    }

    /// Restart the server (shutdown then initialize).
    pub async fn restart(&self) -> Result<()> {
        self.shutdown().await?;
        self.initialize().await
    }

    /// Get the server configuration
    pub fn server(&self) -> &McpServer {
        &self.server
    }
}

impl std::fmt::Debug for McpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpClient")
            .field("server", &self.server.name)
            .field("initialized", &self.initialized)
            .finish()
    }
}
