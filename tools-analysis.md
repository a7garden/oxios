# Oxios Tool System & Skill Architecture Analysis

**Date:** 2026-05-26  
**Scope:** Tool registration, skill system, access control  
**Files analyzed:** 12 core modules across `tools/`, `skill/`, and `access_manager/`

---

## Executive Summary

The Oxios tool system is well-architected with clear separation between registration, capability-gated access, and execution. The skill system is a clean multi-format parser with good format detection but carries some unnecessary complexity in its requirements subsystem. The access control model is comprehensive but has **three notable gaps**: dual registration paths that can diverge, an RBAC↔permissions split that creates enforcement ambiguity, and a critical bypass in ExecTool when `agent_name` is `None`.

---

## 1. Tool Registration Consistency

### 1.1 Two Registration Paths Exist

There are **two independent registration paths** that can produce different tool sets:

| Path | Entry Point | Mechanism |
|------|------------|-----------|
| **Kernel Bridge** | `OxiosKernelBridge::register_tools()` → `register_all_kernel_tools()` | Registers **all** kernel tools unconditionally |
| **CSpace-driven** | `register_tools_from_cspace()` | Registers tools based on capability space |

**Finding:** `OxiosKernelBridge::register_tools()` always calls `register_all_kernel_tools()`, which registers everything regardless of CSpace. The CSpace-driven path (`registration.rs`) is more granular and enforces rights-based gating (e.g., memory tools require `READ`/`WRITE` rights). But the bridge path — which is what the agent builder actually uses — bypasses all CSpace checks.

**Risk:** Medium. The CSpace system exists but may not be the active code path in production. If it is activated, the bridge path needs to be updated to use CSpace.

### 1.2 Tool Name Inventory

| # | Tool Name | Source | Registered By |
|---|-----------|--------|---------------|
| 1 | `read` | oxi-sdk | `register_always_on()` |
| 2 | `write` | oxi-sdk | `register_always_on()` |
| 3 | `edit` | oxi-sdk | `register_always_on()` |
| 4 | `grep` | oxi-sdk | `register_always_on()` |
| 5 | `find` | oxi-sdk | `register_always_on()` |
| 6 | `ls` | oxi-sdk | `register_always_on()` |
| 7 | `web_search` | oxi-sdk | `register_always_on()` |
| 8 | `get_search_results` | oxi-sdk | `register_always_on()` |
| 9 | `exec` | oxios-kernel | `register_all_kernel_tools()` |
| 10 | `memory_read` | oxios-kernel | `register_all_kernel_tools()` |
| 11 | `memory_write` | oxios-kernel | `register_all_kernel_tools()` |
| 12 | `memory_search` | oxios-kernel | `register_all_kernel_tools()` |
| 13 | `space` | oxios-kernel | `register_all_kernel_tools()` |
| 14 | `agent` | oxios-kernel | `register_all_kernel_tools()` |
| 15 | `a2a_delegate` | oxios-kernel | `register_all_kernel_tools()` |
| 16 | `a2a_send` | oxios-kernel | `register_all_kernel_tools()` |
| 17 | `a2a_query` | oxios-kernel | `register_all_kernel_tools()` |
| 18 | `persona` | oxios-kernel | `register_all_kernel_tools()` |
| 19 | `cron` | oxios-kernel | `register_all_kernel_tools()` |
| 20 | `security` | oxios-kernel | `register_all_kernel_tools()` |
| 21 | `budget` | oxios-kernel | `register_all_kernel_tools()` |
| 22 | `resource` | oxios-kernel | `register_all_kernel_tools()` |
| 23 | `mcp` | oxios-kernel | `register_all_kernel_tools()` |
| 24 | `knowledge` | oxios-kernel | `register_all_kernel_tools()` |
| 25 | `browser` | oxios-kernel | `register_all_kernel_tools()` (feature-gated) |
| 26 | `marketplace` | oxios-kernel | `register_all_kernel_tools()` |

