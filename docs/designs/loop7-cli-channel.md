# Loop 7: CLI Channel Design

## 1. Overview

The CLI channel replaces the current single-shot `oxios run <prompt>` with an
interactive multi-turn terminal session, reusing the existing Gateway/Orchestrator
pipeline via the standard `Channel` trait.

```
┌─────────────────────────────────────────┐
│  Terminal (reedline)                    │
│  > Hello agent                          │
│  < Agent is thinking...                  │
│  < Here's my response...                │
│  > .context                              │
│  < Session: abc123  Phase: evaluate      │
└─────────────────────────────────────────┘
         │                              │
         ▼                              ▼
┌─────────────────────────────────────────┐
│  CliChannel (implements Channel trait)   │
│  receive(): reads from stdin mpsc       │
│  send(): writes to stdout               │
└─────────────────────────────────────────┘
         │                              │
         ▼                              ▼
┌─────────────────────────────────────────┐
│  Gateway (existing)                    │
│  route() → Orchestrator                 │
└─────────────────────────────────────────┘
         │                              │
         ▼                              ▼
┌─────────────────────────────────────────┐
│  Kernel / Ouroboros (existing)          │
└─────────────────────────────────────────┘
```

## 2. Architecture

### 2.1 Channel Registration

Same pattern as `WebChannel`:

```rust
// src/main.rs (conceptual)
use oxios_cli::CliChannel;

let cli_channel = CliChannel::new("cli", session_store);
kernel.gateway.register(Box::new(cli_channel)).await;
```

### 2.2 CliChannel Type

```rust
// channels/oxios-cli/src/channel.rs

pub struct CliChannel {
    name: String,
    // mpsc channel for routing messages to gateway
    outgoing_tx: mpsc::Sender<IncomingMessage>,
    // For returning responses back to the interactive loop
    incoming_rx: Mutex<mpsc::Receiver<OutgoingMessage>>,
    outgoing_tx_local: mpsc::Sender<OutgoingMessage>,
}

#[async_trait]
impl Channel for CliChannel {
    fn name(&self) -> &str { "cli" }

    // receive() is unused by gateway for CLI — gateway polls it but CLI
    // uses send_incoming() directly. Kept for trait completeness.
    async fn receive(&self) -> Result<Option<IncomingMessage>> {
        Ok(None) // CLI injects via send_incoming directly
    }

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        self.outgoing_tx_local.send(msg).await?;
        Ok(())
    }
}
```

### 2.3 Relationship to WebChannel

Both implement the same `Channel` trait. The key difference:

| Aspect | WebChannel | CliChannel |
|--------|-----------|------------|
| Inbound delivery | HTTP POST → mpsc | stdin reedline → mpsc |
| Outbound delivery | broadcast → WebSocket/SSE | mpsc → stdout print |
| Session management | Cookie/session-id header | In-memory session store |
| Response correlation | `send_and_wait()` with oneshot | Direct mpsc rx |

CLI channels share the same `Gateway.route()` pipeline, so Ouroboros phases,
evaluation, and seed management are identical to web requests.

## 3. Interactive Session

### 3.1 Session State

```rust
// channels/oxios-cli/src/session.rs

pub struct Session {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub message_history: Vec<ChatEntry>,
    pub context: HashMap<String, String>,
}

pub struct ChatEntry {
    pub role: Role, // User | Agent
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

pub struct SessionStore {
    sessions: RwLock<HashMap<Uuid, Session>>,
    current: RwLock<Option<Uuid>>,
}
```

### 3.2 Session Lifecycle

```
Session starts → session_id generated (uuid)
  ↓
User types message → append to history as User entry
  ↓
CliChannel.send_incoming() → gateway.route()
  ↓
Orchestrator runs Ouroboros → response
  ↓
CliChannel receives OutgoingMessage → append as Agent entry → print
  ↓
User types again → same session_id in metadata
```

### 3.3 History

- Each session stores full message history in `Session.message_history`
- `.context` command shows session metadata and recent history
- History is lost on exit (no disk persistence in v1 — can be layered on later)
- Maximum history length: configurable (default 50 messages)

