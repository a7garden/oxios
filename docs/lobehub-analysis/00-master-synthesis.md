# LobeHub ↔ Oxios: Master Synthesis & Design Direction

> Written 2026-07-19 from a deep-dive analysis of the LobeHub codebase (v2.2.9, 13020 files, 76+ packages).
> Purpose: Identify what Oxios should adopt from LobeHub's architecture, UX, and self-hosting model.

## 1. High-Level Architecture Comparison

| Dimension | LobeHub | Oxios |
|-----------|---------|-------|
| **Runtime** | Next.js 16 + Node.js (Hono backend) | Rust daemon (tokio async) |
| **Frontend** | React 19 SPA, `@lobehub/ui` (antd-style CSS-in-JS), react-router | React 19, Tailwind CSS, @tanstack/react-router |
| **Database** | PostgreSQL (Drizzle ORM) + Redis | Filesystem (~/.oxios) + in-memory state |
| **Storage** | S3-compatible (MinIO) | Filesystem |
| **Auth** | Better Auth + Casdoor SSO, OAuth (Google, GitHub, Microsoft) | None (local daemon) |
| **Multi-tenancy** | Workspaces (full multi-tenant) | Single-user daemon |
| **Deployment** | Docker (multi-container), Vercel, Zeabur, Sealos, Dokploy | Single binary, systemd/launchd |
| **Package mgmt** | pnpm monorepo (76+ packages) | Cargo workspace (7 crates) |
| **State mgmt** | Zustand (30+ stores) | Zustand (web) + Rust state (kernel) |
| **i18n** | 18 languages (react-i18next) | Korean + English |
| **Observability** | OpenTelemetry, Prometheus, Grafana, Tempo | None |
| **Plugin system** | 30+ builtin-tools, MCP, chat-adapters | Unified skill model (SKILL.md) |

### Key Architectural Insight

LobeHub is a **multi-tenant cloud platform** designed for SaaS deployment. Oxios is a **single-user local daemon**. The gap is fundamental: LobeHub needs PostgreSQL/Redis/S3/MinIO/Casdoor/Observability because it serves many users. Oxios runs on one machine for one user.

**However**, Oxios should adopt LobeHub's _self-hosting story_ — a Docker Compose that wraps the daemon with optional PostgreSQL for persistence, MinIO for file storage, and optional SSO for team access.

## 2. Design System & UI Quality

### LobeHub's Design System (from DESIGN.md)

LobeHub has a **mature, documented design system**:
- **Themeable tokens**: Users pick primary/neutral colors; semantic tokens (`cssVar.colorPrimary`, `cssVar.colorText`, etc.) adapt
- **4px spacing scale**: XXS(4) → XL(32)
- **Radius scale**: 4/6/8/12px — distinct from spacing
- **Typography**: Geist + Geist Mono, 12/14/16/20/24/30/38px headings
- **Elevation**: 3 tiers of box-shadow
- **Motion**: 100-300ms, respects `prefers-reduced-motion`
- **Component priority**: `@lobehub/ui/base-ui` → `@lobehub/ui` → antd
- **Voice & content**: Calm, professional, precise copy guidelines

### Oxios Design System

Oxios uses Tailwind CSS with shadcn/ui components. No formal design tokens. No documented spacing/radius/typography scale. No theme system beyond light/dark.

### Gap: Design System Maturity

**What Oxios needs:**
1. **Formal design tokens** — colors, spacing, radius, typography defined once
2. **Theme provider** — light/dark + user-selectable accent colors
3. **Component library discipline** — enforce primitives over ad-hoc markup
4. **Design documentation** — a DESIGN.md modeled after LobeHub's

> **Recommendation**: Adopt `@lobehub/ui` directly? No — it's antd-dependent and CSS-in-JS. Instead, build Oxios tokens _inspired_ by LobeHub's scale. Tailwind's `theme.extend` can encode the same 4px spacing, radius scale, and semantic color tokens.

## 3. Chat UI/UX: The Biggest Gap

### LobeHub's Chat

LobeHub's chat is production-grade:

1. **Message types**: Assistant, User, Tool, Task, TaskCallback, Tasks, GroupTasks, AgentCouncil, CompressedGroup, Verify — each with dedicated renderers
2. **Thinking/Reasoning blocks**: Collapsible `Accordion` with streaming animation, shows duration, dimmed prose
3. **Tool calls**: Custom renderers per tool (registered via `registerBuiltinRenders`), each tool has its own UI card
4. **Artifacts/Works**: Generated content (code, images, pages) rendered inline with rich previews
5. **Message actions**: Copy, retry, branch, edit, share, forward, delete — context menu per message
6. **Markdown rendering**: Custom plugin system (artifact extraction, LaTeX, code highlighting)
7. **Streaming**: Smooth, real-time token streaming with typing indicators
8. **Error handling**: Rich error cards with retry, trace IDs, quota limit warnings
9. **Follow-up chips**: AI-suggested follow-up questions after each response
10. **Chat minimap**: Scroll position indicator for long conversations

