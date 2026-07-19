# LobeHub ↔ Oxios: Extension & Plugin Systems

> LobeHub: 30+ builtin-tool npm packages, 6 UI surface types, 5 chat adapters.
> Oxios: 26 Rust AgentTool implementations, CSpace capability-based access, unified SkillManager.

## 1. LobeHub Extension Architecture

LobeHub's extension system is a **multi-layered TypeScript monorepo**:

### Layer 1: Tool Manifest & Registration

Each tool is a standalone npm package (`packages/builtin-tool-*`) exporting:

```typescript
interface LobeBuiltinTool {
  identifier: string;
  manifest: BuiltinToolManifest;
  hidden?: boolean;
  discoverable?: boolean;
  resolveManifest?: (runtime: RuntimeContext) => Partial<BuiltinToolManifest>;
}

interface BuiltinToolManifest {
  api: ApiSchema[];          // JSON Schema per operation
  systemRole: string;         // Injected system prompt
  meta: { avatar, title, description };
  settings?: JsonSchema;
  humanIntervention?: Record<string, InterventionPolicy>;
}
```

**Central registry** (`packages/builtin-tools/src/index.ts`):
- `defaultToolIds`: 13 tools every agent gets
- `alwaysOnToolIds`: 4 tools that can't be disabled (user interaction, verify)
- `runtimeManagedToolIds`: 8 tools decided by system conditions
- `chatModeAllowedToolIds`: 3 tools in chat-only mode
- `groupSupervisorToolIds`: 1 tool for group orchestration

**Sample: Calculator tool** (`builtin-tool-calculator/src/manifest.ts`):
- 10 API operations: calculate, evaluate, sort, base, differentiate, integrate, defintegrate, execute, limit, solve
- Full JSON Schema parameters per operation
- System role prompt injection for math context

### Layer 2: Execution Runtime

Tools have server-side runtimes implementing their APIs:

```typescript
// ComputerRuntime (tool-runtime/src/ComputerRuntime.ts)
abstract class ComputerRuntime {
  abstract listFiles(params: ListFilesParams): Promise<ServiceResult>;
  abstract readFile(params: ReadFileParams): Promise<ServiceResult>;
  abstract writeFile(params: WriteFileParams): Promise<ServiceResult>;
  abstract editFile(params: EditFileParams): Promise<ServiceResult>;
  // ... 11 operations total
}
```

Concrete implementations:
- **LocalSystemExecutionRuntime**: Electron IPC for desktop
- **Cloud sandbox runtime**: Remote execution
- **Custom runtimes**: CalculatorExecutionRuntime, WebBrowsingExecutionRuntime

### Layer 3: UI Surface Registration

Each tool registers **6 UI surface types** via `register.ts`:

| Surface | Purpose | Example |
|---------|---------|---------|
| **Render** | Result visualization in chat | File preview, image display, diff view |
| **Inspector** | Inline expansion panel for call details | Args preview, status, timing |
| **Streaming** | Streaming progress indicators | Progress bar, partial output |
| **Intervention** | Human-in-the-loop approval flows | Confirm/cancel/edit dialogs |
| **Placeholder** | Pre-execution placeholder | "Running..." with icon |
| **Portal** | Full-screen overlay | Web browser results, generated pages |

**Shared tool UI** (`packages/shared-tool-ui`):
- 14 inspector factories: EditLocalFile, GlobLocalFiles, GrepContent, ListLocalFiles, MoveLocalFiles, ReadLocalFile, RunCommand, SearchLocalFiles, WriteLocalFile, Twitter, GitHub, Linear
- `ToolRenderContext`: React context injecting platform capabilities (relative paths, loading states, file open actions)

### Layer 4: Agent Runtime (Decision Loop)

```typescript
// AgentRuntime step loop:
// state + context → runner → instructions → executors → events + new state + next context
```

Instruction types: `call_llm`, `call_tool`, `call_tools_batch`, `finish`, `request_human_approve`, `request_human_prompt`, `request_human_select`, `resolve_blocked_tools`.

**GeneralChatAgent** decision loop:
```
user_input → call_llm → check intervention
  → execute safe tools + request approval for risky ones
  → call_llm → finish
```

