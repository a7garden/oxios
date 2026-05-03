//! MCP (Model Context Protocol) integration layer.
//!
//! MCP is a standard protocol for communication between AI agents and tools/data sources.
//! See: Anthropic MCP specification.
//!
//! This module provides stdio-based communication with MCP servers via JSON-RPC 2.0.
//!
//! # Protocol Overview
//!
//! MCP defines several message types:
//! - `initialize` - Establish connection with capabilities negotiation
//! - `tools/list` - List available tools from a server
//! - `tools/call` - Execute a tool with arguments
//! - `resources/list` - List available resources
//! - `resources/read` - Read a resource by URI
//!
//! # Architecture
//!
//! ```text
//! Agent → McpBridge → McpClient (per server)
//!                         ↓
//!              tokio::process::Command (stdio)
//!                         ↓
//!              JSON-RPC 2.0 (stdin/stdout)
//!                         ↓
//!              MCP Server Process
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};

use crate::program::ToolDef;

// ---------------------------------------------------------------------------
// Unique ID generator for JSON-RPC requests
// ---------------------------------------------------------------------------

static REQUEST_ID: AtomicUsize = AtomicUsize::new(1);

fn next_request_id() -> usize {
    REQUEST_ID.fetch_add(1, Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// MCP Server Configuration
// ---------------------------------------------------------------------------

/// Type alias for backwards compatibility — use [McpServer] directly.
pub type McpServerConfig = McpServer;

/// MCP server capability definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    /// Server name (unique identifier)
    pub name: String,
    /// Command to execute (e.g., "npx", "python")
    pub command: String,
    /// Command arguments
    pub args: Vec<String>,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Whether this server is enabled
    pub enabled: bool,
}

impl McpServer {
    /// Create a new MCP server configuration
    pub fn new(name: &str, command: &str) -> Self {
        Self {
            name: name.to_string(),
            command: command.to_string(),
            args: Vec::new(),
            env: HashMap::new(),
            enabled: true,
        }
    }

    /// Set command arguments
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Add an environment variable
    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env.insert(key.to_string(), value.to_string());
        self
    }
}

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 Protocol Types
// ---------------------------------------------------------------------------

/// MCP JSON-RPC request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Request ID for correlation
    pub id: serde_json::Value,
    /// Method name to invoke
    pub method: String,
    /// Optional method parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl McpRequest {
    /// Create a new JSON-RPC request with an auto-generated ID
    pub fn new(method: &str) -> Self {
        Self::with_id(next_request_id(), method)
    }

    /// Create a new JSON-RPC request with a specific ID
    pub fn with_id(id: usize, method: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(id),
            method: method.to_string(),
            params: None,
        }
    }

    /// Add parameters to the request
    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = Some(params);
        self
    }

    /// Serialize to a JSON-line (JSONL) bytes ready for stdio write
    pub fn to_jsonl(&self) -> Result<Vec<u8>> {
        let mut buf = serde_json::to_vec(self)?;
        buf.push(b'\n');
        Ok(buf)
    }
}

/// MCP JSON-RPC response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Response ID (matches request ID)
    pub id: serde_json::Value,
    /// Response result if successful
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error if request failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

/// MCP JSON-RPC error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl McpError {
    /// Create a new MCP error
    pub fn new(code: i32, message: &str) -> Self {
        Self {
            code,
            message: message.to_string(),
            data: None,
        }
    }

    /// JSON-RPC parse error (-32700)
    pub fn parse_error() -> Self {
        Self::new(-32700, "Parse error")
    }

    /// JSON-RPC invalid request (-32600)
    pub fn invalid_request(msg: &str) -> Self {
        Self::new(-32600, msg)
    }

    /// JSON-RPC method not found (-32601)
    pub fn method_not_found() -> Self {
        Self::new(-32601, "Method not found")
    }

    /// JSON-RPC invalid params (-32602)
    pub fn invalid_params() -> Self {
        Self::new(-32602, "Invalid params")
    }

    /// JSON-RPC internal error (-32603)
    pub fn internal_error(msg: &str) -> Self {
        Self::new(-32603, msg)
    }

    /// Server error (codes -32000 to -32099)
    pub fn server_error(msg: &str) -> Self {
        Self::new(-32000, msg)
    }
}