### Oxios Chat (Current)

- `message-bubble.tsx`: Basic markdown rendering with react-markdown
- `tool-call-card.tsx`: Simple tool call display
- `activity-timeline.tsx`: Activity/thinking steps
- `typing-indicator.tsx`: Simple dots animation
- No artifact rendering, no custom tool renderers, no follow-up chips, no minimap

### Gap: Chat UX is Oxios's #1 Priority

| Feature | LobeHub | Oxios | Priority |
|---------|---------|-------|----------|
| Thinking/reasoning blocks | Collapsible, animated, duration | Activity timeline (basic) | **High** |
| Custom tool renders | Per-tool UI cards | Generic tool card | **High** |
| Artifact rendering | Images, code, pages inline | None | **High** |
| Message actions | Copy/retry/branch/edit/share/delete | Minimal | **Medium** |
| Follow-up chips | AI-suggested | None | **Medium** |
| Chat minimap | Scroll indicator | None | **Low** |
| Streaming UX | Smooth token-by-token | Basic | **High** |
| Error states | Rich cards with retry/trace | Basic error display | **High** |

## 4. Provider & Model Configuration

### LobeHub's Provider System

LobeHub supports **72+ providers** through a unified `model-runtime` package:
- **Provider interface**: Each provider implements a standard API (chat, embeddings, TTS, STT, image generation)
- **Factory pattern**: `openaiCompatibleFactory` / `anthropicCompatibleFactory` for easy provider creation
- **Model bank**: `packages/model-bank` — central model registry with pricing, context windows, capabilities
- **Provider config UI**: Full settings page with enable/disable, API key input (password field), base URL override, model list management, connection checker
- **Model list management**: Add custom models, sort/filter, enable/disable specific models
- **OAuth device flow**: Some providers support OAuth-based auth instead of API keys
- **Ollama integration**: Local model downloader, setup guide, health check

### Oxios Provider System (Current)

- `oxi-sdk` handles provider/model resolution
- `web/src/components/engine/`: Basic provider card with API key input, model select
- No model list management UI
- No connection checking
- No provider-specific configuration beyond API key + base URL

### Gap: Provider UX

| Feature | LobeHub | Oxios | Priority |
|---------|---------|-------|----------|
| Provider count | 72+ | ~15 (via oxi-sdk) | **Low** (oxi-sdk covers major ones) |
| Model list management | Add/remove/sort/rename models | None | **Medium** |
| Connection checker | Built-in "Check" button | None | **High** |
| OAuth provider auth | Supported | None | **Low** |
| Ollama local models | Full integration | Via oxi-sdk | **Medium** |
| API key UX | Password field, AES-GCM encryption link | Password field | Good enough |

## 5. Extension & Plugin System

### LobeHub's Plugin Architecture

LobeHub has a layered extension system:

1. **Builtin tools** (30+ packages under `packages/builtin-tool-*`):
   - Each tool = manifest (identity) + implementation (server-side) + renders (client-side) + inspectors + streamings + interventions
   - Tools registered via `register.ts` → `registerBuiltinRenders()`, `registerBuiltinInspectors()`, etc.
   - Tool lifecycle: manifest → tool definition → runtime execution → rendered result

2. **Tool runtime** (`packages/tool-runtime`): Execution sandbox for tools

3. **Chat adapters** (`packages/chat-adapter-*`): WeChat, QQ, Feishu, LINE, iMessage — each adapter translates between platform messages and LobeHub's internal format

4. **Skill system** (`packages/builtin-skills`): Higher-level compositions of tools

5. **Skill Store** (`packages/builtin-tool-skill-store`): Community marketplace for skills

6. **MCP integration**: Full MCP client/server support

### Oxios Extension System (Current)

- **Unified skill model**: Each skill = `SKILL.md` + YAML frontmatter
- **Tool registration**: `tools/kernel_bridge.rs::register_all_kernel_tools()`
- **No builtin tool separation**: All tools are kernel-level Rust implementations
- **No chat adapters**: Only Web/CLI/Telegram channels (in-process)
- **No marketplace**: Skills are filesystem-based

### Gap: Extension Architecture

| Feature | LobeHub | Oxios | Priority |
|---------|---------|-------|----------|
| Tool manifest system | Structured manifests | SKILL.md frontmatter | Adequate |
| Custom tool renders | Per-tool React components | Generic tool cards | **High** |
| Tool marketplace | Skill Store (community) | None | **Medium** |
| Chat adapters | WeChat/QQ/Feishu/LINE/iMessage | CLI/Telegram (in-process) | **Low** |
| MCP integration | Full client/server | MCP client only | Adequate |
| Tool sandboxing | Cloud sandbox + local execution | Host execution (AccessManager) | Adequate |

