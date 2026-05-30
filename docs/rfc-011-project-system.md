# RFC-011: Project System

> **Status:** Approved (v2 — comprehensive)
> **Date:** 2025-05-30
> **Replaces:** Space system (full removal)
> **Revisions:** v2 adds complete touch-point inventory, memory migration decision, API redesign, 30-step migration plan

## Summary

Replace the existing Space system with a Project system. Projects are user-registered or auto-detected work contexts (code projects, blog writing, travel planning — any topic). Sessions reference Projects via `primary_project_id` + `secondary_project_ids`, and memories link to Projects through a `project_memory` junction table (N:M).

## Motivation

### Problems with Space

1. **Session ownership is inverted.** Space assumes sessions belong to a Space (1:1). But "recommend me lunch" shouldn't need a Space at all.
2. **Auto-detection overreach.** Space's 3-layer detection tries to classify every conversation into a named partition. Simple project aliasing is more practical.
3. **No multi-project support.** A session working on `oxios` often needs `oxi` and `oxibrowser` context too. Space only allows one active partition.
4. **Unused complexity.** Space merge, archive, restore, cross-Space memory bridge — all infrastructure for a model that doesn't match real usage patterns.

### What Project solves

- **Explicit registration**: User registers `oxios` → `/Volumes/MERCURY/PROJECTS/oxios`. Agent knows the path immediately.
- **Session-centric**: Session has Projects, not the other way around. Project-less sessions are natural.
- **Multi-project**: One session = 1 primary + N secondary projects. Cross-repo work is first-class.
- **Memory association**: Project ↔ Memory is N:M via junction table, not ownership.

## Design

### Data Model

```
┌─────────────┐       N:M (junction)       ┌──────────────┐
│   Project   │ ◄─────────────────────────► │    Memory    │
└──────┬──────┘                             └──────────────┘
       │
       │ 1:N (session meta)
       ▼
┌─────────────────────────────────┐
│         Session                 │
│ primary_project_id: Option      │
│ secondary_project_ids: Vec      │
└─────────────────────────────────┘
```

### Project

```rust
pub struct Project {
    pub id: ProjectId,              // Uuid
    pub name: String,               // "oxios", "pi", "game1"
    pub description: String,        // Optional human-readable description
    pub paths: Vec<PathBuf>,        // Filesystem paths (empty for non-code)
    pub tags: Vec<String>,          // Keywords for AI detection
    pub emoji: String,              // Display emoji
    pub source: ProjectSource,      // Manual | AutoDetected
    pub memory_visible: bool,       // Allow cross-project memory access
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
}

pub enum ProjectSource {
    Manual,         // User explicitly created via UI/CLI
    AutoDetected,   // OS auto-detected from path in conversation
}
```

> **Note:** `AiSuggested` removed from Phase 1. Deferred to Phase 2 when AI suggestion flow is designed.

**Key differences from Space:**
- `paths` can be empty (non-code projects like "travel planning")
- No `active` flag — activity is per-session, not per-project
- No `workspace_dir` field — scratch files go to `~/.oxios/projects/{id}/workspace/` (auto-created)
- No `interaction_count` — tracked per-session, not per-project

### Session-Project Association

```rust
pub struct SessionContext {
    pub recall_timing: Option<RecallTiming>,
    pub primary_project_id: Option<ProjectId>,
    pub secondary_project_ids: Vec<ProjectId>,
}
```

- `primary_project_id = None`: free-floating session (no project context)
- Primary project's `paths[0]` becomes the AgentRuntime CWD
- Secondary projects provide additional context/memory for cross-project work

### Project-Memory Junction Table

```sql
CREATE TABLE IF NOT EXISTS project_memory (
    project_id TEXT NOT NULL,
    memory_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (project_id, memory_id)
);
CREATE INDEX IF NOT EXISTS idx_pm_project ON project_memory(project_id);
CREATE INDEX IF NOT EXISTS idx_pm_memory ON project_memory(memory_id);
```