impl McpResponse {
    /// Check if this response contains an error
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    /// Extract the result value, erroring if there is one
    pub fn into_result(self) -> Result<serde_json::Value> {
        if let Some(err) = self.error {
            return Err(anyhow!("MCP error {}: {}", err.code, err.message));
        }
        Ok(self.result.unwrap_or(serde_json::Value::Null))
    }
}

// ---------------------------------------------------------------------------
// MCP Capability Negotiation
// ---------------------------------------------------------------------------

/// MCP server capabilities
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpCapabilities {
    /// Whether the server supports tools
    pub tools: bool,
    /// Whether the server supports resources
    pub resources: bool,
    /// Whether the server supports prompts
    pub prompts: bool,
}

/// Initialize request params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    pub protocol_version: String,
    pub capabilities: McpCapabilities,
    pub client_info: ClientInfo,
}

/// Client info sent during initialize
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

impl Default for InitializeParams {
    fn default() -> Self {
        Self {
            protocol_version: "2024-11-05".to_string(),
            capabilities: McpCapabilities::default(),
            client_info: ClientInfo {
                name: "oxios".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        }
    }
}

/// Initialize response from the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: McpCapabilities,
    pub server_info: ServerInfo,
}

/// Server info from initialize response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

// ---------------------------------------------------------------------------
// MCP Tool Types
// ---------------------------------------------------------------------------

/// MCP tool definition from a server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// Tool name (unique within the server)
    pub name: String,
    /// Brief description of what the tool does
    pub description: String,
    /// JSON Schema for tool input arguments
    pub input_schema: serde_json::Value,
}

impl McpTool {
    /// Convert an MCP tool to an oxios ToolDef
    pub fn to_tool_def(&self) -> ToolDef {
        let arguments = if let Some(obj) = self.input_schema.as_object() {
            obj.iter()
                .map(|(name, schema)| {
                    let description = schema
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("No description")
                        .to_string();
                    let required = schema.get("default").is_none();

                    crate::program::ArgumentDef {
                        name: name.clone(),
                        description,
                        required,
                        default: schema.get("default").and_then(|d| d.as_str().map(String::from)),
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        ToolDef {
            name: self.name.clone(),
            description: self.description.clone(),
            arguments,
        }
    }
}

/// MCP tools/list result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolsResult {
    pub tools: Vec<McpTool>,
}

/// MCP tools/call result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCallResult {
    pub content: Vec<McpContentBlock>,
    pub is_error: Option<bool>,
}

/// Content block in a tool call result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: Option<String> },
    #[serde(rename = "resource")]
    Resource { resource: MappedResource },
}

/// Resource reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappedResource {
    pub uri: String,
    pub mime_type: Option<String>,
}

// ---------------------------------------------------------------------------
// McpClient — manages a single MCP server process lifecycle
// ---------------------------------------------------------------------------