## 6. Self-Hosting Architecture

### LobeHub's Deployment Model

LobeHub's production Docker Compose includes **7 services**:

```
lobehub (app) + postgresql (ParadeDB pg17) + minio (S3) + casdoor (SSO)
  + searxng (web search) + grafana + prometheus + tempo (otel) + otel-collector
```

Key features:
- **One-command setup**: `bash <(curl -fsSL https://lobe.li/setup.sh)`
- **Health checks**: All services have health checks with proper depends_on ordering
- **Startup verification**: The app container verifies Casdoor OIDC + MinIO health before serving
- **Observability**: Full OpenTelemetry pipeline (traces → Tempo, metrics → Prometheus, dashboards → Grafana)
- **Multi-platform**: Vercel, Zeabur, Sealos, Dokploy, RepoCloud, Docker — each with dedicated docs

### Oxios Deployment Model (Current)

- Single Rust binary, serves web UI on localhost
- No Docker Compose
- No persistence beyond filesystem
- No observability

### Gap: Self-Hosting Story

Oxios needs a **Docker Compose** that wraps the daemon with:
1. **Optional PostgreSQL** for persistence (agent state, sessions, chat history)
2. **Optional Redis** for caching/pubsub
3. **Optional S3/MinIO** for file storage
4. **Optional SSO** for team access
5. **One-command setup script** like LobeHub's

> **Recommendation**: Create `docker-compose/production/` with the daemon + optional services. The daemon should work standalone (current mode) AND with external services.

## 7. Feature Comparison Matrix

| Feature Area | LobeHub | Oxios | Gap |
|-------------|---------|-------|-----|
| **Chat** | Multi-modal, rich rendering | Basic markdown | Large |
| **Agent Builder** | Conversational agent creation | SKILL.md files | Medium |
| **Agent Groups** | Multi-agent collaboration | Via Supervisor | Medium |
| **Pages/Documents** | Collaborative editor | Knowledge base (.md) | Medium |
| **Scheduling** | Cron-based agent runs | Cron jobs | Adequate |
| **Workspaces** | Multi-tenant team spaces | Single-user | Different scope |
| **Memory** | Personal + agent memory | Tiered memory (Hot/Warm/Cold) | Adequate |
| **Knowledge Base** | File upload + RAG | Markdown files | Adequate |
| **Model Providers** | 72+ with full UI | 15+ via oxi-sdk | Medium |
| **Plugins/Tools** | 30+ builtin + marketplace | Kernel tools | Medium |
| **Auth** | Better Auth + OAuth/SSO | None (local) | Different scope |
| **Observability** | Full OTEL stack | None | Large |
| **Mobile** | Dedicated mobile SPA | Responsive (limited) | Large |
| **Desktop** | Electron app | None | Different scope |
| **i18n** | 18 languages | 2 languages | Medium |

## 8. Prioritized Action Plan

### Phase 1: Chat UX (Immediate)
These are the highest-impact, visible improvements:

1. **Thinking/Reasoning blocks**: Collapsible accordion with streaming animation
2. **Custom tool renders**: Per-tool React components (at minimum for common tools: file read, bash, web search, knowledge search)
3. **Artifact rendering**: Inline code blocks with syntax highlighting, image display, file previews
4. **Improved streaming**: Smooth token-by-token rendering with proper cursor
5. **Error states**: Rich error cards with retry, error classification

### Phase 2: Provider & Settings UX
1. **Model list management**: Add/remove/reorder models per provider
2. **Connection checker**: "Test Connection" button per provider
3. **Settings search**: CMD+K search across all settings (LobeHub has `SettingsSearch` feature)
4. **Design token standardization**: Formalize spacing, radius, typography in Tailwind config

### Phase 3: Self-Hosting & Platform
1. **Docker Compose**: Production-ready multi-container setup
2. **One-command setup**: `curl | bash` installer
3. **Health checks**: Service health verification
4. **Optional persistence**: PostgreSQL backend for agent state

### Phase 4: Extension & Ecosystem
1. **Custom tool render system**: Plugin API for tool UI components
2. **Chat adapter framework**: Standard interface for external chat platforms

## 9. What NOT to Adopt

1. **Antd / CSS-in-JS**: Oxios's Tailwind approach is simpler and more maintainable for Oxios's scale
2. **Multi-tenant architecture**: Oxios is a single-user daemon; don't over-engineer
3. **Electron desktop app**: Web UI + PWA is sufficient
4. **Full OTEL stack**: Premature for Oxios's current stage
5. **76-package monorepo**: Oxios's 7-crate workspace is appropriate
6. **Casdoor SSO**: Overkill for single-user; optional for future team mode