### 3.4 Ctrl+C Handling

```rust
// SIGINT signal handler
async fn handle_interrupt(
    sig_rx: tokio::sync::mpsc::Receiver<()>,
    current_session: Arc<RwLock<Option<Uuid>>>,
) {
    tokio::select! {
        _ = sig_rx.recv() => {
            // Send SIGINT to orchestrator if agent is running
            if let Some(session_id) = *current_session.read().await {
                // Signal running agent to stop
                kernel.orchestrator.cancel_session(session_id).await;
            }
            println!("\n[Session interrupted. Type .quit to exit or continue with .reset]");
        }
    }
}
```

On Ctrl+C:
1. Signal the running orchestrator task to cancel (via `CancellationToken`)
2. Print a friendly message
3. Return to prompt (session is preserved)
4. User can continue or `.reset` to clear session

## 4. Meta Commands

| Command | Description |
|---------|-------------|
| `.quit` / `.exit` | End session and exit |
| `.help` | Show available meta commands |
| `.context` | Show session info, history summary, current phase |
| `.reset` | Clear message history, start fresh session |
| `.model <id>` | Switch model (if supported by config) |
| `.info` | Show kernel config, MCP servers, available gardens |

### 4.1 Command Parsing

```rust
// Simple prefix check — no full CLI parser needed
fn parse_command(input: &str) -> Option<MetaCommand> {
    if input == ".quit" || input == ".exit" { Some(MetaCommand::Quit) }
    else if input == ".help" { Some(MetaCommand::Help) }
    else if input == ".context" { Some(MetaCommand::Context) }
    else if input == ".reset" { Some(MetaCommand::Reset) }
    else if input.starts_with(".model ") { Some(MetaCommand::Model(input.trim_start_matches(".model ").to_string())) }
    else if input == ".info" { Some(MetaCommand::Info) }
    else { None }
}
```

Commands are NOT sent to the orchestrator — handled locally in the interactive loop.

## 5. Output Modes

### 5.1 Streaming Text Output

The orchestrator's response is streamed via the existing `OutgoingMessage.content`.
For the CLI, we print as the message arrives:

```rust
// Print each outgoing message as it arrives
for msg in incoming_rx.recv().await {
    match msg.metadata.get("phase") {
        Some(phase) => print_phase_indicator(phase),
        None => {}
    }
    println!("{}", msg.content);
}
```

### 5.2 Phase Progress Indicators

During the Ouroboros lifecycle, the gateway populates response metadata:

```
metadata:
  "phase": "interview" | "seed" | "execute" | "evaluate"
  "evaluation_passed": "true" | "false"
```

CLI renders these as visual progress:

```
────────────────────────────────────────
[1/4] Interview    ................
[2/4] Seed          ................
[3/4] Execute       ................
[4/4] Evaluate      ................
────────────────────────────────────────

Thinking about your request...
```

As each phase completes (detected via orchestrator progress events), the indicator
fills. When the final `OutgoingMessage` arrives with `phase=evaluate`, the full
indicator is shown and the response is printed.

### 5.3 Color and Formatting

Using ANSI escape codes (no external crate needed):

```rust
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[36m";
const YELLOW: &str = "\x1b[33m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";

// Phase indicator colors
const PHASE_COLORS: &[&str] = &[CYAN, YELLOW, GREEN, DIM];
```

## 6. Implementation Plan

### 6.1 New Crate: `channels/oxios-cli`

```
channels/oxios-cli/
├── Cargo.toml
└── src/
    ├── lib.rs          — exports CliChannel, CliChannelHandle
    ├── channel.rs      — CliChannel implements Channel trait
    ├── session.rs      — Session, SessionStore
    ├── interactive.rs  — InteractiveLoop, line editing with reedline
    ├── commands.rs    — MetaCommand parsing
    ├── output.rs       — formatted output (phase indicators, colors)
    └── main.rs         — optional: standalone `oxios chat` binary
```

### 6.2 File Breakdown