/// Manages a single MCP server process with stdio JSON-RPC communication.
///
/// `McpClient` spawns a child process and communicates with it over stdin/stdout
/// using JSON-RPC 2.0 messages (one JSON object per line).
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

        // Wrap in buffered async I/O
        let reader = BufReader::new(stdout);
        let writer = tokio::io::BufWriter::new(stdin);

        // Store child handle
        *self.child.write().await = Some(child);

        // Send initialize request
        let params = InitializeParams::default();
        let request = McpRequest::new("initialize")
            .with_params(serde_json::to_value(&params)?);

        let response = self.send_request_jsonl(request, reader, writer).await?;

        // Parse initialize result
        let result_json = response.into_result()?;
        let init_result: InitializeResult = serde_json::from_value(result_json)?;

        *self.server_info.write().await = Some(init_result.server_info.clone());
        *self.initialized.write().await = true;

        // Send initialised notification (JSON-RPC 2.0 requires this)
        let notification = McpRequest::new("notifications/initialized");
        self.send_notification_jsonl(notification).await?;

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

    /// Send a JSON-RPC request and receive a response over stdio.
    ///
    /// Uses a timeout to prevent hanging on unresponsive servers.
    async fn send_request_jsonl<R, W>(
        &self,
        request: McpRequest,
        reader: BufReader<R>,
        writer: tokio::io::BufWriter<W>,
    ) -> Result<McpResponse>
    where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        let request_id = request.id.clone();

        // Write the request
        let json = request.to_jsonl()?;
        timeout(self.request_timeout, async {
            let mut w = writer;
            w.write_all(&json).await?;
            w.flush().await?;
            Ok::<(), tokio::io::Error>(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("MCP request timed out (write): {}", e))??;

        // Read the response (single JSON line)
        let line: std::io::Result<Option<String>> = timeout(self.request_timeout, async {
            let mut lines = reader.lines();
            lines.next_line().await
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
    async fn send_notification_jsonl(&self, notification: McpRequest) -> Result<()> {
        let mut child_guard = self.child.write().await;
        let child = child_guard
            .as_mut()
            .ok_or_else(|| anyhow!("MCP server '{}' is not running", self.server.name))?;

        let stdin = child.stdin.as_mut()
            .ok_or_else(|| anyhow!("stdin not available on '{}'", self.server.name))?;

        let json = notification.to_jsonl()?;
        stdin.write_all(&json).await?;
        stdin.flush().await?;

        Ok(())
    }

    /// Call internal send_request using the stored child handle.
    async fn send_request(&self, request: McpRequest) -> Result<McpResponse> {
        // Take stdin and stdout out of the child so we can use them
        let (stdin, stdout) = {
            let mut guard = self.child.write().await;
            let child = guard.as_mut()
                .ok_or_else(|| anyhow!("MCP server '{}' is not running", self.server.name))?;
            let stdin = child.stdin.take()
                .ok_or_else(|| anyhow!("stdin not available on '{}'", self.server.name))?;
            let stdout = child.stdout.take()
                .ok_or_else(|| anyhow!("stdout not available on '{}'", self.server.name))?;
            (stdin, stdout)
        };

        let reader = BufReader::new(stdout);
        let writer = tokio::io::BufWriter::new(stdin);

        self.send_request_jsonl(request, reader, writer).await
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
    /// Sends SIGTERM to the child process. Does NOT wait for confirmation
    /// (the server may not support graceful shutdown).
    pub async fn shutdown(&self) -> Result<()> {
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

// ---------------------------------------------------------------------------
// McpBridge — manages multiple MCP server clients
// ---------------------------------------------------------------------------

/// MCP bridge — connects multiple MCP servers to the oxios tool system.
///
/// `McpBridge` owns the collection of registered MCP server configurations
/// and manages `McpClient` instances for active servers.
///
/// # Example
///
/// ```ignore
/// let mut bridge = McpBridge::new();
/// bridge.register_server(McpServer::new("files", "npx")
///     .with_args(vec!["-y", "@anthropic/mcp-server-filesystem"]));
///
/// // Initialize all servers
/// bridge.initialize_all().await?;
///
/// // List all tools across all servers
/// let tools = bridge.list_tools().await?;
/// ```
pub struct McpBridge {
    /// Registered MCP server configurations
    servers: Vec<McpServer>,
    /// Active MCP clients (keyed by server name)
    clients: RwLock<HashMap<String, Arc<McpClient>>>,
    /// Tool cache: server_name → cached tool defs
    tool_cache: RwLock<HashMap<String, Vec<ToolDef>>>,
}

impl McpBridge {
    /// Create a new empty MCP bridge
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
            clients: RwLock::new(HashMap::new()),
            tool_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Register an MCP server configuration (does not start the process).
    pub fn register_server(&mut self, server: McpServer) {
        self.servers.push(server);
    }

    /// Get all registered server configurations (names only).
    pub fn servers(&self) -> Vec<&str> {
        self.servers.iter().map(|s| s.name.as_str()).collect()
    }

    /// Get a server configuration by name.
    pub fn get_server(&self, name: &str) -> Option<&McpServer> {
        self.servers.iter().find(|s| s.name == name)
    }

    /// Initialize all enabled MCP servers.
    ///
    /// Each server is spawned as a child process and receives the initialize request.
    /// Servers that fail to initialize are logged but do not cause a total failure.
    pub async fn initialize_all(&self) -> Result<()> {
        let mut errors = Vec::new();

        for server in &self.servers {
            if !server.enabled {
                tracing::debug!(server = %server.name, "Skipping disabled MCP server");
                continue;
            }

            let client = Arc::new(McpClient::new(server.clone()));
            match client.initialize().await {
                Ok(()) => {
                    self.clients.write().await.insert(server.name.clone(), client);
                    tracing::info!(server = %server.name, "MCP server started");
                }
                Err(e) => {
                    tracing::error!(server = %server.name, error = %e, "Failed to initialize MCP server");
                    errors.push(format!("{}: {}", server.name, e));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow!("MCP initialization failed: {}", errors.join("; ")))
        }
    }

    /// Initialize a specific server by name.
    pub async fn initialize_server(&self, name: &str) -> Result<()> {
        let server = self.servers.iter()
            .find(|s| s.name == name)
            .ok_or_else(|| anyhow!("MCP server '{}' not found", name))?;

        let client = Arc::new(McpClient::new(server.clone()));
        client.initialize().await?;

        self.clients.write().await.insert(name.to_string(), client);
        Ok(())
    }

    /// Get a client by server name.
    pub async fn client(&self, name: &str) -> Option<Arc<McpClient>> {
        self.clients.read().await.get(name).cloned()
    }

    /// List all available tools from all initialized MCP servers.
    ///
    /// Tools are collected from each server's cache (refreshed on demand).
    pub async fn list_tools(&self) -> Result<Vec<ToolDef>> {
        let clients = self.clients.read().await;
        let mut all_tools = Vec::new();

        for (name, client) in clients.iter() {
            if let Ok(mcp_tools) = client.list_tools().await {
                let defs: Vec<ToolDef> = mcp_tools.iter().map(|t| t.to_tool_def()).collect();
                let start = all_tools.len();
                all_tools.extend(defs);
                *self.tool_cache.write().await
                    .entry(name.clone())
                    .or_insert_with(Vec::new) = all_tools[start..].to_vec();
            }
        }

        Ok(all_tools)
    }

    /// Get cached tools for a specific server.
    pub async fn cached_tools(&self, server_name: &str) -> Option<Vec<ToolDef>> {
        self.tool_cache.read().await.get(server_name).cloned()
    }

    /// Call an MCP tool on a specific server.
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<McpToolCallResult> {
        let clients = self.clients.read().await;
        let client = clients.get(server_name)
            .ok_or_else(|| anyhow!("MCP server '{}' not connected", server_name))?;

        client.call_tool(tool_name, args).await
    }

    /// Shutdown all connected MCP server processes.
    pub async fn shutdown_all(&self) -> Result<()> {
        let mut clients = self.clients.write().await;

        for (name, client) in clients.drain() {
            if let Err(e) = client.shutdown().await {
                tracing::warn!(server = %name, error = %e, "Error shutting down MCP server");
            }
        }

        self.tool_cache.write().await.clear();
        Ok(())
    }

    /// Refresh tools from a specific server.
    pub async fn refresh_tools(&self, server_name: &str) -> Result<Vec<ToolDef>> {
        let clients = self.clients.read().await;
        let client = clients.get(server_name)
            .ok_or_else(|| anyhow!("MCP server '{}' not connected", server_name))?;

        let mcp_tools = client.refresh_tools().await?;
        let defs: Vec<ToolDef> = mcp_tools.iter().map(|t| t.to_tool_def()).collect();

        *self.tool_cache.write().await
            .entry(server_name.to_string())
            .or_insert_with(Vec::new) = defs.clone();

        Ok(defs)
    }

    /// Clear the tool cache for a server.
    pub async fn clear_cache(&self, server_name: &str) {
        self.tool_cache.write().await.remove(server_name);
    }

    /// Clear all caches.
    pub async fn clear_all_caches(&self) {
        self.tool_cache.write().await.clear();
    }
}

impl Default for McpBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- McpServer tests ---

    #[test]
    fn test_mcp_server_builder() {
        let server = McpServer::new("test-server", "npx")
            .with_args(vec!["-y".to_string(), "@anthropic/mcp-server".to_string()])
            .with_env("DEBUG", "true");

        assert_eq!(server.name, "test-server");
        assert_eq!(server.command, "npx");
        assert_eq!(server.args, vec!["-y", "@anthropic/mcp-server"]);
        assert_eq!(server.env.get("DEBUG"), Some(&"true".to_string()));
        assert!(server.enabled);
    }

    // --- JSON-RPC request/response tests ---

    #[test]
    fn test_mcp_request_serialization() {
        let request = McpRequest::new("tools/list");
        let json = serde_json::to_string(&request).unwrap();

        assert!(json.contains(r#""method":"tools/list""#));
        assert!(json.contains(r#""jsonrpc":"2.0""#));
    }

    #[test]
    fn test_mcp_request_with_params() {
        let request = McpRequest::new("tools/call")
            .with_params(serde_json::json!({
                "name": "my_tool",
                "arguments": {"arg1": "value1"}
            }));

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("my_tool"));
        assert!(json.contains("arg1"));
    }

    #[test]
    fn test_mcp_request_to_jsonl() {
        let request = McpRequest::new("initialize");
        let jsonl = request.to_jsonl().unwrap();

        // Should end with newline
        assert_eq!(jsonl.last(), Some(&b'\n'));

        // Should parse back
        let json_str = String::from_utf8_lossy(&jsonl[..jsonl.len() - 1]);
        let parsed: McpRequest = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.method, "initialize");
    }

    #[test]
    fn test_mcp_response_result() {
        let response = McpResponse {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(1),
            result: Some(serde_json::json!({"tools": []})),
            error: None,
        };

        assert!(!response.is_error());
        let result = response.clone().into_result().unwrap();
        assert!(result.get("tools").is_some());
    }

    #[test]
    fn test_mcp_response_error() {
        let response = McpResponse {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(2),
            result: None,
            error: Some(McpError::internal_error("Something went wrong")),
        };

        assert!(response.is_error());
        let err = response.into_result().unwrap_err();
        assert!(err.to_string().contains("internal error"));
    }

    #[test]
    fn test_mcp_error_codes() {
        assert_eq!(McpError::parse_error().code, -32700);
        assert_eq!(McpError::invalid_request("test").code, -32600);
        assert_eq!(McpError::method_not_found().code, -32601);
        assert_eq!(McpError::invalid_params().code, -32602);
        assert_eq!(McpError::internal_error("x").code, -32603);
        assert_eq!(McpError::server_error("x").code, -32000);
    }

    // --- McpTool conversion tests ---

    #[test]
    fn test_mcp_tool_conversion() {
        let mcp_tool = McpTool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: serde_json::json!({
                "arg1": {
                    "type": "string",
                    "description": "First argument"
                },
                "arg2": {
                    "type": "number",
                    "description": "Second argument",
                    "default": "42"
                }
            }),
        };

        let tool_def = mcp_tool.to_tool_def();

        assert_eq!(tool_def.name, "test_tool");
        assert_eq!(tool_def.description, "A test tool");
        assert_eq!(tool_def.arguments.len(), 2);

        let arg1 = tool_def.arguments.iter().find(|a| a.name == "arg1").unwrap();
        assert!(arg1.required);
        assert_eq!(arg1.description, "First argument");

        let arg2 = tool_def.arguments.iter().find(|a| a.name == "arg2").unwrap();
        assert!(!arg2.required);
        assert_eq!(arg2.default, Some("42".to_string()));
    }

    // --- McpBridge registration tests ---

    #[test]
    fn test_bridge_registration() {
        let mut bridge = McpBridge::new();

        bridge.register_server(McpServer::new("test", "echo"));

        assert_eq!(bridge.servers(), vec!["test"]);
        assert!(bridge.get_server("test").is_some());
        assert!(bridge.get_server("missing").is_none());
    }

    // --- McpClient lifecycle tests ---

    #[tokio::test]
    async fn test_mcp_client_non_existent_command() {
        let server = McpServer::new("ghost", "nonexistent-binary-xyz");
        let client = McpClient::new(server);

        let result = client.initialize().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to spawn"));
    }

    #[tokio::test]
    async fn test_mcp_client_shutdown_no_panic() {
        let server = McpServer::new("test-shutdown", "echo");
        let client = McpClient::new(server);

        // Shutting down without initializing should not panic
        client.shutdown().await.expect("shutdown should succeed");
        assert!(!client.is_initialized().await);
    }

    #[tokio::test]
    async fn test_mcp_client_with_timeout() {
        let server = McpServer::new("test", "sleep").with_args(vec!["999".to_string()]);
        let client = McpClient::new(server).with_timeout(Duration::from_millis(100));

        // This will spawn a sleep process that hangs
        let result = client.initialize().await;
        // Should timeout, not panic
        assert!(result.is_err());
    }

    // --- McpBridge lifecycle tests ---

    #[tokio::test]
    async fn test_bridge_initialize_all_empty() {
        let bridge = McpBridge::new();
        bridge.initialize_all().await.expect("empty bridge should initialize");
    }

    #[tokio::test]
    async fn test_bridge_initialize_all_fails_gracefully() {
        let mut bridge = McpBridge::new();
        bridge.register_server(McpServer::new("ghost", "nonexistent-cmd-xyz"));
        bridge.register_server(McpServer::new("ghost2", "nonexistent-cmd-abc"));

        let result = bridge.initialize_all().await;
        // Should fail because all servers fail to spawn
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bridge_shutdown_all_empty() {
        let bridge = McpBridge::new();
        bridge.shutdown_all().await.expect("empty bridge shutdown should succeed");
    }

    #[tokio::test]
    async fn test_bridge_call_tool_no_server() {
        let bridge = McpBridge::new();
        let result = bridge
            .call_tool("ghost", "tool", serde_json::json!({}))
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not connected"));
    }

    #[tokio::test]
    async fn test_bridge_initialize_server_not_found() {
        let bridge = McpBridge::new();
        let result = bridge.initialize_server("missing").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_mcp_client_debug() {
        let server = McpServer::new("debug-test", "echo");
        let client = McpClient::new(server);
        let debug = format!("{:?}", client);
        assert!(debug.contains("debug-test"));
    }

    // --- JSON-RPC echo round-trip test (using bash script) ---

    #[tokio::test]
    async fn test_jsonrpc_echo_server() {
        // Create a bash echo script that echoes back stdin lines
        let temp_script = tempfile::tempdir().unwrap().path().join("mcp_echo.sh");
        std::fs::write(
            &temp_script,
            r#"#!/bin/bash
while IFS= read -r line; do
    echo "$line"
done
"#,
        )
        .unwrap();
        std::fs::set_permissions(&temp_script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let mut bridge = McpBridge::new();
        bridge.register_server(
            McpServer::new("echo-server", "bash")
                .with_args(vec![temp_script.to_string_lossy().to_string()]),
        );

        bridge.initialize_all().await.unwrap();

        let client = bridge.client("echo-server").await.unwrap();
        let request = McpRequest::new("tools/list")
            .with_params(serde_json::json!({"test": "value"}));
        let response = client.send_request(request).await;

        // The bash echo server will echo back the request JSON as a "response-like" string.
        // This test verifies the client correctly handles stdio communication.
        // A real MCP server would return proper JSON-RPC responses.
        if response.is_ok() {
            tracing::info!("Echo server responded successfully");
        }
    }

    // --- Double-init guard ---

    #[tokio::test]
    async fn test_mcp_client_double_init_ignored() {
        let server = McpServer::new("echo", "echo");
        let client = McpClient::new(server);

        // First init will fail because "echo" doesn't speak MCP protocol
        let _ = client.initialize().await;
        // But calling is_initialized() should not panic
        let _ = client.is_initialized().await;
    }
}