- Pure N:M relationship
- Memories exist independently; projects reference them
- A single memory can be associated with multiple projects
- Query: "all memories for project X" → `JOIN project_memory WHERE project_id = ?`

### Memory `space_id` Column Decision

The existing `memories` table has a `space_id TEXT` column. Three options were evaluated:

| Option | Decision | Rationale |
|--------|----------|-----------|
| Rename `space_id` → `project_id` | ❌ Rejected | Not a1:1 rename — Space→Project is semantic change |
| Remove `space_id` + use junction table only | ⚠️ Decision: **Do this with migration** | Cleanest model; junction table replaces all space_id use |
| Keep `space_id` as primary project | ❌ Rejected | Dual-track (column + junction) is confusing |

**Decision: Remove `space_id` column + use `project_memory` junction table.**

Migration:
1. Read all existing `memories` rows with non-null `space_id`
2. For each unique `space_id`, create a corresponding `project_memory` entry
3. Add `project_memory` table to schema
4. Drop `space_id` column from `memories` table (or leave as NULL with deprecation note)
5. Update `MemoryEntry.space_id` → remove or deprecate

> **Practical note:** For Phase 1, we'll keep `space_id` as a nullable column (backward compat) but stop writing to it. Full removal in Phase 3.

### Project Detection Flow

```
User: "oxios 코드리뷰해줘"
    │
    ▼
1. ProjectManager.lookup("oxios")
   → Match by name, tag, or path
   → Found: Project { paths: ["/Volumes/MERCURY/PROJECTS/oxios"] }
    │
    ▼
2. Set session.primary_project_id = project.id
    │
    ▼
3. AgentRuntime sets CWD = project.paths[0]
    │
    ▼
4. Inject project-associated memories into agent context
```