Intervention checking:
- Security blacklist (regex patterns for dangerous commands)
- Global audits (custom validation functions)
- Per-tool config: `always`, `never`, `required`
- Dynamic resolvers (context-aware decisions)
- Approval modes: `auto-run` (no human), `manual` (always prompt), `headless` (never prompt)

### Layer 5: Chat Platform Adapters

Five adapters (`packages/chat-adapter-*`):

| Adapter | Protocol | Auth |
|---------|----------|------|
| **WeChat** | iLink protocol | QR code |
| **Feishu/Lark** | Event Subscription v2 | Webhook + AES decrypt |
| **QQ** | QQ Bot API | Token |
| **iMessage** | Apple Messages | Device-based |
| **LINE** | LINE Messaging API | Channel token |

Each adapter implements the `chat-sdk` `Adapter` interface:
- Platform-specific message format → unified SDK format
- Media attachment metadata extraction (deferred download)
- Thread/conversation ID encoding
- Authentication lifecycle

### Layer 6: Skill System

Skills are `BuiltinSkill` objects:
```typescript
interface BuiltinSkill {
  identifier: string;
  name: string;
  description: string;
  avatar: string;
  content: string;         // System prompt
  resources: Resource[];    // Reference files for progressive disclosure
}
```

Loaded via the `Skills` and `SkillStore` builtin tools. The **Verify** skill is a portable self-evidence system for task delivery verification — 4-step loop (discover plan → pick surface → capture evidence → submit).

## 2. Oxios Extension Architecture

Oxios takes a **unified Rust approach**:

### Layer 1: Tool Registration (`tools/registration.rs`)

Two-tier system:
- **Tier 1: Always-on** — `read`, `write`, `edit`, `grep`, `find`, `ls`, `web_search`, `get_search_results`
- **Tier 2: CSpace-driven** — Registered only if agent's `CSpace` contains matching capabilities

Tools are wrapped in `GatedTool` for path-based access control. Static catalog (`registry.rs`) provides metadata for the frontend: 26 tools across 6 categories (fs, exec, comms, memory, system, a2a).

### Layer 2: Tool Implementation

Each tool implements the `oxi_sdk::AgentTool` trait:
```rust
trait AgentTool {
    fn name(&self) -> &str;
    fn label(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;
    async fn execute(&self, args: Value) -> Result<ToolOutput>;
}
```

**Key tools** (`tools/builtin/`):
- `ExecTool` — Dual-mode: shell (bash -c) + structured (binary + args with allowlist + metacharacter blocking)
- `SkillForgeTool` — 10 CRUD actions for skills with marketplace integration
- `MemoryReadTool` / `MemorySearchTool` / `MemoryWriteTool` — Agent memory access
- `AskUserTool` — Async agent clarification via event bus + oneshot channel
- `McpToolWrapper` — Dynamic MCP tool enumeration
- `SubagentTool` — Fork child agents for parallel work

### Layer 3: Security (`tools/gated_tool.rs`)

`GatedTool` intercepts all tool executions through `AccessGate`:
1. Path extraction for file tools
2. `PathMode` determination (Read vs Write)
3. Multi-layer deny: Capability → RBAC → Permissions → ExecPolicy
4. Formatted denial messages with suggestions and layer tags

### Layer 4: Kernel Bridge (`tools/kernel_bridge.rs`)

`OxiosKernelBridge` implements `KernelToolProvider` to plug Oxios kernel tools into `oxi_sdk::AgentBuilder`:
- `tool_names()`: Returns 26 tool names
- `register_tools()`: Registers always-on + kernel domain tools

### Layer 5: Skill System

Skills are filesystem artifacts (folders with `SKILL.md`) managed by `SkillManager`:
- YAML frontmatter: `name`, `description` (as trigger)
- Progressive disclosure: name+description always in context, body loaded on trigger, resources on demand
- Marketplace: ClawHub (community) + Skills.sh (Anthropic-compatible)

### Layer 6: Integration System

TOML-based `default-integrations.toml`:
- Package managers: brew, npm, cargo, bun, go, uv
- CLI tools: GitHub (OAuth device-code), Resend (API key)
- User overrides via `~/.oxios/integrations.d/`

