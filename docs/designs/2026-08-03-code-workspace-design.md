# Oxios Code Workspace — Design Document

> **Date:** 2026-08-03
> **Status:** Draft (autonomous design — user review pending)
> **Scope:** New "Code" top-level surface in the Oxios web UI — a full GUI coding agent IDE

---

## 1. Vision

Build a **browser-based AI coding IDE** as the fourth top-level surface in the Oxios web UI.
This is the GUI successor to oxicode's TUI — a completely new architecture that treats the
Oxios daemon as the execution host and the browser as a rich, structured IDE.

**Not** a chat with code capabilities. A real IDE with a file explorer, code editor, terminal,
diff review, and an integrated AI agent that reads, writes, and runs code on the host machine.

### Design Principles

1. **Host-native execution.** The agent operates directly on the host filesystem via the Oxios
   daemon. No sandbox, no container — the project directory IS the workspace. Full file path
   access, per user's explicit instruction.
2. **GUI-first structure.** Exploit what a GUI can do that a CLI cannot: multi-panel layouts,
   inline diffs, interactive file trees, visual project graphs, real-time activity timelines,
   and one-click review workflows.
3. **Specialized agent, not a generalist.** A dedicated `code` persona with a coding-focused
   system prompt, distinct from Oxios's many other agent personas. Laser-focused on software
   engineering.
4. **Leverage the existing daemon.** The Oxios binary already runs the engine, model catalog,
   auth, exec tools, file tools, git layer, and event bus. The coding tab reuses all of this —
   it's a new *surface*, not a new *system*.