**Inconsistency:** `OxiosKernelBridge::tool_names()` lists 24 names (comment says "6 always-on + 17 kernel domain = 23 ... plus knowledge = 24"), but `register_all_kernel_tools()` actually registers **18 kernel tools** (exec, 3 memory, space, agent, persona, cron, security, budget, resource, 3 a2a, mcp, knowledge, browser, marketplace). Plus 8 always-on = **26 total**. The `tool_names()` list is stale — it's missing `marketplace` and `knowledge` may have been added after the count was written.

**Recommendation:** Derive `tool_names()` dynamically from the registry after registration, or maintain a single source of truth.

### 1.3 Naming Conventions

Tool names are consistent:
- Always-on tools: snake_case (`read`, `write`, `grep`, `find`, `ls`, `web_search`)
- Kernel tools: snake_case with optional domain prefix (`exec`, `memory_read`, `memory_write`, `memory_search`, `space`, `agent`, `a2a_delegate`, `knowledge`)
- Feature-gated: `browser` (behind `#[cfg(feature = "browser")]`)

**One anomaly:** The kernel tool struct is `AgentTool` but exported as `KernelAgentTool` to avoid collision with the `oxi_sdk::AgentTool` trait. The tool name registered in the registry is `"agent"` (not `"kernel_agent"`). This is correct but could confuse contributors.

---

## 2. Tool API Surface Coherence

### 2.1 Action-Based Dispatch Pattern

All kernel domain tools follow the same pattern: a single `AgentTool` implementation with an `action` parameter that dispatches to specific operations. For example:
- `SpaceTool` with actions: `list`, `get`, `create`, `archive`, `merge`, `restore`
- `BudgetTool` with actions: `check`, `set`, `reserve`, `reset`
- `SecurityTool` with actions: `verify_chain`, `query_audit`, `audit_count`

This is a clean design — it avoids tool explosion while keeping the schema discoverable.

### 2.2 Tool Construction Patterns

There are **two construction patterns** used inconsistently:

| Pattern | Used By | Signature |
|---------|---------|-----------|
| `from_kernel(&KernelHandle)` | SpaceTool, AgentTool, PersonaTool, CronTool, SecurityTool, BudgetTool, ResourceTool, KnowledgeTool, MarketplaceTool | Takes reference, clones Arcs internally |
| `from_kernel(&KernelHandle)` returning Self | ExecTool, MemoryReadTool, MemoryWriteTool, MemorySearchTool, A2aDelegateTool, A2aSendTool, A2aQueryTool, McpToolWrapper, BrowserTool | Same name, same pattern |

The pattern is actually consistent — all use `from_kernel()`. The difference is that some tools need additional parameters (A2A tools need `agent_id`, MCP needs server/name/schema). This is handled correctly.

### 2.3 The `exec` Tool — Security-Critical

The `ExecTool` is the most security-sensitive tool. It implements two modes:

**Shell mode** (`bash -c`):
- Controlled by `config.allow_shell_mode` flag
- Access control: checks `can_use_tool(name, "bash")` via AccessManager
- Minimal env (`HOME`, `USER`, `LOGNAME`, `PATH`, `LANG`, `TERM=dumb`)
- Timeout enforcement with clamping

**Structured mode** (binary + args):
- Binary must be a bare name (no `/`, no `..`)
- Binary must be in allowlist (`ExecConfig::is_binary_allowed`)
- Arguments validated for shell metacharacters and path traversal
- Access control: checks `can_use_tool(name, binary)`

**Environment hardening:** Both modes `env_clear()` then set a minimal whitelist. This is good practice.

**Finding:** The `agent_name: Option<String>` field is the bypass switch. When `None` (via `ExecTool::new()`), access control is completely skipped. This is documented as "tests / development mode" but:
1. `from_kernel()` hardcodes `"oxios-agent"` — good, no bypass.
2. `new()` creates an unrestricted tool — risky if misused.
3. The test `test_no_agent_name_bypasses_access_control` explicitly validates the bypass.

**Recommendation:** Gate `ExecTool::new()` behind `#[cfg(test)]` or make it return a result that requires explicit acknowledgment of the bypass.

