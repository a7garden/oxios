# RFC-0001: Remove Container Layer

> **Status:** Draft — reviewed, 4 issues found  
> **Date:** 2026-05-10  
> **Scope:** oxios-kernel, oxios binary, oxios-web

## Summary

Remove the Apple Container layer entirely. Replace all container-based command execution with direct host process execution via `tokio::process::Command`. Simplify the codebase by ~3,500 lines while keeping the existing RBAC + audit + allowlist security model.

## Motivation

1. **Self-contradicting architecture.** Container Minimalism ships minimal tools inside the container, then relays all real work (git, gh, osascript) back to the host via `host_exec`. The container adds latency and complexity without meaningful isolation.

2. **Single-user macOS desktop.** Oxios runs on a personal macOS machine with a single user. There is no multi-tenant scenario that demands container isolation.

3. **Security boundary is already on the host.** `HostExecBridge` has allowlist + metachar blocking + path traversal prevention. `AccessManager` has RBAC + sandbox paths + audit. These provide sufficient security for a desktop agent OS.

4. **Maintenance cost.** Container backend code is ~3,500 lines of Apple Container-specific logic that must track CLI changes, handle platform detection, manage Containerfiles per toolchain — all for a layer that delegates real work to the host anyway.

## What Changes

### Delete (entire files)

| File | Lines | Reason |
|------|-------|--------|
| `container.rs` | 890 | Apple Container backend — entire abstraction |
| `container_manager.rs` | 658 | Container lifecycle, Containerfile templates, toolchain management |
| `tools/container_exec.rs` | 341 | Container exec tool — replaced by new `exec` tool |
| `host_exec.rs` | 525 | UDS relay protocol (container → host bridge) — no longer needed |

**Total removed: ~2,414 lines**

### Rewrite

| File | Change |
|------|--------|
| `tools/host_exec_tool.rs` → `tools/exec_tool.rs` | Rename. Replace `HostExecBridge` dependency with direct `tokio::process::Command`. Keep allowlist + metachar blocking logic inline. |
| `tools/program_tool.rs` | Replace `HostExecTool` dependency with new `ExecTool`. Remove `_container_config` parameter. |
| `agent_runtime.rs` | Remove `host_bridge` field. Create `ExecTool` directly instead of `HostExecTool`. |
| `config.rs` | Replace `ContainerConfig` with `ExecConfig`. Remove container-specific fields. |
| `lib.rs` | Remove `container`, `container_manager`, `host_exec` module exports. Export new types. |
| `error.rs` | Remove `ContainerUnavailable` variant. Add `ExecFailed` if needed. |
| `kernel.rs` (binary) | Remove `HostExecBridge` creation. Use new `ExecTool` directly. |
| `audit_trail.rs` | Remove `ContainerStart`, `ContainerStop` audit actions. |

### Update (minor)

| File | Change |
|------|--------|
| `metrics.rs` | Replace `oxios_container_exec_*` metrics with `oxios_exec_*`. |
| `backup.rs` | Remove `"containers"` from backup paths. |
| `host_tools.rs` | Update doc comments to remove container references. |
| `git_layer.rs` | Remove `container_volumes/` from gitignore patterns. |
| `program.rs` | Update test references from `container_exec` to `exec`. |
| `main.rs` | Remove container config get path. |
| `channels/oxios-web/frontend/` | Remove container views/routes. |
| `channels/oxios-web/static/default-config.toml` | Replace `[container]` section with `[exec]`. |

## New Architecture

### Before (current)

```
AgentRuntime
  ├── ToolRegistry
  │   ├── oxi native tools (read, write, search, ...)
  │   ├── ContainerExecTool ──→ ContainerManager ──→ AppleBackend
  │   │                                                 ↓
  │   │                                          Apple Container CLI
  │   │                                                 ↓
  │   │                                          container process
  │   │                                                 ↓
  │   │                                          host_exec relay (UDS)
  │   │                                                 ↓
  │   │                                          HostExecBridge
  │   │                                                 ↓
  │   │                                          tokio::process::Command
  │   ├── HostExecTool ──→ HostExecBridge ──→ tokio::process::Command
  │   └── ProgramTool ──→ HostExecTool ──→ HostExecBridge
  └── ...
```

### After (proposed)

```
AgentRuntime
  ├── ToolRegistry
  │   ├── oxi native tools (read, write, search, ...)
  │   ├── ExecTool ──→ tokio::process::Command
  │   │                 (with allowlist + metachar blocking)
  │   └── ProgramTool ──→ ExecTool
  └── ...
```

### Key Design: `ExecTool`

The new unified `ExecTool` replaces both `ContainerExecTool` and `HostExecTool`:

```rust
/// Unified workspace command execution tool.
///
/// Executes commands directly on the host via tokio::process::Command.
/// Security is enforced through:
/// - Binary allowlist (config.exec.allowed_commands)
/// - Shell metacharacter blocking (for structured mode)
/// - Working directory restriction (config.exec.workspace_path)
/// - Audit logging via AccessManager
pub struct ExecTool {
    config: Arc<ExecConfig>,
    audit: Arc<Mutex<AccessManager>>,
}

impl ExecTool {
    /// Shell mode: accepts command string, runs via `sh -c`.
    /// Used by agent for workspace commands (build, test, etc.)
    pub fn shell_exec(&self, command: &str, cwd: Option<&Path>, timeout: Duration) -> ExecResult;

    /// Structured mode: binary + args, strict allowlist.
    /// Used by ProgramTool and host-specific commands.
    pub fn structured_exec(&self, binary: &str, args: &[String], timeout: Duration) -> ExecResult;
}
```

Two modes preserve the security distinction:

| Mode | API | Allowlist | Use case |
|------|-----|-----------|----------|
| `shell` | command string | Workspace-restricted | Agent workspace commands (build, test, lint) |
| `structured` | binary + args | Binary allowlist + metachar block | Host tools (git, gh, osascript) |

### New Config: `ExecConfig` → replaces `ContainerConfig`

```toml
[exec]
# Commands allowed for structured execution (binary + args).
# Empty = allow all (development mode).
allowed_commands = ["git", "gh", "osascript", "open", "curl", "python3"]

# Host tools that MUST be available (checked on startup).
required_host_tools = ["git"]

# Optional host tools (checked when needed).
optional_host_tools = ["gh", "remindctl", "shortcuts", "osascript", "open"]

# Default command timeout in seconds.
default_timeout = 120

# Maximum command timeout in seconds.
max_timeout = 600
```

The old `[container]` section fields that no longer apply:
- `container_path` → removed (no container directories)
- `image_tag` → removed
- `memory_limit` → removed
- `cpu_limit` → removed
- `minimal_tools` → removed (no container image)
- `execution_mode` → removed (always host)

Fields that migrate to `[exec]`:
- `allowed_host_commands` → `allowed_commands`
- `required_host_tools` → same
- `optional_host_tools` → same

## Migration Path

### Phase 1: Introduce `ExecTool` (additive, no breakage)

1. Create `tools/exec_tool.rs` with `ExecTool` struct
2. Create `exec.rs` module with `ExecConfig` (can be in `config.rs` for now)
3. Register `ExecTool` alongside existing tools in `agent_runtime.rs`
4. Wire in `kernel.rs`
5. All tests pass with both old and new code present

### Phase 2: Migrate consumers

1. `ProgramTool` → use `ExecTool` instead of `HostExecTool`
2. Agent system prompt → reference `exec` tool instead of `container_exec`
3. Integration tests → use `ExecTool`
4. Config → add `[exec]` section alongside `[container]`

### Phase 3: Remove container layer

1. Delete `container.rs`, `container_manager.rs`, `container_exec.rs`, `host_exec.rs`
2. Remove `ContainerConfig`, `ContainerBackend`, `AppleBackend`, etc. from `lib.rs`
3. Remove `[container]` from config (keep `[exec]` only)
4. Update `kernel.rs` to remove all container wiring
5. Remove container-related frontend views
6. Clean up metrics, audit actions, backup paths

### Phase 4: Cleanup

1. Remove `ContainerUnavailable` from error types
2. Update `AGENTS.md` (remove Container Minimalism section)
3. Update all doc comments referencing containers
4. Verify `cargo test --workspace` passes

---

## Review: Issues Found (2026-05-10)

### Issue 1 (HIGH): Garden 개념이 AccessManager에 깊이 스며들어 있음 — RFC에서 누락

**현황:** `access_manager.rs`에 Garden/Container 샌드박스 시스템이 ~200줄 있음:
- `container_workspaces: HashMap<String, PathBuf>`
- `agent_containers: HashMap<String, String>`
- `garden_agents: HashMap<String, HashSet<String>>`
- 메서드: `register_container_workspace`, `assign_garden`, `can_access_garden`, `can_access_path_in_garden`, `unassign_garden`, `remove_garden`, `list_agents_in_garden`, `list_containers`, `get_container_workspace`
- RBAC `Action::ManageGardens`

**문제:** RFC는 `access_manager.rs`를 "unchanged"라고 썼지만, 실제로는 Garden 샌드박스 전체를 제거/재설계해야 함. 컨테이너를 빼면 Garden(=컨테이너)도 의미가 없어짐.

