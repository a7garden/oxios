# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0-alpha] - 2026-05-03

### Added

#### AIOS-Inspired Kernel Extensions

- **AgentScheduler** (`scheduler.rs`) — Priority-based task scheduler with:
  - Priority queue (Critical > High > Normal > Low)
  - Rate-limit-aware admission control
  - Max concurrent task enforcement
  - Zombie task detection and automatic reaping
  - API endpoints: `GET /api/scheduler/stats`, `GET /api/scheduler/tasks`

- **ContextManager** (`context_manager.rs`) — 3-tier context hierarchy:
  - **Active tier**: In-memory, in-context (configurable tokens)
  - **Cache tier**: In-memory, not in-context (LRU entries)
  - **Archive tier**: Compressed on disk (unlimited)
  - Automatic demotion when active tier fills up

- **AccessManager** (`access_manager.rs`) — OWASP-inspired security:
  - Tool access control (allow-list per agent)
  - Path sandboxing (glob patterns for allowed/denied paths)
  - Network restrictions (disabled by default)
  - Execution limits (time and memory)
  - Audit logging (timestamp, agent, action, resource, decision)
  - API endpoints: `GET /api/audit`, `GET/PUT /api/permissions/:agent`

#### Programs System

- **ProgramManager** (`program.rs`) — OS-level installable applications:
  - Install/uninstall programs from directories
  - Enable/disable programs
  - Host requirements validation
  - Program metadata parsing (program.toml)
  - API endpoints:
    - `GET /api/programs`, `POST /api/programs`
    - `GET /api/programs/:name`, `DELETE /api/programs/:name`
    - `POST /api/programs/:name/enable`, `POST /api/programs/:name/disable`
    - `GET /api/programs/:name/host-requirements`

- **SkillStore** (`skill.rs`) — Markdown-based instruction templates:
  - CRUD operations for skills
  - Storage in `~/.oxios/workspace/skills/`
  - API endpoints: `GET /api/skills`, `POST /api/skills`, `DELETE /api/skills/:name`

#### MCP & Host Tools

- **McpBridge** (`mcp.rs`) — Model Context Protocol awareness:
  - MCP server registration
  - Tool capability enumeration
  - Protocol handshake support
  - API endpoints: `GET /api/mcp/servers`, `POST /api/mcp/servers`

- **HostToolValidator** (`host_tools.rs`) — Minimal container validation:
  - Required vs optional host tool distinction
  - Presence checking via `which`
  - Full host environment audit
  - API endpoint: `GET /api/host-tools`

#### Seeds API Enhancements

- `GET /api/seeds/:id/evolution` — Track seed evolution lineage with parent links and evaluation scores

#### Configuration Enhancements

- `[scheduler]` section — Max concurrent, rate limit, zombie timeout
- `[context]` section — Active/cache/archive tier configuration
- `[access]` section — Audit log size, default tool allowlists

### Changed

- Kernel module structure expanded from core modules to include AIOS extensions
- API routes reorganized to group related endpoints logically

## [0.1.0-alpha] - 2026-05-03

### Added

- **Core kernel** (`oxios-kernel`) with supervisor, event bus, and state store
- **Ouroboros protocol** (`oxios-ouroboros`) — spec-first workflow:
  interview → seed → execute → evaluate → evolve
- **Gateway** (`oxios-gateway`) with channel-agnostic message routing
- **Web dashboard** (`oxios-web`) with chat, control, and browse panels
- **Apple Container integration** with Garden lifecycle management
- **Host Exec Bridge** for secure macOS command execution
- **Skill system** for markdown-based agent instruction templates
- **CLI** with `garden`, `run`, `status`, and `config` subcommands
- **38 tests** (25 unit + 13 integration)
- **7006 lines** of Rust code across 27 source files
- **1761 lines** of HTML for the web dashboard