### 2.4 MCP Tool Registration

`McpToolWrapper` is registered with empty strings for server name and a trivial schema:

```rust
registry.register(crate::tools::McpToolWrapper::from_kernel(
    kernel, "", "", "MCP tools via bridge".into(), json!({"type": "object", "properties": {}}),
));
```

This appears to be a placeholder/default registration. Actual MCP tools should be enumerated dynamically per agent configuration. The static registration may confuse the tool discovery system.

---

## 3. Skill System: Complexity vs. Value

### 3.1 Architecture Overview

```
SKILL.md file
    ↓
frontmatter.rs: parse_skill() → detect format → parse YAML
    ↓
format.rs: resolve_format() → SkillFormat (Oxios/OpenClaw/ClaudeCode/AgentSkills)
    ↓
requirements.rs: check_requirements() → RequirementsCheck
    ↓
manager.rs: SkillManager → load, CRUD, build snapshots
    ↓
prompt.rs: format_skills_for_prompt() → XML for system prompt
```

### 3.2 Multi-Format Support — High Value

The format detection and parsing system supports 4 skill formats:

| Format | Detection Signal | Value |
|--------|-----------------|-------|
| **Oxios** | `requires`, `install`, `primaryEnv`, `skillKey` keys | Native format — full feature support |
| **OpenClaw** | `metadata.openclaw` / `clawdbot` / `clawdis` nesting | Marketplace/ecosystem compatibility |
| **ClaudeCode** | `allowed-tools`, `arguments`, `when_to_use`, `hooks`, etc. | Cross-tool skill reuse |
| **AgentSkills** | Fallback (no format-specific keys) | Minimal standard format |

This is **high value, moderate complexity**. The format normalization (every format → `ParsedSkill` → `SkillMetadata`) is clean and extensible.

### 3.3 Claude Code Sanitization — Smart Touch

The `sanitize_body()` function neutralizes Claude Code's `!` backtick injection syntax (`!`command`` → HTML comment). This is a thoughtful security measure for cross-format compatibility.

### 3.4 Requirements Checking — Over-Engineered

The `RequirementsCheck` evaluates:
- Required binaries (`bins`) — `which` command
- Any-of binaries (`any_bins`) — at least one must exist
- Required environment variables (`env`)
- Required config paths (`config`) — **always satisfied** (hardcoded `true`)
- OS compatibility

**Finding:** The `config` checks are hardcoded to `satisfied: true` — this is a stub that was never implemented. The `always` flag overrides all requirement checks, making the entire requirements system opt-out.

The requirements check runs **synchronously** in `load_skill_entry()` (called from `load_skills_from_dir()`, an async function). The `has_bin()` helper shells out to `which` for every required binary during skill loading. This could be slow for many skills with many binary requirements.

**Recommendation:** Cache binary availability. Consider async requirements checking.

### 3.5 SkillSnapshot Design

`SkillSnapshot` is well-designed — it produces both a human-readable XML prompt and a structured `Vec<SkillRef>` for programmatic access. The `skill_filter` parameter allows per-agent skill scoping.

### 3.6 Skill Hierarchy — Good Design

```
agent-specific skills > workspace skills > global user skills > bundled skills
```

Bundled skills bootstrap into the user's skills directory on first run (if empty). The loading order ensures user skills override bundled skills with the same name.

### 3.7 Complexity Scorecard

| Aspect | Complexity | Value | Verdict |
|--------|-----------|-------|---------|
| Multi-format parsing | Moderate | High | ✅ Worth it |
| Requirements checking | Low-Moderate | Moderate | ⚠️ Config checks are stubs |
| Prompt formatting | Low | High | ✅ Clean |
| Skill CRUD | Low | High | ✅ Straightforward |
| Install spec types | Low | Low-Moderate | ⚠️ Types defined but no install logic |
| `SkillEntry` fields | Moderate | Moderate | ⚠️ 10 fields, some may be unused |

**Overall:** The skill system is appropriately scoped. The main risks are the stubbed `config` checks and the synchronous `which` calls during loading.

---

## 4. Security Model Consistency

