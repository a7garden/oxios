# Loop 7: TUI Design (ratatui)

## 1. Overview

The TUI provides a full terminal user interface as an alternative to the Dioxus
web dashboard. It runs locally in the same process, requires no browser,
and offers sub-second response time for monitoring and interaction.

**Why TUI vs. Web:**
- No browser required — works over SSH, in tmux, on remote servers
- Lower latency — all state accessed in-process, no HTTP round-trips
- Developer-friendly — familiar terminal workflow, scriptable
- Lightweight — no HTML/CSS/JS rendering overhead
- Single binary — no need to run a separate web server process

The TUI is NOT a replacement for the web dashboard — it's a complementary
interface focused on monitoring, debugging, and quick agent interactions.
The web UI remains the primary user-facing product for non-technical users.

## 2. Architecture

### 2.1 Integration Strategy: Single Binary with `--tui` Flag

The TUI is integrated into the main `oxios` binary via a flag, rather than a
separate binary:

```
oxios                    → starts web server (default)
oxios tui                → starts TUI only
oxios tui --web          → starts TUI + web server together
oxios --help             → shows all options
```

This is the preferred approach because:
1. Shares the same `Kernel` instance — no need to sync state between processes
2. TUI can access kernel internals directly (no IPC overhead)
3. Single install, one binary to update
4. Easy to add `--web` flag to TUI mode for hybrid usage

Alternative considered: Separate `oxios-tui` binary. Rejected because it would
require IPC (Unix socket or DBUS) to share kernel state, adding complexity.

### 2.2 Architecture Diagram

```
┌──────────────────────────────────────────────────────────┐
│                     oxios binary                         │
│                                                          │
│  ┌──────────────────┐      ┌──────────────────────────┐ │
│  │   TUI App        │      │   Kernel                 │ │
│  │                  │      │                          │ │
│  │  ratatui         │ ←──→ │  Gateway                 │ │
│  │  (UI rendering)  │      │    ↓                    │ │
│  │                  │      │  Orchestrator           │ │
│  │  crossterm       │      │    ↓                    │ │
│  │  (terminal I/O)  │      │  Supervisor             │ │
│  │                  │      │    ↓                    │ │
│  │                  │      │  EventBus (broadcast)    │ │
│  └──────────────────┘      │                          │ │
│          ↑                 └──────────────────────────┘ │
│          │                                               │
│          │ event_bus.subscribe("tui")                    │
└──────────┴───────────────────────────────────────────────┘
```

### 2.3 Kernel Access from TUI

The `Kernel` struct is passed directly to the TUI app:

```rust
async fn cmd_tui(config_path: &Path, with_web: bool) -> Result<()> {
    let kernel = Kernel::builder()
        .config_path(config_path.to_path_buf())
        .build()
        .await?;

    if with_web {
        // Start web server in background (same as current main.rs)
        start_web_server(kernel.clone()).await?;
    }

    // Run TUI — takes ownership of kernel
    let app = oxios_tui::App::new(kernel);
    app.run()?;

    Ok(())
}
```

The TUI accesses kernel state via shared references:

```rust
pub struct App {
    kernel: Arc<Kernel>,
    // TUI state
    current_tab: Tab,
    chat_history: Vec<ChatMessage>,
    agent_list: Vec<AgentInfo>,
}
```

### 2.4 Event Bus Integration

The TUI subscribes to the kernel's `EventBus` to receive live updates:

```rust
impl App {
    fn subscribe_to_events(&self, kernel: &Kernel) {
        let event_rx = kernel.event_bus.subscribe("tui").await;

        tokio::spawn(async move {
            while let Ok(event) = event_rx.recv().await {
                match event {
                    Event::AgentStarted(id) => {
                        // update agent list panel
                    }
                    Event::GardenStatusChanged(name, status) => {
                        // update gardens panel
                    }
                    Event::LogEntry(entry) => {
                        // append to logs panel
                    }
                    _ => {}
                }
            }
        });
    }
}
```

## 3. Layout

### 3.1 Screen Layout

