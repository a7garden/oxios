# Audit Results: unwrap() & Error Handling

**Date:** 2026-05-31  
**Scope:** 7 critical kernel files  
**Auditor:** Automated audit (Phase 1)

## Summary

The brief estimated ~153 `unwrap()` calls across 7 files. The actual count is dramatically lower — the codebase has already been extensively hardened. Only **12 production-path** `unwrap()`/`expect()` calls remain across all 7 files, of which **3 are ACTIONABLE**.

| File | Production `unwrap()` | Production `expect()` | ACTIONABLE |
|------|----------------------|-----------------------|------------|
| `scheduler.rs` | 0 | 0 | 0 |
| `cron.rs` | 0 | 0 | 0 |
| `orchestrator.rs` | 0 | 1 | 1 |
| `supervisor.rs` | 0 | 0 | 0 |
| `budget.rs` | 0 | 0 | 0 |
| `src/kernel.rs` | 4 | 7 | 2 |
| `access_manager/mod.rs` | 0 | 0 | 0 |
| **Total** | **4** | **8** | **3** |

---

## Classification Details

### `crates/oxios-kernel/src/scheduler.rs` — 0 issues

No `unwrap()` or `expect()` calls in production code. Already clean.

### `crates/oxios-kernel/src/cron.rs` — 0 issues

No `unwrap()` or `expect()` calls in production code. Already clean.

### `crates/oxios-kernel/src/orchestrator.rs` — 1 issue

| Line | Code | Classification | Fix |
|------|------|---------------|-----|
| 951 | `.expect("execute_single_subtask is only called when subtasks is non-empty")` | **ACTIONABLE** | Function is called from `delegate_subtasks` after a `subtasks.len() == 1` check. The `expect` documents the invariant but the function signature accepts `Vec<SubTask>` without enforcing non-empty. Should return `Result` or handle empty case gracefully. |

### `crates/oxios-kernel/src/supervisor.rs` — 0 issues

No `unwrap()` or `expect()` calls in production code. Already clean.

### `crates/oxios-kernel/src/budget.rs` — 0 issues

No `unwrap()` or `expect()` calls in production code. Already clean.

### `src/kernel.rs` — 11 calls (2 ACTIONABLE, 9 SAFE)

| Line | Code | Classification | Notes |
|------|------|---------------|-------|
| 82 | `.expect("KnowledgeBase init failed")` | SAFE | Startup-time invariant in `handle()` (OnceLock). If KB can't initialize, system is misconfigured. Add `// SAFETY:` comment. |
| 89 | `.expect("KnowledgeLens init failed")` | SAFE | Same — startup-time invariant. Add `// SAFETY:` comment. |
| 210 | `.expect("ProjectManager not available — SQLite must be enabled")` | SAFE | Documented API contract — caller must ensure feature is enabled. Add `// SAFETY:` comment. |
| 239 | `ClawHubClient::new(Some("https://clawhub.ai".to_string())).unwrap()` | SAFE | Hardcoded valid URL in fallback branch. Add `// SAFETY:` comment. |
| 391 | `.and_hms_opt(3, 0, 0).unwrap()` | **ACTIONABLE** | `and_hms_opt(3,0,0)` is always `Some`, but inside an async spawned task (not startup). A panic here kills the health check task silently. Use `.expect("3:00:00 is always a valid time")` at minimum, or `unwrap_or_default` with error logging. |
| 393 | `.and_local_timezone(chrono::Local).unwrap()` | **ACTIONABLE** | `and_local_timezone` can theoretically fail for ambiguous or non-existent local times (e.g., DST transitions at exactly 3 AM). In a spawned async task, this would silently kill the health check. Should handle the `Err` case with a fallback. |
| 826 | `.expect("KnowledgeBase init failed")` | SAFE | Startup-time in `build()`. Same as line 82. |
| 835 | `.expect("KnowledgeBase init failed")` | SAFE | Same — second KB instance for KnowledgeLens. |
| 839 | `.expect("KnowledgeLens init failed")` | SAFE | Startup-time invariant in `build()`. |
| 1088 | `ClawHubClient::new(Some("https://clawhub.ai".to_string())).unwrap()` | SAFE | Inside `unwrap_or_else` fallback for `build_marketplace_api_value()`. Hardcoded valid URL. Add `// SAFETY:` comment. |

### `crates/oxios-kernel/src/access_manager/mod.rs` — 0 issues

No `unwrap()` or `expect()` calls in production code. Already clean.

---

## Clippy Secondary Findings

### `uninlined_format_args` (format!("...", var) → format!("{var}"))

**scheduler.rs (1):**