#### `Cargo.toml`

```toml
[package]
name = "oxios-cli"
version = "0.1.0"

[dependencies]
oxios-gateway = { path = "../../crates/oxios-gateway" }
tokio = { version = "1", features = ["sync", "rt"] }
async-trait = "0.1"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
reedline = "0.24"  # Lightweight readline replacement
anyhow = "1"
tracing = "0.1"
```

#### `channel.rs` — Core Channel Implementation

```rust
use async_trait::async_trait;
use oxios_gateway::channel::Channel;
use oxios_gateway::message::{IncomingMessage, OutgoingMessage};

pub struct CliChannel {
    name: String,
    // Send messages INTO the gateway (from CLI to kernel)
    incoming_tx: tokio::sync::mpsc::Sender<IncomingMessage>,
    // Receive responses FROM the gateway (kernel → CLI display)
    outgoing_rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<OutgoingMessage>>,
    // Local channel for routing responses to the interactive loop
    response_tx: tokio::sync::mpsc::Sender<OutgoingMessage>,
}

impl CliChannel {
    pub fn new(capacity: usize) -> (Self, CliChannelHandle) {
        let (in_tx, in_rx) = tokio::sync::mpsc::channel(capacity);
        let (out_tx, out_rx) = tokio::sync::mpsc::channel(capacity);
        let (response_tx, response_rx) = tokio::sync::mpsc::channel(capacity);

        let channel = Self {
            name: "cli".into(),
            incoming_tx: in_tx,
            outgoing_rx: tokio::sync::Mutex::new(out_rx),
            response_tx,
        };

        let handle = CliChannelHandle {
            incoming_tx: in_tx,
            outgoing_tx: out_tx,
            response_tx: response_tx.clone(),
            response_rx: Mutex::new(response_rx),
        };

        (channel, handle)
    }

    /// Called by the interactive loop to inject a user message
    pub async fn send_incoming(&self, msg: IncomingMessage) -> anyhow::Result<()> {
        self.incoming_tx.send(msg).await.map_err(|e| anyhow::anyhow!("{e}"))
    }
}

#[async_trait]
impl Channel for CliChannel {
    fn name(&self) -> &str { "cli" }

    async fn receive(&self) -> anyhow::Result<Option<IncomingMessage>> {
        Ok(None) // CLI uses send_incoming() directly, not gateway polling
    }

    async fn send(&self, msg: OutgoingMessage) -> anyhow::Result<()> {
        // Route to interactive loop for display
        self.response_tx.send(msg).await?;
        Ok(())
    }
}
```

#### `session.rs` — Session Management

```rust
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub enum Role { User, Agent }

#[derive(Debug, Clone)]
pub struct ChatEntry {
    pub role: Role,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug)]
pub struct Session {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub message_history: Vec<ChatEntry>,
    pub metadata: HashMap<String, String>,
    pub current_phase: Option<String>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            created_at: Utc::now(),
            message_history: Vec::new(),
            metadata: HashMap::new(),
            current_phase: None,
        }
    }

    pub fn add_user_message(&mut self, content: String) {
        self.message_history.push(ChatEntry {
            role: Role::User,
            content,
            timestamp: Utc::now(),
        });
    }

    pub fn add_agent_message(&mut self, content: String) {
        self.message_history.push(ChatEntry {
            role: Role::Agent,
            content,
            timestamp: Utc::now(),
        });
    }
}

pub struct SessionStore {
    sessions: RwLock<HashMap<Uuid, Session>>,
    current_id: RwLock<Option<Uuid>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            current_id: RwLock::new(None),
        }
    }

    pub async fn create_session(&self) -> Uuid {
        let session = Session::new();
        let id = session.id;
        let mut sessions = self.sessions.write().await;
        sessions.insert(id, session);
        let mut current = self.current_id.write().await;
        *current = Some(id);
        id
    }

    pub async fn get_current_session(&self) -> Option<Session> {
        let current_id = self.current_id.read().await;
        if let Some(id) = *current_id {
            let sessions = self.sessions.read().await;
            sessions.get(&id).cloned()
        } else {
            None
        }
    }

    pub async fn reset_current(&self) -> Uuid {
        // Clear current session and create new one
        let new_id = self.create_session().await;
        new_id
    }
}
```