```
┌─ Oxios ─────────────────────────────────────────────────┐  ← Title bar (fixed)
│ [1:Dashboard] [2:Chat] [3:Agents] [4:Gardens] [5:Logs] │  ← Tab bar (clickable/numbered)
│─────────────────────────────────────────────────────────│
│                                                          │
│                                                          │
│                    Content Area                          │
│            (changes per selected tab)                    │
│                                                          │
│                                                          │
│─────────────────────────────────────────────────────────│  ← Status bar (fixed)
│  3 agents │ 2 gardens │ Memory: 42 │ CPU 8%             │  ← Live stats
└──────────────────────────────────────────────────────────┘
```

### 3.2 Navigation

- **Tab switching**: Click tab, or press `1-5` number keys
- **Quit**: `q` key or Ctrl+C
- **Help overlay**: `?` key shows keyboard shortcuts
- **Command mode**: `:` key opens a command palette (search agents, exec in garden)

### 3.3 Sizing

The layout uses percentage-based splitting:

```
┌─ Title bar ─────────────────────────┐  2 lines (hardcoded)
├─ Tab bar ───────────────────────────┤  1 line
├─────────────────────────────────────┤
│                                     │
│         Content area                │  flex: fill
│                                     │
├─ Status bar ────────────────────────┤  2 lines (hardcoded)
└─────────────────────────────────────┘
```

## 4. Panels

### 4.1 Dashboard Panel (Tab 1)

Shows system overview:

```
┌─ Oxios Dashboard ──────────────────────────────────────┐
│                                                         │
│  ACTIVE AGENTS                      MEMORY              │
│  ─────────────                      ──────              │
│  ┌────────────────────────┐         Used: 42 blocks     │
│  │ ● claude-sonnet        │         Available: 200      │
│  │   Phase: evaluate      │                             │
│  │   Seed: abc-123        │         CPU: 8%            │
│  └────────────────────────┘                             │
│                                                         │
│  RECENT EVENTS                                        │
│  ─────────────                                        │
│  12:45:03  Agent started: claude-sonnet                │
│  12:44:58  Garden "dev" started                        │
│  12:44:12  MCP server "filesystem" connected            │
│  12:43:55  Evaluation passed: seed-xyz                  │
│                                                         │
│  GARDENS                                               │
│  ───────                                               │
│  ● dev      running   ubuntu:22.04                     │
│  ○ staging  stopped   ubuntu:22.04                     │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### 4.2 Chat Panel (Tab 2)

Interactive agent conversation (reuses CLI channel logic):

```
┌─ Oxios Chat ───────────────────────────────────────────┐
│  Session: abc-123    Model: claude-sonnet             │
│───────────────────────────────────────────────────────│
│  [12:42] You: Run the tests for the auth module        │
│                                                         │
│  [12:43] Agent: Running pytest on tests/auth/         │
│           Collecting tests...                           │
│           test_login ✓                                 │
│           test_logout ✓                                │
│           test_token_refresh ✓                         │
│           3 passed in 2.1s                             │
│                                                         │
│───────────────────────────────────────────────────────│
│  [User input area — multiline editor]                  │
│  > _                                                   │
└─────────────────────────────────────────────────────────┘
```

Features:
- Multiline input (Enter to newline, Shift+Enter to send)
- History navigation (↑/↓ arrows)
- Session management (`.reset`, `.context` same as CLI)
- Streaming output as agent responds
- Phase indicator during Ouroboros cycle

### 4.3 Agents Panel (Tab 3)

List and manage running agents:

```
┌─ Agents ───────────────────────────────────────────────┐
│                                                         │
│  Running (3)                       Paused (0)           │
│  ───────────                      ────────              │
│                                                         │
│  ┌─────────────────────────────────────────────────────┐│
│  │ ID: agent-001                                       ││
│  │ Model: claude-sonnet-4-20250514                    ││
│  │ Phase: execute     Seed: seed-xyz                  ││
│  │ Started: 12:41:03  Duration: 2m 34s               ││
│  │                                                    ││
│  │ [Kill] [Pause] [Inspect] [Logs]                   ││
│  └─────────────────────────────────────────────────────┘│
│                                                         │
│  ┌─────────────────────────────────────────────────────┐│
│  │ ID: agent-002                                       ││
│  │ Model: claude-opus-4-20250514                      ││
│  │ Phase: seed      Seed: —                           ││
│  │ Started: 12:43:12  Duration: 34s                  ││
│  │                                                    ││
│  │ [Kill] [Pause] [Inspect] [Logs]                   ││
│  └─────────────────────────────────────────────────────┘│
│                                                         │
│  Press Enter on agent to inspect                       │
│  j/k to navigate                                       │
└─────────────────────────────────────────────────────────┘
```

### 4.4 Gardens Panel (Tab 4)

Container management:

```
┌─ Gardens ──────────────────────────────────────────────┐
│                                                         │
│  Container Runtime: docker  Available: ✓               │
│                                                         │
│  ┌─ dev ─────────────────────────────────────────────┐ │
│  │ Image:    ubuntu:22.04                            │ │
│  │ Status:   running                                 │ │
│  │ Created:  2026-05-01                              │ │
│  │ Ports:    8080→8080                              │ │
│  │                                                    │ │
│  │ [Start] [Stop] [Restart] [Exec] [Logs] [Remove]   │ │
│  └────────────────────────────────────────────────────┘ │
│                                                         │
│  ┌─ staging ─────────────────────────────────────────┐ │
│  │ Image:    ubuntu:22.04                            │ │
│  │ Status:   stopped                                  │ │
│  │ Created:  2026-05-02                              │ │
│  │                                                    │ │
│  │ [Start] [Restart] [Logs] [Remove]                 │ │
│  └────────────────────────────────────────────────────┘ │
│                                                         │
│  Press Enter on garden for detailed view               │
└─────────────────────────────────────────────────────────┘
```

### 4.5 Logs Panel (Tab 5)

Real-time event stream:

```
┌─ Logs ─────────────────────────────────────────────────┐
│  Filter: [all ▼]  Search: [_________]  [Clear]        │
│───────────────────────────────────────────────────────│
│ 12:45:12 INFO   gateway.route      channel=web        │
│ 12:45:11 DEBUG  orchestrator      phase=seed          │
│ 12:45:10 INFO   agent.start       id=agent-003        │
│ 12:45:09 WARN   mcp_bridge        server=fs timeout   │
│ 12:45:08 INFO   container.start   garden=dev          │
│ 12:45:07 DEBUG  event_bus          subscribers=12     │
│ 12:45:06 INFO   session.create    id=abc-123          │
│ 12:45:05 DEBUG  orchestrator      phase=interview     │
│ 12:45:04 INFO   gateway.route     channel=web         │
│                                                         │
│  ← Scroll up  ↓ Scroll down  / Search  c Clear        │
└─────────────────────────────────────────────────────────┘
```

Logs panel uses `kernel.event_bus` subscription to receive all events.
The TUI renders them with filtering by level (INFO/DEBUG/WARN/ERROR) and
search by content.

## 5. Key Implementation Decisions

### 5.1 Separate Binary vs. Integrated

**Decision: Integrated into main binary with `--tui` flag**

Rationale:
- Single binary distribution
- Kernel state shared without IPC
- Consistent with `oxios run` / `oxios chat` CLI approach
- Web server and TUI can run together if needed

### 5.2 Event Bus Reuse

The TUI does NOT need to be a `Channel` (implements `Channel` trait). Instead,
it subscribes to the `EventBus` directly:

```rust
// TUI gets events, doesn't implement Channel trait
let event_rx = kernel.event_bus.subscribe("tui").await;