5. **Review-centric workflow.** Inspired by Zed's checkpoint + review model: every agent edit
   is reviewable, revertible, and attributable. The human stays in control.

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Browser (React SPA)                       │
│  ┌──────────┬──────────────────────────┬─────────────────┐  │
│  │ File     │  Editor / Diff / Canvas  │  Agent Panel    │  │
│  │ Explorer │  (CodeMirror 6)          │  (Conversation  │  │
│  │          │                          │   + Activity    │  │
│  │          ├──────────────────────────┤   + Todos)      │  │
│  │          │  Terminal (xterm.js)     │                 │  │
│  └──────────┴──────────────────────────┴─────────────────┘  │
│  │ Status Bar: branch · changes · agent state · tokens     │  │
└──┬──────────────────────┬──────────────────┬───────────────┘
   │ HTTP /api/code/*      │ WS /api/code/     │ WS /api/code/
   │ (REST CRUD)           │ session/:id/stream│ terminal/:id
   ▼                       ▼                    ▼
┌─────────────────────────────────────────────────────────────┐
│               Oxios Daemon (Rust binary)                     │
│                                                              │
│  code_routes.rs ──▶ CodeApi (new facade)                     │
│    ├── CodeSessionManager    (session lifecycle)             │
│    ├── FileSystemApi         (host FS browse/read/write)     │
│    ├── CheckpointManager     (git-based snapshots)           │
│    ├── ChangeTracker         (diff accumulation)             │
│    ├── PtyManager            (interactive terminals)         │
│    └── CodeAgentRunner       (persona + CSpace + run_goal)   │
│         ├── Persona "code"   (coding system prompt)          │
│         ├── CSpace "coder"   (file+exec+todo+git tools)      │
│         └── AgentRuntime     (existing oxi-sdk loop)         │
│                                                              │
│  Reuses: EngineHandle, EventBridge, AccessManager (disabled),│
│         GitLayer, Project detection                          │
└─────────────────────────────────────────────────────────────┘
```

### Execution Model Decision: In-Process, Not Subprocess

The coding agent runs **within the Oxios daemon process**, not as a spawned subprocess. Rationale:

- The daemon already has the engine, model catalog, API keys, and auth wired.
- `AgentRuntime::run_agent` builds a fresh oxi-sdk `Agent` per execution — no process overhead.
- The agent's file/shell tools operate directly on the host filesystem via the daemon's
  process — same as how Oxios already edits files.
- Terminal sessions use PTY (new `PtyManager`) which spawns shell processes, but these are
  managed by the daemon, not a separate agent process.

The agent and terminal processes are decoupled: the agent uses `ExecTool` (one-shot commands)
while the terminal uses `PtyManager` (interactive PTY sessions). Both run on the host.

---

## 3. Backend Design

### 3.1 New Persona: `code`

A coding-specialized persona stored in `share/default-skills/` or created programmatically.

**System prompt characteristics:**
- Identifies as a senior software engineer pair-programming with the user
- Prioritizes understanding before action: reads files, explores structure, then proposes
- Generates structured todos before implementing
- Applies changes surgically (edit, not rewrite)
- Runs tests/build after changes
- Explains decisions concisely
- Asks for clarification on ambiguous requirements

**Role:** `code` (new role value, maps to the `coder` CSpace template in `capability/resolve.rs`)

### 3.2 New CSpace Template: `coder`

Defined in `crates/oxios-kernel/src/capability/template.rs`:

```
coder:
  Always-on (gated):
    - read, write, edit, grep, find, ls  (SDK file tools)
    - web_search, get_search_results     (SDK web tools)
  CSpace-driven:
    - exec                               (shell mode, full access)
    - todo                               (SDK TodoTool — task tracking)
    - git                                (new GitTool wrapping GitLayer)
  NOT included:
    - memory, persona, cron, security, budget, a2a, knowledge,
      marketplace, skill_forge, calendar, email, image_gen, mcp
```

The coding agent gets a **minimal, focused toolset** — file operations, shell execution,
task tracking, and git. No access to Oxios's general-purpose domain tools.

**Access policy:** `AccessManager` path sandboxing is **disabled** for the `code` persona.
The agent operates with full host filesystem access within the selected project root.
The `GatedTool` wrapper's `ApprovalGate` defaults to **auto-approve** for file operations
within the project root, but `exec` retains interactive approval (user can configure).

### 3.3 New Kernel Facade: `CodeApi`

File: `crates/oxios-kernel/src/kernel_handle/coding_api.rs`

```rust
pub struct CodeApi {
    sessions: Arc<DashMap<String, CodeSession>>,
    pty_manager: Arc<PtyManager>,
    git: GitLayer,
}
```

Holds active coding sessions, terminal manager, and git operations.

### 3.4 New API Routes: `/api/code/*`

File: `src/api/routes/coding_routes.rs`

#### Sessions
| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/code/sessions` | Create a coding session (project_path, model) |
| `GET` | `/api/code/sessions` | List active sessions |
| `GET` | `/api/code/sessions/:id` | Get session state (files, changes, todos, checkpoints) |
| `DELETE` | `/api/code/sessions/:id` | End session |
| `WS` | `/api/code/sessions/:id/stream` | Agent activity stream (tokens, tool calls, diffs) |

#### File Operations (host filesystem)
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/code/fs/browse?path=` | List directory contents (with file type, size, git status) |
| `GET` | `/api/code/fs/read?path=` | Read file content (with language detection) |
| `PUT` | `/api/code/fs/write?path=` | Write file content (raw body) |
| `POST` | `/api/code/fs/create` | Create file or directory `{path, type: "file"\|"dir"}` |
| `DELETE` | `/api/code/fs/delete?path=` | Delete file or directory |
| `POST` | `/api/code/fs/move` | Move/rename `{from, to}` |
| `POST` | `/api/code/fs/search` | Search file contents `{path, query, regex?}` |

#### Agent Messaging
| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/code/sessions/:id/message` | Send user message to coding agent |
| `POST` | `/api/code/sessions/:id/interrupt` | Interrupt agent execution |
| `POST` | `/api/code/sessions/:id/compact` | Compact conversation context |

#### Checkpoints
| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/code/sessions/:id/checkpoint` | Create checkpoint (git stash/commit snapshot) |
| `GET` | `/api/code/sessions/:id/checkpoints` | List checkpoints |
| `POST` | `/api/code/sessions/:id/checkpoint/:cp/revert` | Revert to checkpoint |

#### Changes Review
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/code/sessions/:id/changes` | List pending file changes (diffs) |
| `POST` | `/api/code/sessions/:id/changes/:file/accept` | Accept a file change |
| `POST` | `/api/code/sessions/:id/changes/:file/reject` | Reject a file change (revert) |
| `POST` | `/api/code/sessions/:id/changes/accept-all` | Accept all changes |
| `POST` | `/api/code/sessions/:id/changes/reject-all` | Reject all changes (revert all) |

#### Terminal
| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/code/sessions/:id/terminal` | Create a new PTY terminal |
| `WS` | `/api/code/terminal/:tid` | Terminal I/O stream (bidirectional) |
| `DELETE` | `/api/code/terminal/:tid` | Close terminal |

#### Project Intelligence
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/code/sessions/:id/project-info` | Project type, languages, structure summary |
| `GET` | `/api/code/sessions/:id/git-status` | Git branch, status, recent commits |

### 3.5 PTY Terminal Backend

New module: `crates/oxios-kernel/src/pty.rs`

Uses the `portable-pty` crate for cross-platform PTY support.

```rust
pub struct PtyManager {
    sessions: DashMap<String, PtySession>,
}

struct PtySession {
    id: String,
    pty: Box<dyn Master + Send>,
    reader_handle: JoinHandle<()>,
    project_path: PathBuf,
}

impl PtyManager {
    pub fn create(project_path: &Path, shell: Option<&str>) -> Result<String>;
    pub fn write(&self, id: &str, data: &[u8]) -> Result<()>;
    pub fn resize(&self, id: &str, rows: u16, cols: u16) -> Result<()>;
    pub fn kill(&self, id: &str) -> Result<()>;
    pub fn subscribe(&self, id: &str) -> broadcast::Receiver<Vec<u8>>;
}
```

Each PTY session:
- Spawns the user's default shell (`$SHELL` or `/bin/zsh` on macOS)
- CWD set to the project root
- Reads PTY output on a background task, broadcasts to subscribers via tokio `broadcast`
- WebSocket endpoint relays bidirectional data: client → `PtyManager::write`, PTY → client

### 3.6 Checkpoint System

Uses git for snapshots. Requires the project to be a git repository (or auto-initializes one).

**On agent edit:**
1. Before the agent modifies a file, the `ChangeTracker` records the original content
2. The change is staged as a "pending change" with a diff
3. The user can review, accept (keep), or reject (revert) each change

**Checkpoint creation:**
1. `git stash create` → captures current working tree state
2. Checkpoint ID = stash hash
3. Stored in session state with timestamp and description

**Revert to checkpoint:**
1. `git stash apply <hash>` → restores file state
2. Clears all pending changes after the checkpoint

### 3.7 Agent Execution Flow

The coding agent uses the existing `AgentRuntime` with specialized configuration:

```
User sends message
  → POST /api/code/sessions/:id/message
  → CodeAgentRunner::send_message(session_id, content)
    → Bridge sends IncomingMessage with:
       - persona: "code"
       - cspace_hint: "coder"
       - workspace_dir: project_path
       - session_id: coding_session_id
       - model_override: session.model
    → Gateway → Orchestrator → AgentRuntime::run_agent
       - resolve_cspace("coder") → coder template
       - build_system_prompt with "code" persona
       - register file tools + exec + todo + git
       - run_streaming → WS chunks flow to /api/code/sessions/:id/stream
    → Tool calls (read/write/edit/exec) fire events → broadcast → WS
    → File changes tracked by ChangeTracker
  → Response streams back over WS
```

The WS stream reuses the existing chunk protocol (`token`, `phase`, `tool_start`, `tool_end`,
`tool_progress`, `usage`, `reasoning`, `done`, `error`) with coding-specific additions:
- `file_change`: `{path, action: "create"|"modify"|"delete", diff}`
- `todo_update`: `{todos: [{text, status}]}`
- `checkpoint`: `{id, description}`

---

## 4. Frontend Design

### 4.1 Navigation Integration

Add "Code" as the fourth top-level mode:

**Files to modify:**
1. `web/src/stores/sidebar.ts` — add `'code'` to `SidebarMode` union, update `deriveSidebarMode`
2. `web/src/components/layout/mode-tabs.tsx` — add Code entry to `SIDEBAR_MODES` (icon: `Code2`)
3. `web/src/hooks/use-tab-shortcuts.ts` — extend `Digit1..Digit3` to include `Digit4`
4. `web/src/components/layout/sidebar.tsx` — add `CodeNav` component for sidebar content
5. `web/src/components/layout/app-layout.tsx` — add `isCode` branch (immersive full-height, like chat)
6. `web/src/components/layout/bottom-nav.tsx` — auto-includes via `SIDEBAR_MODES`
7. i18n keys in `web/src/i18n/locales/{en,ko}.json`

**Keyboard shortcut:** `⌃4` (Control+4) — follows the existing `⌃1`/`⌃2`/`⌃3` pattern.

### 4.2 Route Structure

File-based routing (auto-generated by TanStackRouterVite):

```
web/src/routes/code/
├── $sessionId.tsx          # Active coding session (full IDE layout)
└── index.tsx               # Session picker / project launcher
```

- `/code` → Session picker (recent projects, open directory, new session)
- `/code/:sessionId` → Full IDE workspace

### 4.3 Layout Architecture

The coding workspace is a **multi-panel resizable layout** filling the entire viewport
(below the header, right of the sidebar). No scroll on the outer container — each panel
manages its own scroll.

```
┌─────────────────────────────────────────────────────────────────┐
│ Workspace Header (session title, model selector, actions)        │
├──────────┬───────────────────────────────┬─────────────────────┤
│          │                               │                     │
│  File    │   Editor Tabs                 │   Agent Panel       │
│  Explorer│   ┌─────┬──────┬─────────┐   │   ┌───────────────┐ │
│          │   │Code │Diff  │Canvas   │   │   │ Conversation  │ │
│  ├─ src  │   └─────┴──────┴─────────┘   │   │               │ │
│  │  ├─…  │                               │   │ (messages,    │ │
│  ├─ test │   CodeMirror 6 Editor         │   │  tool calls,  │ │
│  │       │   (or Diff View, or Canvas)   │   │  diffs,       │ │
│  ├─ Cargo│                               │   │  todos)       │ │
│  │       │                               │   │               │ │
│  ├─ .git │                               │   │               │ │
│  │       ├───────────────────────────────┤   ├───────────────┤ │
│  │       │  Terminal Panel (xterm.js)    │   │  Todos Panel  │ │
│          │  (collapsible, resizable)     │   │  (checklist)  │ │
├──────────┴───────────────────────────────┴───┴───────────────┤
│ Status Bar: ●main │ 3 changes │ Agent: idle │ 12.4k tokens    │
└─────────────────────────────────────────────────────────────────┘
```

**Panel layout system:** CSS Grid with `grid-template-columns` / `grid-template-rows` and
drag handles for resizing. Stores panel sizes in a Zustand store (persisted to localStorage).
Uses a library like `react-resizable-panels` (Allotment-style) for robust resize behavior.

**Panel states:**
- All visible (default)
- Explorer collapsed (editor + agent)
- Agent collapsed (editor + explorer)
- Terminal expanded (editor shrinks)
- Zen mode (agent only, for focused conversation)

### 4.4 Component Architecture

```
web/src/components/code/
├── workspace/
│   ├── code-workspace.tsx       # Root layout orchestrator
│   ├── workspace-header.tsx     # Title, model, run, checkpoint
│   ├── workspace-status-bar.tsx # Bottom status bar
│   ├── panel-layout.tsx         # Resizable panel container
│   └── panel-tabs.tsx           # Editor tab bar
├── explorer/
│   ├── file-explorer.tsx        # File tree (virtualized)
│   ├── file-tree-node.tsx       # Individual tree node
│   ├── file-actions.tsx         # Context menu (new, delete, rename)
│   ├── project-picker.tsx       # Directory browser for session start
│   └── git-status-badge.tsx     # Modified/untracked indicators
├── editor/
│   ├── code-editor.tsx          # CodeMirror 6 wrapper
│   ├── diff-viewer.tsx          # Unified/split diff (CM6 merge)
│   ├── editor-tab.tsx           # Tab with close button, dirty indicator
│   ├── breadcrumb-bar.tsx       # Path breadcrumb above editor
│   └── search-panel.tsx         # In-editor find/replace (CM6 search)
├── canvas/
│   ├── project-canvas.tsx       # ReactFlow project visualization
│   ├── file-node.tsx            # File/module node in graph
│   ├── dependency-edge.tsx      # Dependency relationship
│   └── activity-overlay.tsx     # Real-time agent activity on nodes
├── agent/
│   ├── agent-panel.tsx          # Right panel container
│   ├── conversation-view.tsx    # Message timeline (virtualized)
│   ├── message-bubble.tsx       # Individual message
│   ├── tool-call-card.tsx       # Tool invocation visualization
│   ├── diff-preview.tsx         # Inline diff in conversation
│   ├── todo-list.tsx            # Agent's task checklist
│   ├── agent-input.tsx          # Message input with @-mentions
│   ├── context-chips.tsx        # Attached context files
│   └── model-selector.tsx       # Model picker (reuses engine API)
├── terminal/
│   ├── terminal-panel.tsx       # Terminal container
│   ├── terminal-view.tsx        # xterm.js instance
│   └── terminal-tabs.tsx        # Multiple terminal tabs
├── review/
│   ├── review-bar.tsx           # "N files changed" accordion bar
│   ├── review-diff.tsx          # Full diff review with accept/reject
│   └── checkpoint-list.tsx      # Checkpoint timeline
└── stores/
    ├── code-session.ts          # Session state (messages, changes, todos)
    ├── code-editor.ts           # Open tabs, active file, cursor
    ├── code-terminal.ts         # Terminal sessions
    └── code-layout.ts           # Panel sizes, visibility
```

### 4.5 File Explorer

A virtualized file tree using the existing `virtua` library (already a dependency).

**Features:**
- Lazy-load directory contents (expand on click, API call to `/api/code/fs/browse`)
- Git status indicators (modified = amber dot, untracked = green dot, staged = blue dot)
- File icons by extension (using `lucide-react` icons — already available)
- Context menu: New File, New Folder, Rename, Delete, Copy Path, Open in Terminal
- Search files by name (fuzzy match, like VS Code's Quick Open)
- Drag-and-drop file moves (optional, phase 2)
- Project root selector at the top (path breadcrumb with home/quick-access)

**Tree node states:**
- Default: folder/file icon + name
- Active (open in editor): highlighted background
- Modified by agent: pulsing animation (`animate-file-blink` — already in CSS)
- Currently being read by agent: blue ring pulse
- Currently being written by agent: amber ring pulse

### 4.6 Code Editor

CodeMirror 6 — the project already has full CM6 integration (`@codemirror/*` packages).

**Configuration:**
- Language: auto-detect from file extension (using `@codemirror/language-data`)
- Theme: `one-dark` (already installed) or custom oxi-themed dark theme
- Extensions: line numbers, code folding, bracket matching, autocomplete, search
- Read-only mode when file is being modified by agent (lock indicator)
- Dirty indicator (●) on tab when file has unsaved changes

**Agent interaction:**
- When agent reads a file → file opens in a read-only tab with a "scanning" indicator
- When agent writes a file → diff appears inline in the conversation AND the editor tab
  shows the new content with a "modified" badge
- User can edit files directly — changes are saved on `Cmd+S`

**Editor tabs:**
- Pinned tabs (left side), modified tabs (middle), preview tabs (italic, replaced on next open)
- Close button with dirty-state confirmation
- Tab overflow: horizontal scroll or dropdown menu

### 4.7 Diff View

Two modes:
1. **Inline diff** (in conversation) — compact unified diff preview when agent makes changes
2. **Full diff review** (in editor area) — split-pane or unified view with accept/reject buttons

Implementation: `@codemirror/merge` (official CM6 merge addon) for the full diff view,
and a lightweight custom diff renderer for inline previews (using the `diff` npm package
to compute hunks).

**Review workflow:**
1. Agent makes changes → "3 files changed" bar appears above the agent input
2. Click the bar → opens review mode with all changed files
3. Each file shows a diff with per-hunk accept/reject
4. "Accept All" / "Reject All" buttons
5. Accepted changes are kept; rejected changes are reverted (git checkout)

### 4.8 Canvas View

An interactive project visualization using **ReactFlow** (already a dependency, v11.11.4).

**Purpose:** Give a high-level structural overview of the project — files, modules, and their
relationships — with real-time agent activity overlaid.

**Graph construction:**
- Nodes = directories and significant files (grouped by directory)
- Edges = import/dependency relationships (parsed from source)
- Layout = hierarchical (top-down) or force-directed (user toggle)
- Node appearance = file type color-coded (using the 6-hue label palette)

**Agent activity overlay:**
- When agent reads a file → corresponding node gets a blue pulse ring
- When agent writes a file → node gets an amber pulse + diff count badge
- When agent runs a command → command output streams in a floating panel near the node
- Activity trail: last 5 actions visualized as a connected path

**Interaction:**
- Click node → open file in editor
- Double-click directory node → zoom into subdirectory
- Search → highlights matching nodes
- Collapse/expand groups

This is the "killer feature" that distinguishes the GUI from a CLI — seeing the agent's
impact on the codebase structure in real-time.

### 4.9 Agent Panel

The right-side conversation panel, specialized for coding.

**Message types:**
1. **User message** — text with optional @-mentions (files, code selections)
2. **Agent text** — markdown rendered (reuse existing `react-markdown` + `rehype-highlight`)
3. **Tool call cards** — compact visualizations:
   - `read(file.rs)` → card with file icon, line count, "read" badge
   - `edit(file.rs, L42-58)` → card with inline diff preview (expandable)
   - `exec(cargo test)` → card with command + collapsible output
   - `grep("pattern")` → card with match count + preview
   - `todo(plan)` → checklist card with items
4. **Phase indicators** — "Thinking...", "Reading files...", "Editing...", "Running tests..."

**Conversation features:**
- Virtualized list (using `virtua`) for performance with long conversations
- Message queuing (user can type while agent is working — queued messages sent on completion)
- Message editing (click any user message to edit and re-submit)
- Stop button (interrupts agent)
- Context attachment via @-mention (files, code selections, terminal output)

**Todo checklist:**
- Agent generates a structured todo list before implementing
- Each item has status: pending → in_progress → done
- Real-time updates as agent works through tasks
- User can check/uncheck items to guide the agent

### 4.10 Terminal Panel

**xterm.js** integration (`@xterm/xterm` + `@xterm/addon-fit` + `@xterm/addon-web-links`).

**Features:**
- Multiple terminal tabs (create/close)
- CWD = project root
- Resizable (fits container, addon-fit handles dimensions)
- Agent command output appears inline (shared terminal, or agent uses its own invisible exec)
- Terminal title shows current shell + CWD
- Theme matches the oxi dark theme
- Web links are clickable (addon-web-links)
- Copy/paste support
- Resize → sends new dimensions to backend PTY

**Layout:** Bottom panel, collapsible. Default height ~200px. User can drag to resize.
When collapsed, shows a thin bar with terminal icon + last command status.

### 4.11 Session Picker (`/code`)

The landing page when navigating to the Code tab without an active session.

**Layout:**
- Centered card with:
  - "Open Project" — directory browser (host filesystem picker)
  - "Recent Projects" — list of recently opened project paths (from session history)
  - "Quick Start" — common directories (home, Documents, Projects)
- Each project card shows: path, language icon, last session time, "Resume" button
- Model selector at the top (defaults to user's preferred model)

### 4.12 Design System Compliance

All components follow the **Oxi Design System v1.0**:
- Semantic Tailwind utilities only (`bg-surface`, `text-text`, `border-line`)
- OKLCH colors from the token system
- `Geist Mono` for code/terminal, `SUIT Variable` for UI text
- No `dark:` variants in component code
- Reuse existing UI components (Button, Card, Tabs, Badge, Tooltip, ScrollArea, Separator)
- Status colors for agent states (blue=in-progress, green=done, amber=modified, red=error)

---

## 5. Data Flow

### 5.1 Session Lifecycle

```
1. User opens /code → Session picker
2. User selects project directory → POST /api/code/sessions {project_path, model}
   → Backend validates path exists, detects project type
   → Creates CodeSession with unique ID
   → Initializes ChangeTracker, checkpoint baseline (git stash)
   → Returns session_id
3. Frontend navigates to /code/:sessionId → mounts CodeWorkspace
4. WebSocket connects to /api/code/sessions/:id/stream
5. File explorer loads root directory via GET /api/code/fs/browse
6. Agent panel ready for user input
```

### 5.2 Agent Interaction Cycle

```
User types message + optionally attaches @files
  → POST /api/code/sessions/:id/message {content, context_files: [...]}
  → Backend builds IncomingMessage with persona="code", workspace=project_path
  → Bridge → Gateway → Orchestrator → AgentRuntime
  → Agent streams response via WS:
     ├── token chunks → conversation appends
     ├── tool_start {tool: "read", args: {path: "src/main.rs"}}
     │   → File explorer highlights src/main.rs (blue pulse)
     │   → Editor opens file in read-only "preview" tab
     ├── tool_end {tool: "read", result_summary: "450 lines"}
     ├── tool_start {tool: "edit", args: {path: "src/main.rs", lines: "42-58"}}
     │   → Editor shows the file
     ├── file_change {path: "src/main.rs", diff: "..."}
     │   → ChangeTracker records the change
     │   → Review bar updates: "1 file changed"
     │   → Editor tab shows dirty indicator
     │   ├── todo_update {todos: [{text: "Fix bug", status: "done"}, ...]}
     │   │   → Todo panel updates
     ├── tool_start {tool: "exec", args: {cmd: "cargo test"}}
     │   → Terminal panel shows command running
     ├── tool_end {tool: "exec", exit_code: 0, output: "..."}
     └── done
  → User reviews changes, accepts/rejects
```

### 5.3 File Change Tracking

The `ChangeTracker` (backend) maintains a per-session list of file changes:

```rust
struct FileChange {
    path: PathBuf,
    action: ChangeAction,          // Create | Modify | Delete
    original_content: Option<String>,
    new_content: Option<String>,
    diff: String,                  // Unified diff
    timestamp: DateTime<Utc>,
    accepted: bool,
    tool_call_id: Option<String>,  // Which agent tool call caused it
}
```

Before the agent modifies a file (via the `WriteTool`/`EditTool`), the tracker snapshots
the original content. After the write, it computes the diff and emits a `file_change` event.

**Accept:** Marks the change as accepted, keeps the file content.
**Reject:** Reverts the file to `original_content` (or deletes if it was a creation).

### 5.4 Checkpoint Flow

```
Agent completes a unit of work
  → POST /api/code/sessions/:id/checkpoint {description: "Added login form"}
  → Backend: git stash create → checkpoint_id
  → Checkpoint stored with timestamp, description, file list
  → WS event: checkpoint {id, description}

Later, user wants to revert:
  → POST /api/code/sessions/:id/checkpoint/:cp/revert
  → Backend: git checkout <files at checkpoint state>
  → All changes after checkpoint are discarded
  → File explorer refreshes, editor tabs update
```

---

## 6. New Dependencies

### Frontend (npm)
| Package | Version | Purpose |
|---------|---------|---------|
| `@xterm/xterm` | `^5.5.0` | Terminal emulation |
| `@xterm/addon-fit` | `^0.10.0` | Terminal auto-resize |
| `@xterm/addon-web-links` | `^0.11.0` | Clickable links in terminal |
| `@codemirror/merge` | `^6.8.0` | Diff/merge view in editor |
| `diff` | `^7.0.0` | Diff computation for inline previews |
| `react-resizable-panels` | `^2.1.0` | Resizable panel layout |
| `fuzzysearch` | `^3.2.1` | Fuzzy file search |

### Backend (crates.io)
| Crate | Version | Purpose |
|-------|---------|---------|
| `portable-pty` | `^0.8` | Cross-platform PTY for terminal |

All other backend functionality reuses existing crates (tokio, axum, serde, git2).

---

## 7. File Structure (New Files)

### Backend
```
crates/oxios-kernel/src/
├── kernel_handle/
│   └── coding_api.rs              # CodeApi facade
├── pty.rs                         # PtyManager (PTY terminal sessions)
├── code/
│   ├── mod.rs                     # Module exports
│   ├── session.rs                 # CodeSession struct + lifecycle
│   ├── change_tracker.rs          # FileChange tracking + diff computation
│   ├── checkpoint.rs              # Git-based checkpoint system
│   └── runner.rs                  # CodeAgentRunner (persona + CSpace wiring)

src/api/routes/
└── coding_routes.rs               # All /api/code/* handlers

share/default-skills/
└── code-persona.md                # Coding persona system prompt

crates/oxios-kernel/src/capability/
└── template.rs                    # (modified — add "coder" template)
```

### Frontend
```
web/src/
├── routes/code/
│   ├── index.tsx                  # Session picker
│   └── $sessionId.tsx             # Active workspace
├── components/code/               # (see §4.4 for full tree)
│   ├── workspace/
│   ├── explorer/
│   ├── editor/
│   ├── canvas/
│   ├── agent/
│   ├── terminal/
│   ├── review/
│   └── ...
├── stores/code/
│   ├── code-session.ts            # Session state
│   ├── code-editor.ts             # Open tabs, active file
│   ├── code-terminal.ts           # Terminal sessions
│   └── code-layout.ts             # Panel sizes
├── hooks/code/
│   ├── use-code-ws.ts             # WebSocket connection for coding session
│   ├── use-file-tree.ts           # Lazy-load file tree data
│   ├── use-code-agent.ts          # Agent messaging + streaming
│   └── use-terminal.ts            # Terminal WebSocket management
└── types/code.ts                  # TypeScript types for coding domain
```

---

## 8. Implementation Phases

### Phase 1: Foundation (Backend)
1. Add `portable-pty` dependency to `oxios-kernel`
2. Create `CodeApi` facade + `CodeSession` struct
3. Create `PtyManager` for terminal sessions
4. Create `ChangeTracker` for file change tracking
5. Create `CheckpointManager` (git-based)
6. Add "coder" CSpace template to `capability/template.rs`
7. Add "code" persona (system prompt)
8. Wire up `CodeAgentRunner` using existing `AgentRuntime`
9. Create `coding_routes.rs` with all endpoints
10. Register routes in `build_routes`

### Phase 2: Foundation (Frontend)
1. Add "Code" mode to navigation (sidebar, mode-tabs, shortcuts)
2. Create session picker route (`/code`)
3. Create workspace route (`/code/:sessionId`)
4. Build panel layout system (react-resizable-panels)
5. Create Zustand stores (session, editor, terminal, layout)
6. Create TypeScript types
7. Build WebSocket hook for session streaming

### Phase 3: Core Panels
1. **File Explorer** — lazy-load tree, git status, context menu
2. **Code Editor** — CM6 wrapper, tabs, language detection
3. **Agent Panel** — conversation view, message input, tool call cards
4. **Status Bar** — git branch, changes, agent state, tokens

### Phase 4: Terminal & Diff
1. **Terminal** — xterm.js + WebSocket PTY
2. **Diff View** — CM6 merge for full review, inline diff for previews
3. **Review System** — change tracking UI, accept/reject flow

### Phase 5: Advanced Features
1. **Canvas** — ReactFlow project visualization with agent activity overlay
2. **Checkpoints** — checkpoint timeline, revert flow
3. **Todo System** — agent todo checklist with real-time updates
4. **Context Attachments** — @-mention files, drag-and-drop
5. **Search** — project-wide file search and content search

### Phase 6: Polish
1. Keyboard shortcuts (panel focus, file navigation, terminal toggle)
2. Theme refinement (oxi-themed CM6, terminal theme)
3. Performance optimization (virtualization, lazy loading)
4. Error handling and edge cases
5. i18n (Korean + English)
6. Testing

---

## 9. Testing Strategy

### Backend
- Unit tests for `ChangeTracker` (diff computation, accept/reject)
- Unit tests for `CheckpointManager` (git operations)
- Integration test for `PtyManager` (create, write, read, kill)
- Integration test for coding session lifecycle (create → message → changes → checkpoint → revert)
- Integration test for file operations (browse, read, write, search)

### Frontend
- Component tests for file explorer (tree rendering, expansion, context menu)
- Component tests for agent panel (message rendering, tool call cards, streaming)
- Store tests for session state management
- E2E test for full workflow (open project → send message → review changes → accept)

---

## 10. Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Full filesystem access = security risk | High | Explicitly requested by user. Mitigate by: scoping agent to selected project root, logging all file ops to audit trail, requiring session-scoped access (not global). |
| PTY on different platforms | Medium | `portable-pty` handles cross-platform. Test on macOS (primary target). |
| Large file tree performance | Medium | Virtualized tree (`virtua`), lazy-load directories, cache results. |
| WebSocket connection management | Medium | Reuse existing WS patterns from chat. Add reconnect logic, session resumption. |
| Git not initialized | Low | Auto-detect; suggest `git init` in UI. Checkpoints fall back to file snapshots. |
| Agent modifies file while user is editing | Medium | Lock files being modified by agent (read-only indicator). Queue user saves until agent completes. |
| xterm.js bundle size | Low | Lazy-load terminal component (dynamic import). |

---

## 11. Open Questions (Autonomous Decisions)

Since the user is unavailable for clarification, the following decisions were made autonomously:

1. **In-process vs subprocess agent** → **In-process.** Leverages existing daemon infrastructure, no process management overhead.
2. **Terminal approach** → **PTY via `portable-pty`.** Provides true interactive terminal (not just one-shot exec). The agent's `ExecTool` remains separate (one-shot commands); the terminal is for user interaction and agent command visibility.
3. **Diff rendering** → **`@codemirror/merge`** for full review + `diff` package for inline previews. Consistent with existing CM6 integration.
4. **Checkpoint mechanism** → **Git-based** (stash/checkout). Leverages existing `GitLayer`. Requires git repo (auto-suggest init if absent).
5. **Canvas framework** → **ReactFlow** (already installed). Provides drag, zoom, pan, custom nodes out of the box.
6. **Panel layout** → **`react-resizable-panels`**. Purpose-built for IDE-style layouts, handles collapse/expand, persists sizes.
7. **Keyboard shortcut** → **`⌃4`** (Control+4). Follows existing `⌃1`/`⌃2`/`⌃3` convention.
8. **Access policy** → **Full host filesystem access** within selected project root. `AccessManager` path sandboxing disabled for `code` persona. Exec approval configurable (default: auto-approve for trusted commands, prompt for others).

---

## Appendix A: Competitive Analysis Summary

| Feature | Cursor | Zed Agent | Windsurf | **Oxios Code (proposed)** |
|---------|--------|-----------|----------|---------------------------|
| Agent panel | ✅ | ✅ | ✅ | ✅ |
| Inline diff review | ✅ | ✅ | ✅ | ✅ |
| Terminal | ✅ | ✅ | ✅ | ✅ (PTY) |
| File explorer | ✅ | ✅ | ✅ | ✅ |
| Checkpoints/revert | ✅ | ✅ | ❌ | ✅ (git-based) |
| Todo tracking | ❌ | ❌ | ✅ | ✅ |
| Context @-mentions | ✅ | ✅ | ✅ | ✅ |
| Model switching | ✅ | ✅ | ✅ | ✅ (engine catalog) |
| Project canvas/graph | ❌ | ❌ | ❌ | ✅ (ReactFlow) |
| Multi-agent visibility | ❌ | ✅ | ✅ | Phase 2 |
| Browser-based | ❌ | ❌ | ❌ | ✅ |
| Host-native exec | ✅ | ✅ | ✅ | ✅ |

**Key differentiator:** Browser-based (accessible anywhere) + project canvas visualization + integrated with the Oxios agent ecosystem.

---

## Appendix B: WS Chunk Protocol Extensions

New chunk types for the coding session stream (extending the existing protocol):

```typescript
// File change event
{ type: "file_change", path: string, action: "create"|"modify"|"delete", diff: string }

// Todo update event
{ type: "todo_update", todos: Array<{ id: string, text: string, status: "pending"|"in_progress"|"done" }> }

// Checkpoint event
{ type: "checkpoint", id: string, description: string, files: string[] }

// Agent file activity (for explorer highlighting)
{ type: "file_activity", path: string, activity: "reading"|"writing"|"deleting" }

// Terminal output relay (agent's exec output to shared terminal)
{ type: "terminal_output", terminal_id: string, data: string }
```

These are additive — existing chunk types (`token`, `phase`, `tool_start`, `tool_end`,
`usage`, `reasoning`, `done`, `error`) continue unchanged.