#### `interactive.rs` — Main Interactive Loop

```rust
use reedline::{Reedline, Signal, Event, EventHandler, KeyModifiers};
use anyhow::Result;

pub struct InteractiveLoop {
    reedline: Reedline,
    channel: CliChannelHandle,
    session_store: Arc<SessionStore>,
    kernel: Arc<Kernel>,
}

impl InteractiveLoop {
    pub async fn run(&mut self) -> Result<()> {
        self.print_banner()?;
        self.print_help()?;

        loop {
            let line = self.reedline.read_line(&self.prompt())?;
            let input = line.trim();

            if input.is_empty() {
                continue;
            }

            // Check for meta commands
            if let Some(cmd) = parse_command(input) {
                if self.handle_command(cmd).await? {
                    break; // .quit / .exit
                }
                continue;
            }

            // Send to orchestrator
            self.process_message(input).await?;
        }

        println!("Goodbye!");
        Ok(())
    }

    async fn process_message(&self, input: &str) -> Result<()> {
        let session_id = self.session_store.get_or_create_session().await;

        // Build incoming message
        let mut msg = IncomingMessage::new("cli", "cli-user", input);
        msg.metadata.insert("session_id".to_string(), session_id.to_string());
        msg.metadata.insert("interactive".to_string(), "true".to_string());

        // Send to channel (gateway will process it)
        self.channel.send_incoming(msg).await?;

        // Wait for response (blocking on response_rx)
        // The CliChannelHandle.response_rx receives responses sent via channel.send()
        while let Some(response) = self.channel.recv_response().await {
            self.print_response(response).await?;
        }

        Ok(())
    }

    fn prompt(&self) -> String {
        format!("\x1b[36m>\x1b[0m ")
    }

    async fn handle_command(&self, cmd: MetaCommand) -> Result<bool> {
        match cmd {
            MetaCommand::Quit | MetaCommand::Exit => Ok(true),
            MetaCommand::Help => { self.print_help(); Ok(false) }
            MetaCommand::Context => { self.print_context().await; Ok(false) }
            MetaCommand::Reset => {
                self.session_store.reset_current().await;
                println!("Session reset.");
                Ok(false)
            }
            MetaCommand::Info => { self.print_info().await; Ok(false) }
            MetaCommand::Model(model) => {
                // Update model in config
                println!("Model switched to: {}", model);
                Ok(false)
            }
        }
    }
}
```

### 6.3 Modifications to `src/main.rs`

Add a new `Chat` command variant:

```rust
// In Command enum
#[derive(Debug, Subcommand)]
enum Command {
    /// Run an interactive terminal chat with the agent.
    Chat,

    /// Run a single prompt (existing).
    Run { prompt: String },

    /// Manage container gardens (existing).
    Garden { action: GardenAction },

    // ... rest unchanged
}

// In match block:
Some(Command::Chat) => {
    cmd_chat(config_path).await
}

// New function:
async fn cmd_chat(config_path: &Path) -> Result<()> {
    use oxios_cli::CliChannel;

    let kernel = Kernel::builder()
        .config_path(config_path.to_path_buf())
        .build()
        .await?;

    // Register CLI channel
    let (cli_channel, handle) = CliChannel::new(256);
    kernel.gateway.register(Box::new(cli_channel)).await;

    // Spawn gateway loop (background)
    let gateway_handle = tokio::spawn({
        let g = kernel.gateway;
        async move { g.run().await }
    });

    // Run interactive loop (foreground)
    let mut loop_ = oxios_cli::InteractiveLoop::new(handle, kernel.clone());
    loop_.run().await?;

    // Cleanup
    gateway_handle.abort();
    Ok(())
}
```

Alternative: make `oxios chat` the default interactive mode (replacing the web server default),
with an `--web` flag to launch the web UI instead:

```rust
None => {
    // Default: interactive CLI
    if cli.no_web {
        cmd_chat(&config_path).await
    } else {
        // Existing web server path
        cmd_web_server(kernel).await
    }
}
```

## 7. Key Types Summary

```rust
// CliChannel — implements Channel trait (gateway-facing)
pub struct CliChannel {
    name: String,
    incoming_tx: mpsc::Sender<IncomingMessage>,
    outgoing_rx: Mutex<mpsc::Receiver<OutgoingMessage>>,
    response_tx: mpsc::Sender<OutgoingMessage>,
}

// CliChannelHandle — used by interactive loop
pub struct CliChannelHandle {
    incoming_tx: mpsc::Sender<IncomingMessage>,
    outgoing_tx: mpsc::Sender<OutgoingMessage>,
    response_rx: Mutex<mpsc::Receiver<OutgoingMessage>>,
}

// Session — per-user conversational state
pub struct Session {
    id: Uuid,
    message_history: Vec<ChatEntry>,
    current_phase: Option<String>,
    created_at: DateTime<Utc>,
}

// MetaCommand — CLI-only commands (not sent to orchestrator)
pub enum MetaCommand {
    Quit, Exit, Help, Context, Reset, Info,
    Model(String),
}

// InteractiveLoop — the main REPL
pub struct InteractiveLoop {
    reedline: Reedline,
    handle: CliChannelHandle,
    session_store: Arc<SessionStore>,
}
```

## 8. Security Considerations

Same as WebChannel since both go through the same gateway pipeline:

- **Authentication**: CLI has no auth — runs locally, authenticated by OS user.
  Optional: `.login` command that checks API key presence.
- **Rate limiting**: Applied by gateway's orchestrator, not per-channel.
  CLI is implicitly rate-limited by the same token/budget controls.
- **Input validation**: Message content length limited by gateway
  (same as web). Anti-spam: max message frequency in session store.
- **Sensitive data**: No PII logged; messages stored only in memory
  (not persisted to disk in v1).

## 9. Dependencies

| Dependency | Version | Purpose |
|-----------|---------|---------|
| `reedline` | 0.24 | Lightweight readline replacement (history, completion, Emacs/vi mode) |
| `tokio` | 1 | async runtime (already in workspace) |
| `async-trait` | 0.1 | Channel trait async methods |
| `uuid` | 1 | Session ID generation |
| `chrono` | 0.4 | Timestamps |
| `tracing` | 0.1 | Logging |
| `anyhow` | 1 | Error handling |

**No external crates beyond reedline**. All formatting uses ANSI escape codes,
all concurrency uses tokio primitives already in the workspace.

## 10. Phase Indicator Detail

The orchestrator emits phase transitions via the event bus (see `crates/oxios-gateway/src/gateway.rs`).
For CLI, we subscribe to the event bus to detect phase changes:

```rust
// In CliChannel::new(), subscribe to kernel.event_bus:
let event_rx = kernel.event_bus.subscribe("cli-phase").await;

// In interactive loop, poll event_rx alongside response_rx:
tokio::select! {
    Some(event) = event_rx.recv() => {
        if let Event::PhaseChange(phase) = event {
            print_phase_indicator(phase);
        }
    }
    Some(response) = self.handle.recv_response() => {
        self.print_response(response).await;
    }
}
```

The phase indicator renders in-place using carriage return (`\r`) and
overwrites the current line, then prints the response below it.

## 11. Open Questions

1. **Response delivery**: Gateway's `route()` calls `channel.send()` once at the
   end. For CLI, we need to stream intermediate progress. The current design
   waits for the single `OutgoingMessage`. Future: add `send_progress()` method
   to Channel trait for incremental updates.

2. **Multiple CLI sessions**: In v1, CLI is single-user. If multiple terminals
   connect, each gets its own `CliChannel` instance and session. No cross-session
   coordination needed yet.

3. **Session persistence**: v1 keeps sessions in memory only. Future: persist
   to `~/.oxios/sessions/<uuid>.json` for session resumption.