// Only the Chat panel needs a CliChannel for message routing
// The CliChannel is embedded in the Chat panel, not the whole TUI app
```

This separation is important: the TUI is primarily a monitoring/management
interface. The `Channel` trait is for message routing (CLI/Web/Telegram).
TUI uses events for state updates and the Chat panel uses CliChannel for
conversation.

### 5.3 State Sharing

```rust
pub struct AppState {
    kernel: Arc<Kernel>,
    // Derived/computed state for TUI
    agents: Arc<RwLock<Vec<AgentInfo>>>,
    gardens: Arc<RwLock<Vec<GardenInfo>>>,
    events: Arc<RwLock<Vec<LogEntry>>>,
    // UI state
    current_tab: Tab,
    popup: Option<Popup>,
}
```

TUI state is updated by:
1. Event subscriptions (kernel.event_bus) → background tokio task → updates `AppState`
2. Direct kernel queries (e.g., `kernel.supervisor.list()`) → on tab switch

### 5.4 Rendering Model

Using ratatui's stateful widgets:

```rust
struct App {
    framework: RatatuiAppState,
    // TUI-specific state
}

impl StatefulWidget for App {
    type State = RatatuiAppState;

    fn render(
        self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut Self::State,
    ) {
        // Render title bar
        self.render_title_bar(area, buf, state);
        // ... etc
    }
}
```

The framework state (`RatatuiAppState`) manages cursor, focus, and scroll
positions. The `App` struct holds Oxios-specific data.

## 6. Dependencies

| Dependency | Version | Purpose |
|-----------|---------|---------|
| `ratatui` | 0.27 | Terminal UI framework |
| `crossterm` | 0.27 | Terminal I/O (bundled with ratatui) |

**No additional widget libraries needed.** All custom widgets are built
from ratatui primitives (Paragraph, Block, List, Table, etc.).

```
# Cargo.toml additions
[dependencies]
ratatui = "0.27"
```

## 7. Implementation Plan

### 7.1 File Structure

```
channels/oxios-tui/
├── Cargo.toml
└── src/
    ├── lib.rs              — exports App, AppState, init_tui_mode()
    ├── app.rs              — main App struct, StatefulWidget impl
    ├── tabs.rs             — Tab enum, tab rendering
    ├── panels/
    │   ├── mod.rs
    │   ├── dashboard.rs    — Dashboard panel
    │   ├── chat.rs         — Chat panel (CliChannel integration)
    │   ├── agents.rs       — Agents panel
    │   ├── gardens.rs      — Gardens panel
    │   └── logs.rs         — Logs panel
    ├── widgets/
    │   ├── mod.rs
    │   ├── phase_indicator.rs
    │   ├── agent_card.rs
    │   └── table.rs
    ├── events.rs           — Event subscription and handling
    └── main.rs             — tui entry point (optional)