## 3. Architectural Comparison

| Aspect | LobeHub | Oxios |
|--------|---------|-------|
| **Registration** | Central registry with category rules | CSpace capability-based (agent authorization) |
| **Tool Structure** | npm packages (manifest + runtime + UI) | Rust structs (AgentTool trait) |
| **UI Integration** | 6 surface types per tool (Render/Inspector/Streaming/Intervention/Placeholder/Portal) | No tool-level UI in Rust layer |
| **Execution Runtime** | `ComputerRuntime` abstract class with Electron IPC / cloud sandbox | `ExecTool` with shell + structured modes |
| **Human Intervention** | Rich policy engine: per-API policies, dynamic resolvers, 3 approval modes | `AskUserTool` + path-based access gate |
| **Chat Adapters** | 5 adapters (WeChat, Feishu, QQ, iMessage, LINE) | None (Oxios is personal, not multi-platform) |
| **Skills** | Prompt+reference bundles loaded via tool calls | Filesystem artifacts with marketplace |
| **Language** | TypeScript monorepo (Node + React + Electron) | Rust (tokio async) |
| **Agent Loop** | Step-based `AgentRuntime` with `GeneralChatAgent` brain | oxi-sdk `AgentBuilder` pattern |
| **Marketplace** | Skill store via `lh skill install` | ClawHub + Skills.sh via `SkillForgeTool` |
| **Configuration** | Per-agent plugin settings UI | CSpace authorization + ExecConfig allowlist |

## 4. What Each Does Better

### LobeHub Strengths
1. **Rich tool UI**: 6 surface types enable polished, tool-specific chat rendering
2. **Human intervention**: Sophisticated policy engine with dynamic resolvers
3. **Chat adapters**: Production-quality multi-platform bridge
4. **Tool sandboxing**: Cloud sandbox for untrusted code execution
5. **Tool marketplace**: Community tool discovery and installation

### Oxios Strengths
1. **Type-safe Rust**: No runtime type errors, memory safety, better performance
2. **CSpace authorization**: Capability-based access is more granular than role-based
3. **Unified skill model**: Single `SKILL.md` format works everywhere (CLI, Web, MCP)
4. **Progressive disclosure**: Skills scale from 1 line to full documentation
5. **Hot-reload**: ExecConfig and integrations update without restart
6. **Cost-efficient routing**: Model-level routing with fallback and exclusion

## 5. Design Recommendations

### Phase 1: Tool UI Surface Types (Critical)

Add a tool render registry to the Oxios web frontend:

```typescript
// web/src/lib/tool-renders.ts
interface ToolRender {
  identifier: string;
  component: React.ComponentType<ToolRenderProps>;
  inspector?: React.ComponentType<ToolInspectorProps>;
}

const toolRenderRegistry = new Map<string, ToolRender>();

export function registerToolRender(render: ToolRender): void;
export function getToolRender(identifier: string): ToolRender | undefined;
```

Register custom renders for 5 core tools: file read, file edit, bash, web search, knowledge search.

### Phase 2: Human Intervention Flow

Extend the `AskUserTool` with:
- Per-tool intervention policies (auto-approve, always-ask, never-allow)
- UI approval cards with argument editing
- Batch approval for multi-tool calls
- Timeout-based auto-deny

### Phase 3: Integration Catalog UI

Build a web UI around the `default-integrations.toml`:
- Detect installed tools (brew, npm, cargo, bun, go, uv)
- Show available/installable integrations
- OAuth device-code flow for GitHub, Resend
- Credential management (add, validate, remove)

### Phase 4: Chat Adapter Framework (Optional)

If Oxios needs multi-platform support:
- Define a `ChannelAdapter` trait (already exists in gateway)
- Allow WASM-based adapter plugins
- Ship Telegram adapter as reference implementation

### What NOT to Adopt
1. **30+ builtin-tool npm packages**: Oxios's unified Rust approach is cleaner
2. **Lexical/rich-text chat input**: Plain textarea with @mentions is sufficient
3. **Tool sandboxing via cloud**: Oxios's AccessManager + RBAC is adequate for single-user
4. **Per-tool npm package structure**: Oxios tools fit naturally in Rust modules