### 4.1 Three-Layer Security Model

The access control has three layers:

```
Layer 1: RBAC (RbacManager) — Role-based, Subject + Action + Resource
Layer 2: Agent Permissions (AccessManager) — Per-agent tool/path/network/fork/time/memory
Layer 3: ExecConfig — Binary allowlist, shell mode toggle, timeouts
```

**All three layers exist independently** and are checked at different points:
- RBAC is checked in `can_access_path_in_workspace()`
- Agent Permissions are checked in `ExecTool::shell_exec()` and `structured_exec()`
- ExecConfig is checked in `ExecTool` (allowlist, metacharacter blocking)

### 4.2 RBAC ↔ Permissions Gap

**Critical finding:** RBAC and Agent Permissions are **not unified**.

| Feature | RBAC | Agent Permissions |
|---------|------|-------------------|
| Subject model | `Subject::User(String)` / `Subject::Agent(AgentId)` / `Subject::System` | `agent_name: String` (just a string key) |
| Tool access | `Action::UseTool("bash")` | `allowed_tools: HashSet<String>` |
| Path access | `Action::AccessPath("/workspace/**")` | `allowed_paths: Vec<String>` + `denied_paths` |
| Fork control | Not modeled | `can_fork: bool` |
| Memory limits | Not modeled | `max_memory_mb: u64` |
| Time limits | Not modeled | `max_execution_time_secs: u64` |
| Network | Not modeled | `network_access: bool` |
| HitL approvals | ✅ `request_approval()` / `approve()` / `reject()` | ❌ Not supported |

**The problem:** `ExecTool` checks Agent Permissions (`can_use_tool`) but does NOT check RBAC. `can_access_path_in_workspace()` checks both RBAC and Agent Permissions, but this method is not called by ExecTool.

**Gap:** An agent that is denied a tool by RBAC but allowed by Agent Permissions (or vice versa) will get inconsistent behavior depending on which code path runs.

### 4.3 Workspace Sandboxing

The workspace sandbox in `AccessManager` is well-implemented:
- Canonical path resolution to prevent symlink escape
- Denied paths take precedence over allowed paths
- Workspace reassignment cleans up old mappings
- Agent-to-workspace is a 1:1 mapping (an agent can only be in one workspace)

**Limitation:** Workspace assignment is never enforced at the ExecTool level. ExecTool doesn't call `can_access_path_in_workspace()`. The sandbox is only effective if the caller explicitly uses it.

### 4.4 Audit Logging

Both RBAC and Agent Permissions maintain separate audit logs:
- `AccessManager::audit_log` — `Vec<AuditEntry>` (in-memory, optional file persistence via bounded channel)
- `RbacManager::audit_log` — `Vec<RbacAuditEntry>` (in-memory only)

Both logs have max entry limits with pruning. The AccessManager's file persistence is well-designed with backpressure (bounded channel of 1000).

**Gap:** `RbacManager::audit_log` has no file persistence. If the process crashes, RBAC audit history is lost.

### 4.5 Default Permissions Analysis

Default `AgentPermissions`:
```rust
allowed_tools: {"read", "write", "edit", "bash", "grep", "find"}
allowed_paths: ["/workspace/**"]
denied_paths: ["/etc/**", "/root/**", "/sys/**", "/proc/**", ".oxios/**"]
network_access: false
max_execution_time_secs: 300  // 5 minutes
max_memory_mb: 512
can_fork: false
```

This is a good default-deny posture. However:
- **`ls` is not in the default allowed_tools** but is registered as always-on. This is inconsistent.
- The always-on tools (`read`, `write`, `edit`, `grep`, `find`, `ls`) bypass Agent Permissions entirely because they're oxi-sdk tools, not ExecTool. This is by design (file tools don't go through AccessManager) but creates a blind spot.

### 4.6 RBAC Default Policies