| Line | Current | Fix |
|------|---------|-----|
| 494 | `format!("task {} not found in queue", task_id)` | `format!("task {task_id} not found in queue")` |

**cron.rs (5):**

| Line | Current | Fix |
|------|---------|-----|
| 211 | `format!("0 {}", expr)` | `format!("0 {expr}")` |
| 220 | `anyhow!("Invalid cron expression '{}': {}", expr, e)` | `anyhow!("Invalid cron expression '{expr}': {e}")` |
| 259 | `anyhow!("Job {} not found", id)` | `anyhow!("Job {id} not found")` |
| 274 | `anyhow!("Job {} not found", id)` | `anyhow!("Job {id} not found")` |
| 354 | `anyhow!("Job {} not found", id)` | `anyhow!("Job {id} not found")` |
| 500 | `format!("Timed out after {} seconds", timeout_secs)` | `format!("Timed out after {timeout_secs} seconds")` |

**orchestrator.rs (3):**

| Line | Current | Fix |
|------|---------|-----|
| 357 | `format!("User: {}", user_message)` | `format!("User: {user_message}")` |
| 836 | `format!("seeds/{}.json", key)` | `format!("seeds/{key}.json")` |
| 1205 | `format!("agent_groups/{}.json", group_id)` | `format!("agent_groups/{group_id}.json")` |

**budget.rs (2):**

| Line | Current | Fix |
|------|---------|-----|
| 167 | `format!("No budget configured for agent {}", agent_id)` | `format!("No budget configured for agent {agent_id}")` |
| 225 | `format!("No budget configured for agent {}", agent_id)` | `format!("No budget configured for agent {agent_id}")` |

**src/kernel.rs (10):**

| Line | Current | Fix |
|------|---------|-----|
| 121 | `format!("{}/{}", prefix, path)` | `format!("{prefix}/{path}")` |
| 123 | `format!("knowledge: create {}", p)` | `format!("knowledge: create {p}")` |
| 124 | `format!("knowledge: update {}", p)` | `format!("knowledge: update {p}")` |
| 125 | `format!("knowledge: delete {}", p)` | `format!("knowledge: delete {p}")` |
| 127 | `format!("knowledge: rename {} → {}", old, new)` | `format!("knowledge: rename {old} → {new}")` |
| 137 | `format!("{}/{}", prefix, old)` | `format!("{prefix}/{old}")` |
| 262 | `anyhow!("audit flush failed: {}", e)` | `anyhow!("audit flush failed: {e}")` |
| 462 | `format!(...)` (multi-line release URL) | Inline `latest_tag` |
| 605 | `format!("Failed to resolve model: {}", model_id)` | `format!("Failed to resolve model: {model_id}")` |
| 987 | `format!("OXIOS_MCP_{}_ARGS", name)` | `format!("OXIOS_MCP_{name}_ARGS")` |
| 990 | `format!("OXIOS_MCP_{}_ENV", name)` | `format!("OXIOS_MCP_{name}_ENV")` |

**access_manager/mod.rs (3):**

| Line | Current | Fix |
|------|---------|-----|
| 147 | `writeln!(f, "{}", line)` | `writeln!(f, "{line}")` |
| 547-549 | `format!("Path '{}' is outside workspace '{}' boundary", path, workspace_name)` | `format!("Path '{path}' is outside workspace '{workspace_name}' boundary")` |
| 566-568 | `format!("Path '{}' is outside assigned workspace '{}' boundary", path, assigned_workspace)` | `format!("Path '{path}' is outside assigned workspace '{assigned_workspace}' boundary")` |

### Unused imports / dead code

| File | Line | Issue | Fix |
|------|------|-------|-----|
| `orchestrator.rs` | 28 | `use crate::project::{ConversationBuffer, ProjectId, ProjectManager}` — `ProjectId` unused | Remove `ProjectId` from import |
| `orchestrator.rs` | 280 | unused variable `tag` in closure | Prefix with `_`: `_tag` |
| `orchestrator.rs` | 308 | unused variable `conversation_turns` | Prefix with `_`: `_conversation_turns` or remove |
| `src/kernel.rs` | 15 | unused import `RoutingStats` | Remove from import |

---

## Action Plan (Phase 2)

1. Fix 3 ACTIONABLE items (2 in `kernel.rs`, 1 in `orchestrator.rs`)
2. Add `// SAFETY:` comments to 9 SAFE items in `kernel.rs`
3. Fix 24 `uninlined_format_args` clippy warnings
4. Fix 4 unused import/variable warnings
5. Run `cargo test -p oxios-kernel` and `cargo clippy` to verify