**해결:**
- Garden 개념 → **Workspace** 개념으로 재명명. 컨테이너 대신 작업 디렉토리 기반 샌드박스.
- `container_workspaces` → `workspace_paths: HashMap<String, PathBuf>` (프로젝트명 → 경로)
- `assign_garden` → `assign_workspace`
- `Action::ManageGardens` → `Action::ManageWorkspaces`
- 경로 기반 샌드박스 로직(`can_access_path_in_garden`)은 그대로 유지 — 컨테이너 없이도 의미 있음

### Issue 2 (MEDIUM): Garden UI (Frontend)가 완전한 뷰로 존재 — 단순 "Remove"로는 부족

**현황:** `channels/oxios-web/frontend/src/views/gardens.rs`에 완전한 Garden 관리 뷰가 있음 (Start/Stop/Remove 액션). 사이드바에 "🌿 Gardens" 패널. API 타입에 `GardenInfo`, `GardenSummary`.

**문제:** RFC는 "Remove container views/routes"라고만 적었음. Garden UI를 어떻게 처리할지 명확하지 않음.

**해결:**
- Garden 뷰 → **Workspace** 뷰로 전환 (또는 제거)
- 컨테이너 start/stop 액션은 불필요 → 워크스페이스 정보만 표시
- `container_id` 필드 → 제거

### Issue 3 (MEDIUM): `ExecutionMode` enum이 config에 존재하지만 어디도 사용 안 함

**현황:** `config.rs`에 `ExecutionMode { Container, Auto }` enum이 있으나, 현재 코드에서 이 값을 읽어 분기하는 곳이 없음. `container_exec.rs`는 항상 active container 여부로만 분기.

**해결:**
- `ExecutionMode`는 그냥 삭제. Phase 3에서 `ContainerConfig`와 함께 제거.

### Issue 4 (LOW): `KernelHandle`은 Container 의존성이 없어서 변경 불필요

**현황:** `kernel_handle/` 디렉토리의 모든 파일은 ContainerManager, HostExecBridge 등을 직접 참조하지 않음. Facade 패턴으로 이미 잘 분리되어 있음.

**결론:** KernelHandle은 변경 없음. 설계가 좋았음.

---

## Updated: AccessManager Migration Plan

### Rewrite (추가)

| File | Change |
|------|--------|
| `access_manager.rs` | Garden → Workspace 재명명. `container_workspaces` → `workspace_paths`. Garden 관련 메서드/필드 이름 변경. 로직은 동일하게 유지. |

### Update (추가)

| File | Change |
|------|--------|
| `channels/oxios-web/frontend/src/views/gardens.rs` | Workspace 뷰로 전환 또는 제거 |
| `channels/oxios-web/frontend/src/api/mod.rs` | `GardenInfo` → `WorkspaceInfo`. `container_id` 필드 제거 |
| `channels/oxios-web/frontend/src/components/sidebar.rs` | "Gardens" → "Workspaces" |
| `channels/oxios-web/src/routes/events.rs` | `ManageGardens` → `ManageWorkspaces` |

---

## Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| Loss of process isolation | Low — single-user desktop | RBAC + allowlist + audit already sufficient |
| Agent runs destructive command | Medium | Working directory restriction + allowlist |
| Future multi-tenant need | Low — deferred | `ContainerBackend` trait can be re-introduced later |
| Breaking config files | Low | Migration: rename `[container]` → `[exec]` with fallback |

## What We Keep

- **Allowlist security model** — moves from `HostExecBridge` into `ExecTool`
- **Metacharacter blocking** — same logic, now in `ExecTool::structured_exec`
- **RBAC + AccessManager** — unchanged
- **Audit trail** — `ContainerStart/Stop` → `ExecStart/Stop`
- **Host tool validation** — `HostToolValidator` unchanged
- **Program system** — `ProgramTool` just points to `ExecTool` instead of `HostExecTool`

## What We Lose

- Process-level isolation (container sandbox)
- Per-container resource limits (memory, CPU)
- Toolchain-specific Containerfiles (Rust, Node, Python)
- Container workspace directory structure (`~/.oxios/containers/<name>/`)

None of these are needed for a single-user desktop agent OS.

## Estimated Savings

| Metric | Before | After |
|--------|--------|-------|
| Kernel source files | ~40 | ~36 |
| Lines of code | ~2,400 container + 525 host_exec | ~200 `exec_tool.rs` |
| AccessManager garden code | ~200 lines | ~200 lines (renamed, logic same) |
| Runtime dependencies | Apple Container CLI | None (just macOS) |
| Config sections | `[container]` (10 fields) | `[exec]` (5 fields) |
| Tool registration | 3 tools (container_exec, host_exec, program) | 2 tools (exec, program) |
| Frontend views | Gardens (container CRUD) | Workspaces (directory info only) |
| RBAC actions | `ManageGardens` | `ManageWorkspaces` |