```

### 7.2 Step-by-Step Implementation

**Phase 1: Scaffold**
1. Create `channels/oxios-tui/Cargo.toml` with ratatui dependency
2. Create `channels/oxios-tui/src/lib.rs` with `App` struct
3. Implement basic ratatui app that renders a static layout

**Phase 2: Tab Navigation**
4. Add tab bar with 5 tabs (Dashboard, Chat, Agents, Gardens, Logs)
5. Implement tab switching on keypress (1-5 keys, or click)
6. Create separate render methods for each panel

**Phase 3: Dashboard Panel**
7. Subscribe to event bus for agent/garden updates
8. Render agent list and garden status
9. Add live stats in status bar (CPU, memory — sampled from /proc or via tokio)

**Phase 4: Chat Panel**
10. Integrate `CliChannel` (from `oxios-cli` or reimplemented for TUI)
11. Add multiline input widget
12. Add session management (`.reset`, `.context`)

**Phase 5: Agents Panel**
13. Poll `kernel.supervisor.list()` on tab activation
14. Add action buttons (Kill, Pause, Inspect, Logs)
15. Implement keyboard navigation (j/k arrows, Enter to select)

**Phase 6: Gardens Panel**
16. Poll `kernel.container_manager` for garden list
17. Add start/stop/exec buttons
18. Add exec modal (opens input dialog for command)

**Phase 7: Logs Panel**
19. Subscribe to all events from kernel.event_bus
20. Buffer last 1000 events in `AppState.events`
21. Implement filter by level and text search
22. Add virtual scrolling for large log buffers

**Phase 8: Polish**
23. Add help overlay (`?` key)
24. Add command palette (`:key`)
25. Add status bar with live stats
26. Add keyboard shortcuts for all actions

### 7.3 Modifications to `src/main.rs`

```rust
#[derive(Debug, Subcommand)]
enum Command {
    /// Run an interactive terminal chat (CLI channel).
    Chat,

    /// Run the terminal UI (TUI).
    Tui {
        /// Also start the web server alongside TUI.
        #[arg(long)]
        with_web: bool,
    },

    /// Run a single prompt (existing).
    Run { prompt: String },

    /// Manage container gardens (existing).
    Garden { action: GardenAction },

    /// Show system status (existing).
    Status,
    // ... rest unchanged
}