| Role | Tools | Paths | Max Concurrent |
|------|-------|-------|---------------|
| **User** | read, write, edit, bash, grep, find | /workspace/** | 2 |
| **Superuser** | * (all) | /workspace/**, /tmp/** | 10 |
| **Admin** | * (all) | * (all) | unlimited |

The `User` role's tool list is **hardcoded** in the `default_policy()` method, not derived from the actual always-on tool set. If new always-on tools are added, the RBAC policy must be manually updated.

---

## 5. Gaps in Access Control

### 5.1 Critical Gaps

| # | Gap | Severity | Description |
|---|-----|----------|-------------|
| G1 | ExecTool bypass when `agent_name=None` | **High** | `ExecTool::new()` creates unrestricted tools. Used in tests but accessible from production code. |
| G2 | Dual registration paths | **Medium** | `register_all_kernel_tools()` vs `register_tools_from_cspace()` can diverge. CSpace checks are not active in the bridge path. |
| G3 | RBAC not enforced in ExecTool | **Medium** | ExecTool only checks Agent Permissions, not RBAC. A role-denied tool could still execute. |

### 5.2 Moderate Gaps

| # | Gap | Severity | Description |
|---|-----|----------|-------------|
| G4 | Always-on tools bypass AccessManager | **Medium** | File tools (read/write/edit/grep/find/ls) don't go through permission checks. A denied agent can still read files. |
| G5 | RBAC audit log not persisted | **Low** | Only in-memory. Process crash loses RBAC history. |
| G6 | `config` requirements check is stubbed | **Low** | Always returns `satisfied: true`. Skills with config requirements will pass even if unsatisfied. |
| G7 | MCP placeholder registration | **Low** | `McpToolWrapper` registered with empty strings and trivial schema. May confuse tool discovery. |

### 5.3 Design Observations

| # | Observation | Impact |
|---|-------------|--------|
| O1 | `tool_names()` is stale (24 vs 26 actual tools) | Tests assert exact count; will break on next tool addition |
| O2 | Workspace sandbox not enforced at ExecTool level | Sandbox is opt-in, not automatic |
| O3 | RBAC `User` role tools hardcoded | Must manually sync with always-on tool set |
| O4 | `has_bin()` shells out synchronously during async skill loading | Performance concern at scale |
| O5 | `KnowledgeLens` not registered as a tool | Listed in AGENTS.md as a kernel API but has no tool wrapper |
| O6 | `MarketplaceTool` not in CSpace-driven registration | Only registered via bridge path; no CSpace gating |

---

## 6. Recommendations

### Priority 1 — Security

1. **Gate `ExecTool::new()` behind `#[cfg(test)]`** or make it return `Result` requiring explicit bypass acknowledgment.
2. **Unify RBAC and Agent Permissions checks** in ExecTool — check both layers, or merge them into a single enforcement point.
3. **Activate CSpace-driven registration** in `OxiosKernelBridge` (or remove the dead code path).

### Priority 2 — Consistency

4. **Derive `tool_names()` dynamically** from the registry after registration instead of maintaining a hardcoded list.
5. **Sync RBAC `User` role tool list** with the always-on tool set, or derive it programmatically.
6. **Add `ls` to default `allowed_tools`** in `AgentPermissions::default()` (or document the intentional omission).

### Priority 3 — Completeness

7. **Implement `config` requirement checks** or remove the stub.
8. **Cache binary availability** in requirements checking to avoid repeated `which` calls.
9. **Add file persistence** to `RbacManager` audit log.
10. **Document the workspace sandbox enforcement model** — make it clear that callers must opt into sandbox checks.

---

## 7. Code Quality Assessment

| Metric | Rating | Notes |
|--------|--------|-------|
| Documentation | ★★★★☆ | Module docs, inline comments, and doc tests are excellent |
| Test coverage | ★★★★★ | Every module has thorough unit tests with edge cases |
| Naming consistency | ★★★★☆ | Minor: `KernelAgentTool` vs `agent` registration name |
| Error handling | ★★★★☆ | Proper error types, descriptive messages |
| Security posture | ★★★☆☆ | Good foundation, but gaps in enforcement coverage |
| Modularity | ★★★★★ | Clean separation: tools / skills / access_manager |

---

*End of analysis.*