**Detection layers (simplified from Space's 3-layer):**

1. **Direct name match**: "oxios" → find project with name "oxios"
2. **Path extraction**: "/Volumes/MERCURY/PROJECTS/oxios" → find project with matching path
3. **Tag match**: keywords in message match project tags
4. **AI classification** (Phase 2): LLM determines project from conversation context

### Persistence

**Decision: SQLite table (not file-based JSON).**

Rationale: Projects are few (10-20 typical) but the `project_memory` junction table needs SQL joins. Having projects in SQLite alongside the `memories` table provides:
- Transactional consistency for project_memory operations
- Single query for "project + its memories"
- Schema migration support
- No dual-track data (file + SQLite)

```sql
CREATE TABLE IF NOT EXISTS projects (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    description     TEXT,
    paths           TEXT,            -- JSON array of PathBuf strings
    tags            TEXT,            -- JSON array of strings
    emoji           TEXT NOT NULL DEFAULT '📦',
    source          TEXT NOT NULL DEFAULT 'manual',
    memory_visible  INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    last_active_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_projects_name ON projects(name);
```

Projects stored in `~/.oxios/workspace/memory.db` (same SQLite file as memories).

## API

### Orchestrator API Change

**Before:**
```rust
pub async fn handle_message(
    &self,
    user_id: &str,
    user_message: &str,
    session_id: Option<&str>,
    space_id: Option<&str>,  // ← explicit space override
) -> Result<OrchestrationResult>
```

**After:**
```rust
pub async fn handle_message(
    &self,
    user_id: &str,
    user_message: &str,
    session_id: Option<&str>,
    project_ids: Option<&str>,  // "uuid1,uuid2,uuid3" — primary first
) -> Result<OrchestrationResult>
```

The `OrchestrationResult` changes:
```rust
// Before
pub struct OrchestrationResult {
    pub space_id: Option<Uuid>,
    pub space_tag: Option<String>,
    ...
}

// After
pub struct OrchestrationResult {
    pub primary_project_id: Option<Uuid>,
    pub project_tag: Option<String>,    // e.g. "[🔧 oxios]"
    ...
}
```

**SessionContext creation flow:**
```
1. Web/CLI calls handle_message(session_id, project_ids="uuid1,uuid2")
2. If session_id is new → create SessionContext with project_ids parsed
3. Orchestrator uses SessionContext.primary_project_id for CWD
4. Orchestrator uses SessionContext for project memory injection
5. Response includes primary_project_id + project_tag
```

### CLI

```bash
# CRUD
oxios project list
oxios project add myapp --path ~/projects/myapp --emoji 🔧
oxios project add blog --emoji 📝 --tag "writing" --tag "content"
oxios project remove myapp
oxios project show myapp

# Session project management
oxios run --project oxios "review this code"
oxios run --project oxios --also oxi --also oxibrowser "check API compatibility"
```

### Web API

```
GET    /api/projects              → List all projects
POST   /api/projects              → Create project
GET    /api/projects/:id          → Get project details
PUT    /api/projects/:id          → Update project
DELETE /api/projects/:id          → Remove project
GET    /api/projects/:id/memories → Get project-associated memories
POST   /api/projects/:id/memories → Link memory to project
DELETE /api/projects/:id/memories/:memoryId → Unlink memory
```

### Agent Tool

```json
{
  "name": "project",
  "description": "Query projects — registered work contexts with paths and associated memories",
  "actions": ["list", "get", "link_memory", "unlink_memory"]
}
```

Agents can:
- List available projects (to understand user's workspace)
- Get project details (paths, tags, description)
- Associate/disassociate memories with the session's active projects
- **Cannot** create, update, or remove projects (user-level operations only via CLI/Web)

## Complete Touch-Point Inventory

Space is embedded across **7 layers and 32 files**. Here is the complete inventory:

### Layer 1: Kernel Crate (oxios-kernel)

| File | Type | Space Usage |
|------|------|-------------|
| `lib.rs` | re-exports | `Space`, `SpaceManager`, `SpaceId`, `SpaceManagerError`, `SpaceSource`, `ConversationBuffer`, `ConversationTurn`, `CrossRefEntry`, `MemoryFlow`, `PathMatcher`, `extract_filesystem_path`, `match_keywords`, `SpaceApi` |
| `space.rs` | module | `Space` struct, `SpaceSource` enum, `DEFAULT_SPACE_ID` |
| `space/manager.rs` | module | `SpaceManager` (full implementation) |
| `space/detection.rs` | module | `PathMatcher`, `extract_filesystem_path`, `match_keywords`, `classify_topic_stub` |
| `space/conversation_buffer.rs` | module | `ConversationBuffer`, `ConversationTurn` (has `space_id` field) |
| `space/space_bridge.rs` | module | `SpaceBridge`, `CrossRefEntry`, `MemoryFlow` |
| `kernel_handle/space_api.rs` | module | `SpaceApi` facade |
| `kernel_handle/mod.rs` | module | `spaces: SpaceApi` field in `KernelHandle`, `activate_space()` method |
| `tools/kernel/space_tool.rs` | module | `SpaceTool` (AgentTool wrapper) |
| `tools/kernel/mod.rs` | module | `SpaceTool` in `register_all_kernel_tools()` |
| `tools/kernel_bridge.rs` | module | `"space"` in `tool_names()` |
| `orchestrator.rs` | module | `space_manager`, `conversation_buffer`, `space_id`/`space_tag` throughout `handle_message`, `OrchestrationResult` |
| `event_bus.rs` | module | `SpaceCreated`, `SpaceActivated`, `SpaceArchived`, `SpacesMerged`, `KnowledgeCrossReferenced` events |
| `memory/mod.rs` | module | `MemoryEntry.space_id` field |
| `memory/database.rs` | module | `memories` table has `space_id TEXT` column |
| `memory/sqlite_store.rs` | module | `space_id` in INSERT, UPDATE, SELECT queries |
| `agent_runtime.rs` | module | `AgentRuntimeConfig.space_id` field |

### Layer 2: Kernel Binary (src/)

| File | Type | Space Usage |
|------|------|-------------|
| `kernel.rs` | binary | `space_manager` field, `SpaceManager::new()`, `SpaceApi::new()`, `orchestrator.set_space_manager()`, 15+ references throughout |

### Layer 3: CLI

| File | Type | Space Usage |
|------|------|-------------|
| `cmd_run.rs` | binary | `space_id` and `space_tag` in JSON output |

### Layer 4: Web Backend (surface/oxios-web/src/)

| File | Type | Space Usage |
|------|------|-------------|
| `routes/space_routes.rs` | Axum routes | All 8 space route handlers |
| `routes/mod.rs` | module | `space_routes` module import, 7 route registrations |
| `routes/chat.rs` | Axum routes | `ChatRequest.space_id`, `ChatResponse.space_tag`, metadata storage |
| `routes/events.rs` | SSE | `space_id` in session events, `SpaceActivated`/`SpaceArchived`/`SpaceCreated` event mapping |
| `plugin.rs` | plugin | "activate" comment (low significance) |

### Layer 5: Web Frontend (surface/oxios-web/web/src/)

| File | Type | Space Usage |
|------|------|-------------|
| `types/index.ts` | TS | `Space` interface, `Session.space_id`, `SessionDetail.space_id` |
| `types/memory.ts` | TS | `Memory.space_id` |
| `stores/chat.ts` | Zustand | `activeSpaceId`, `_lastDoneSpaceId`, `setActiveSpace()`, `space_id` in connect/sendMessage/streamMessage |
| `routes/spaces/index.tsx` | React | `SpacesListPage` component |
| `routes/spaces/$spaceId.tsx` | React | Space detail page |
| `components/memory/memory-detail.tsx` | React | `memory.space_id` display |
| `routes/agents/$agentId.tsx` | React | `agent.space_id` display |

**Total: 32 files across 7 layers.**

## Migration Plan

### Phase 1: Core (This RFC)

#### Step 1: Create `project/` module (kernel crate)
- [ ] `crates/oxios-kernel/src/project.rs` — `Project` struct, `ProjectSource` enum, `ProjectId` type alias
- [ ] `crates/oxios-kernel/src/project/manager.rs` — `ProjectManager` (CRUD, lookup, persistence in SQLite)
- [ ] `crates/oxios-kernel/src/project/detection.rs` — simplified detection (name/path/tag match, no LLM)
- [ ] `crates/oxios-kernel/src/project/mod.rs` — module exports

#### Step 2: SQLite schema
- [ ] Add `projects` table to `memory/database.rs`
- [ ] Add `project_memory` junction table
- [ ] Add migration for existing `space_id` → `project_memory` data (Phase 1: keep `space_id` nullable, Phase 3: drop)

#### Step 3: Update `SessionContext`
- [ ] Add `primary_project_id: Option<ProjectId>` field
- [ ] Add `secondary_project_ids: Vec<ProjectId>` field
- [ ] Update serialization/deserialization

#### Step 4: Update `Orchestrator`
- [ ] Replace `space_manager: RwLock<Option<Arc<SpaceManager>>>` with `project_manager: RwLock<Option<Arc<ProjectManager>>>`
- [ ] Change `handle_message(space_id: Option<&str>)` → `handle_message(project_ids: Option<&str>)`
- [ ] Replace `detect_or_create()` with `lookup_or_detect()` returning `Option<ProjectId>`
- [ ] Update `current_space_tag()` → `current_project_tag()`
- [ ] Update `OrchestrationResult`: remove `space_id`/`space_tag`, add `primary_project_id`/`project_tag`
- [ ] Update `ConversationBuffer` usage: replace `SpaceId` with `ProjectId` in `ConversationTurn`
- [ ] Remove `SpaceManager` import and `set_space_manager()` method

#### Step 5: Update `AgentRuntime`
- [ ] Rename `AgentRuntimeConfig.space_id` → `primary_project_id`
- [ ] Update CWD logic: use `primary_project.paths[0]` instead of `space_id`
- [ ] Update `project_paths` to derive from `primary_project_id`

#### Step 6: Update `EventBus`
- [ ] Remove: `SpaceCreated`, `SpaceActivated`, `SpaceArchived`, `SpacesMerged`, `KnowledgeCrossReferenced`
- [ ] Add: `ProjectCreated { project_id, name, source }`, `ProjectActivated { project_id, name }`

#### Step 7: Update `MemoryEntry`
- [ ] Keep `space_id` as nullable for backward compat (Phase 1)
- [ ] Add `project_ids: Vec<String>` field (Phase 3: replaces space_id entirely)
- [ ] Update `sqlite_store.rs` to write both during migration period

#### Step 8: Update `KernelHandle`
- [ ] Replace `spaces: SpaceApi` field with `projects: ProjectApi`
- [ ] Remove `activate_space()` method
- [ ] Add `activate_project()` method
- [ ] Update `kernel_handle/mod.rs` re-exports

#### Step 9: Update `KernelHandle::from_subsystems`
- [ ] Replace `space_manager: Arc<SpaceManager>` parameter with `project_manager: Arc<ProjectManager>`
- [ ] Update `SpaceApi::new()` → `ProjectApi::new()`

#### Step 10: Update `KernelHandle::new`
- [ ] Replace `spaces: SpaceApi` parameter with `projects: ProjectApi`

#### Step 11: Update `kernel_bridge.rs`
- [ ] Remove `"space"` from `tool_names()`
- [ ] Add `"project"` to `tool_names()`
- [ ] Replace `SpaceManager::new()` with `ProjectManager::new()` in test

#### Step 12: Create `ProjectTool`
- [ ] `crates/oxios-kernel/src/tools/kernel/project_tool.rs` — new tool (list, get, link_memory, unlink_memory)
- [ ] Update `tools/kernel/mod.rs`: replace `SpaceTool` with `ProjectTool` in `register_all_kernel_tools()`

#### Step 13: Update `lib.rs` exports
- [ ] Remove all `space::` re-exports
- [ ] Add `project::` re-exports: `Project`, `ProjectId`, `ProjectManager`, `ProjectSource`
- [ ] Add `ProjectApi` to kernel_handle re-exports
- [ ] Remove `SpaceApi` from re-exports

#### Step 14: Update binary (src/kernel.rs)
- [ ] Replace `space_manager: Arc<SpaceManager>` field with `project_manager: Arc<ProjectManager>`
- [ ] Replace `SpaceManager::new()` with `ProjectManager::new()` in `KernelBuilder`
- [ ] Replace `SpaceApi::new()` with `ProjectApi::new()` in `Kernel::handle()` and `KernelBuilder`
- [ ] Remove `orchestrator.set_space_manager()` call
- [ ] Update `execute_prompt_with_session()` signature if needed

#### Step 15: Update CLI (src/cmd_run.rs)
- [ ] Replace `space_id` and `space_tag` in JSON output with `primary_project_id` and `project_tag`
- [ ] Add `--project` and `--also` flags to `RunOptions`

#### Step 16: Update Web Backend routes
- [ ] Create `routes/project_routes.rs` with CRUD + memory link handlers
- [ ] Update `routes/mod.rs`: remove `space_routes`, add `project_routes`, update route registrations
- [ ] Update `routes/chat.rs`: replace `space_id`/`space_tag` with `project_id`/`project_tag` in request/response/metadata
- [ ] Update `routes/events.rs`: replace `space_id` and Space event mappings with Project equivalents

#### Step 17: Update Web Frontend
- [ ] `types/index.ts`: replace `Space` interface with `Project`, update `Session`/`SessionDetail`
- [ ] `types/memory.ts`: replace `space_id` with `project_ids`
- [ ] `stores/chat.ts`: replace `activeSpaceId`/`_lastDoneSpaceId`/`setActiveSpace` with project equivalents
- [ ] `routes/spaces/` → `routes/projects/`: rename directory, update component names
- [ ] `components/memory/memory-detail.tsx`: update display
- [ ] `routes/agents/$agentId.tsx`: update display

#### Step 18: Remove Space module
- [ ] Delete `crates/oxios-kernel/src/space.rs`
- [ ] Delete `crates/oxios-kernel/src/space/` directory entirely
- [ ] Verify no remaining `use crate::space` imports anywhere

#### Step 19: Update tests
- [ ] Update all `space.rs` tests (remove or port to project tests)
- [ ] Update `space/manager.rs` tests
- [ ] Update `orchestrator.rs` tests
- [ ] Update `kernel_bridge.rs` test (assert tool count: 24 → 24 minus space + project)
- [ ] Update `kernel_handle/mod.rs` tests if any

#### Step 20: Update config (if space-related settings exist)
- [ ] Check `crates/oxios-kernel/src/config.rs` for space settings
- [ ] Remove or repurpose any space-related config

### Phase 2: AI Detection

- [ ] Auto-detect project from conversation (name/path/tag matching)
- [ ] Auto-create projects with user notification
- [ ] AI suggestion flow ("Detected path /projects/oxios. Create project 'oxios'?")

### Phase 3: Memory Integration

- [ ] Drop `memories.space_id` column (migration complete)
- [ ] Project-scoped memory queries via `project_memory` junction
- [ ] Auto-associate memories with active project
- [ ] Cross-project memory references (via secondary projects)
- [ ] Web UI: project → memory browser

### Phase 4: Web UI Polish

- [ ] Project list/detail pages with full CRUD
- [ ] Session creation with project picker (primary + multi-select for secondary)
- [ ] Project-attached memory view
- [ ] Project settings (tags, paths, emoji picker)

## CWD Fallback Behavior

When a session has no project (`primary_project_id = None`):

```
if project_paths.is_empty() {
    workspace_dir.clone()  // from config: ~/.oxios/workspace
} else {
    project_paths[0].clone()  // primary project's first path
}
```

Default: `~/.oxios/workspace/oxios-agent-workspace/` (unchanged from current behavior).

## ConversationBuffer Migration

`ConversationBuffer` currently stores `space_id` in each `ConversationTurn`. After migration:

```rust
// Before
pub struct ConversationTurn {
    pub user: String,
    pub agent: String,
    pub space_id: SpaceId,
    pub timestamp: DateTime<Utc>,
}

// After
pub struct ConversationTurn {
    pub user: String,
    pub agent: String,
    pub project_id: Option<ProjectId>,  // None = no project context
    pub timestamp: DateTime<Utc>,
}
```

The `last_project_id` tracking replaces `last_space_id`. The module can stay in `project/conversation_buffer.rs` (or move to `orchestrator.rs` level if preferred — the module has no Project dependency, so location is flexible).

## Open Questions (Resolved)

| Question | Resolution |
|----------|------------|
| Should non-code projects be supported? | **Yes**, `paths` is optional |
| How to handle project-less sessions in Web UI? | Show "No project" label, project picker is optional |
| Project emoji auto-detection? | **Deferred to Phase 2** |
| How to handle existing `memories.space_id`? | **Keep nullable (Phase 1), drop in Phase 3** |
| Persistence: file vs SQLite? | **SQLite** — same `memory.db` file |
| `AiSuggested` in Phase 1? | **No** — removed, add in Phase 2 |
| ConversationBuffer location? | **Flexible** — module has no Project dep, stays where orchestrator uses it |

## Examples

### Scenario 1: Single project session

```
User creates session → picks "oxios" as project
User: "코드리뷰해줘"
Agent: uses /Volumes/MERCURY/PROJECTS/oxios as CWD, loads oxios-associated memories
```

### Scenario 2: Multi-project session

```
User creates session → picks "oxios" primary, "oxi" + "oxibrowser" secondary
User: "oxios에서 oxi SDK API를 어떻게 쓰고 있어?"
Agent: CWD = oxios, but can reference oxi and oxibrowser paths and memories
```

### Scenario 3: No project

```
User: "오늘 저녁 뭐 먹지?"
Agent: session has no project, no CWD override, no project memories injected
→ Just a normal conversation
```

### Scenario 4: AI auto-detection (Phase 2)

```
User: "oxios 버그 수정해줘"
System: detects "oxios" → finds project → auto-assigns to session
Agent: "oxios 프로젝트에서 작업합니다. 어떤 버그인가요?"
```