// In match block:
Some(Command::Tui { with_web }) => {
    cmd_tui(&config_path, with_web).await
}
Some(Command::Chat) => {
    cmd_chat(&config_path).await
}
```

The `cmd_tui` function:

```rust
async fn cmd_tui(config_path: &Path, with_web: bool) -> Result<()> {
    let kernel = Kernel::builder()
        .config_path(config_path.to_path_buf())
        .build()
        .await?;

    // Initialize kernel (MCP, skills, programs — same as main.rs)
    init_kernel_services(&kernel).await?;

    if with_web {
        // Start web server in background task
        let web_channel = oxios_web::WebChannel::new(256);
        kernel.gateway.register(Box::new(web_channel)).await;
        let _web_server = start_web_server(kernel.clone()).await?;
    }

    // Run TUI
    let tui_app = oxios_tui::App::new(kernel);
    let terminal = ratatui::init();
    tui_app.run(terminal)?;

    Ok(())
}
```

## 8. Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `1-5` | Switch to tab |
| `q` / `Ctrl+C` | Quit |
| `?` | Show help overlay |
| `↑` / `↓` | Navigate list |
| `Enter` | Select / confirm |
| `Esc` | Cancel / close popup |
| `/` | Focus search |
| `:` | Command palette |
| `g g` | Go to top (logs) |
| `G` | Go to bottom (logs) |
| `Ctrl+L` | Refresh current panel |

## 9. Concurrency Model

```
Main thread: ratatui rendering loop
    ↓
tick() → render() → draw to terminal
    ↑
Event handler: reads AppState → triggers re-render

Background tasks:
    ├── Event subscription → updates agents/gardens/logs state
    ├── Orchestrator → processes messages (Chat panel)
    ├── Web server → handles HTTP (if --with-web)
    └── Gateway loop → routes messages
```

All UI state is updated via `Arc<RwLock<...>>` wrapped in `tokio::sync::mpsc`
to wake the render loop when data changes.

## 10. Relationship to oxios-cli

The TUI's Chat panel shares code with the standalone `oxios chat` CLI:

```
┌─────────────────────┐      ┌─────────────────────┐
│   oxios-cli         │      │   oxios-tui         │
│   (standalone)      │      │   (TUI)             │
│                     │      │                     │
│  InteractiveLoop    │ ←──┐ │  Chat panel         │
│  (reedline input)   │    │ │  (ratatui input)    │
└─────────────────────┘    │ └─────────────────────┘
                          │
                          │ shares: CliChannel, SessionStore
                          ▼
                   ┌─────────────────────┐
                   │  oxios-cli crate    │
                   │  (CliChannel +      │
                   │   InteractiveLoop)  │
                   └─────────────────────┘
```

The TUI's Chat panel imports `CliChannel` and `SessionStore` from `oxios-cli`.
Both use the same `CliChannel` implementation to talk to the gateway.

## 11. Open Questions

1. **Refresh rate**: Dashboard and status bar should update live (every ~1s).
   Chat panel responds on-demand. Should logs panel update in real-time or
   on-demand? Decision: real-time with rate limiting (max 10 events/s rendered).

2. **Scroll behavior**: Should Tabs persist scroll position when switching?
   Decision: yes, maintain scroll state per tab in `AppState`.

3. **UTF-8 / unicode**: Should TUI support unicode icons (→, ●, ✓) or
   stick to ASCII for maximum compatibility? Decision: use unicode but provide
   ASCII fallback (detect terminal capability via crossterm).

4. **Mouse support**: Should tabs and buttons be clickable?
   Decision: yes, crossterm mouse events enabled. Click on tab switches tab,
   click on button triggers action. Keyboard remains primary input.

5. **Split view**: Future enhancement: split view showing Dashboard + Chat
   side by side. Not in v1 — single tab visible at a time.

## 12. File Inventory

```
channels/oxios-tui/Cargo.toml
channels/oxios-tui/src/lib.rs
channels/oxios-tui/src/app.rs
channels/oxios-tui/src/tabs.rs
channels/oxios-tui/src/panels/mod.rs
channels/oxios-tui/src/panels/dashboard.rs
channels/oxios-tui/src/panels/chat.rs
channels/oxios-tui/src/panels/agents.rs
channels/oxios-tui/src/panels/gardens.rs
channels/oxios-tui/src/panels/logs.rs
channels/oxios-tui/src/widgets/mod.rs
channels/oxios-tui/src/widgets/phase_indicator.rs
channels/oxios-tui/src/widgets/agent_card.rs
channels/oxios-tui/src/events.rs
channels/oxios-tui/src/main.rs      ← optional entry point (or use from main.rs)

Modified files:
  src/main.rs  — add Tui command variant, cmd_tui function
  Cargo.toml   — add oxios-tui dependency
```