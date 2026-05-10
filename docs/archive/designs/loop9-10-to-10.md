# Loop 9: Oxios Agent OS — Complete 10/10 Design

> Brings every scoring dimension from current levels to 10/10.
> Total estimated effort: ~18 working days across 7 areas.

## Table of Contents

1. [Protocol 8→10](#1-protocol-810-1-day)
2. [Tool System 9→10](#2-tool-system-910-2-days)
3. [Multi-Agent 7→10](#3-multi-agent-710-3-days)
4. [Memory 8→10](#4-memory-810-2-days)
5. [Channels 7→10](#5-channels-710-5-days)
6. [Observability 7→10](#6-observability-710-2-days)
7. [Production 8→10](#7-production-810-3-days)
8. [Implementation Schedule](#8-implementation-schedule)

---

## 1. Protocol 8→10 (1 day)

### Problem

`OuroborosEngine.evaluate()` uses `parse_json` which silently falls back to defaults on parse failure. This means:

1. **Silent degradation**: A malformed LLM response produces a fake evaluation instead of surfacing the error.
2. **No mechanical-first**: The mechanical check (`output.contains(criterion)`) is too naïve — a criterion like "no compiler warnings" can't be substring-matched.
3. **No retry on bad output**: If the LLM produces invalid JSON, we accept garbage instead of retrying.
4. **No caching**: Re-evaluating the same seed+output pair burns tokens for no reason.

### Solution

#### 1.1 Structured Output Validation with Retry

```rust
// crates/oxios-ouroboros/src/parse.rs (new file)

use anyhow::{bail, Result};
use serde::de::DeserializeOwned;

/// Maximum retries for LLM JSON parse failures.
const MAX_PARSE_RETRIES: usize = 2;

/// Parse JSON from LLM output with retry capability.
///
/// Strips markdown fences, validates the result against a schema
/// description, and returns the parsed value. On parse failure,
/// the caller can use `retry_prompt` to ask the LLM again.
pub fn parse_llm_json<T: DeserializeOwned>(raw: &str) -> Result<T> {
    let trimmed = raw.trim();
    let json_str = if trimmed.starts_with("```") {
        let after_open = trimmed.find('\n').map(|i| i + 1).unwrap_or(0);
        let before_close = trimmed.rfind("```").unwrap_or(trimmed.len());
        &trimmed[after_open..before_close]
    } else {
        // Also try to find JSON object/array if wrapped in prose
        if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                &trimmed[start..=end]
            } else {
                trimmed
            }
        } else if let Some(start) = trimmed.find('[') {
            if let Some(end) = trimmed.rfind(']') {
                &trimmed[start..=end]
            } else {
                trimmed
            }
        } else {
            trimmed
        }
    };

    serde_json::from_str(json_str.trim()).map_err(|e| {
        anyhow::anyhow!(
            "LLM JSON parse failed: {}. Raw output (first 200 chars): '{}'",
            e,
            &raw[..raw.len().min(200)]
        )
    })
}

/// Build a retry prompt asking the LLM to fix its JSON.
pub fn retry_prompt(original_prompt: &str, raw_output: &str, error: &str) -> String {
    format!(
        "Your previous response was invalid JSON. The error was: {}\n\n\
         Your raw output was:\n```\n{}\n```\n\n\
         Please respond with ONLY valid JSON matching the requested schema. \
         Do not include any text before or after the JSON object.",
        error,
        &raw_output[..raw_output.len().min(500)]
    )
}
```

#### 1.2 Evaluation Cache

```rust
// crates/oxios-ouroboros/src/eval_cache.rs (new file)

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use parking_lot::Mutex;
use crate::evaluation::EvaluationResult;
use crate::seed::Seed;
use crate::protocol::ExecutionResult;

/// Cache key: hash of (seed_id, output_content).
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct EvalKey {
    seed_id: uuid::Uuid,
    output_hash: u64,
}

impl EvalKey {
    fn new(seed: &Seed, execution: &ExecutionResult) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        execution.output.hash(&mut hasher);
        Self {
            seed_id: seed.id,
            output_hash: hasher.finish(),
        }
    }
}

/// In-memory evaluation cache. Same seed + output → same result.
pub struct EvalCache {
    cache: Mutex<HashMap<EvalKey, EvaluationResult>>,
    max_entries: usize,
}

impl EvalCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            max_entries,
        }
    }

    /// Look up a cached evaluation.
    pub fn get(&self, seed: &Seed, execution: &ExecutionResult) -> Option<EvaluationResult> {
        let key = EvalKey::new(seed, execution);
        self.cache.lock().get(&key).cloned()
    }

    /// Store an evaluation result.
    pub fn put(&self, seed: &Seed, execution: &ExecutionResult, result: EvaluationResult) {
        let key = EvalKey::new(seed, execution);
        let mut cache = self.cache.lock();
        if cache.len() >= self.max_entries {
            // FIFO eviction: remove the first entry
            if let Some(first_key) = cache.keys().next().cloned() {
                cache.remove(&first_key);
            }
        }
        cache.insert(key, result);
    }
}
```

#### 1.3 Mechanical-First Evaluation (Enhanced)

The current mechanical check is `output.contains(criterion)`. This is too simple for criteria like "no compiler warnings" or "all tests pass". We need a structured mechanical evaluation layer:

```rust
// crates/oxios-ouroboros/src/evaluation.rs (modifications)

/// Result of mechanical (non-LLM) evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MechanicalEvalResult {
    /// Each criterion and whether it passed mechanically.
    pub criterion_results: Vec<CriterionResult>,
    /// Overall mechanical pass (all criteria passed).
    pub all_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriterionResult {
    pub criterion: String,
    pub passed: bool,
    pub reason: String,
}

impl MechanicalEvalResult {
    /// Run mechanical checks against acceptance criteria.
    ///
    /// Checks for common patterns:
    /// - "no errors/warnings" → check output doesn't contain "error:" or "warning:"
    /// - "all tests pass" → check for "passed" and absence of "failed"
    /// - "file exists" → check output mentions file creation
    /// - "returns X" → substring check
    /// - Otherwise: substring containment
    pub fn evaluate(criteria: &[String], output: &str) -> Self {
        let output_lower = output.to_lowercase();
        let mut results = Vec::new();

        for criterion in criteria {
            let c_lower = criterion.to_lowercase();
            let (passed, reason) = if c_lower.contains("no error") || c_lower.contains("no warning") {
                let has_error = output_lower.contains("error:");
                let has_warning = output_lower.contains("warning:");
                (!has_error && !has_warning, format!("error={} warning={}", has_error, has_warning))
            } else if c_lower.contains("all test") && c_lower.contains("pass") {
                let has_passed = output_lower.contains("passed") || output_lower.contains("✓");
                let has_failed = output_lower.contains("failed") || output_lower.contains("✗");
                (has_passed && !has_failed, format!("passed={} failed={}", has_passed, has_failed))
            } else if c_lower.contains("exit code") || c_lower.contains("exit status") {
                let has_zero = output_lower.contains("exit code 0") || output_lower.contains("exit status 0");
                (has_zero, format!("exit_code_0={}", has_zero))
            } else {
                // Default: substring containment
                let contains = output.contains(criterion);
                (contains, format!("substring_match={}", contains))
            };
            results.push(CriterionResult {
                criterion: criterion.clone(),
                passed,
                reason,
            });
        }

        let all_passed = results.iter().all(|r| r.passed);
        Self { criterion_results: results, all_passed }
    }
}
```

#### 1.4 Enhanced `evaluate()` Method

```rust
// crates/oxios-ouroboros/src/ouroboros_engine.rs (modifications to evaluate)

async fn evaluate(
    &self,
    seed: &Seed,
    execution: &ExecutionResult,
) -> Result<EvaluationResult> {
    self.set_phase(Phase::Evaluate);

    // Check cache first
    if let Some(cached) = self.eval_cache.get(seed, execution) {
        tracing::info!(seed_id = %seed.id, "Evaluation cache hit");
        return Ok(cached);
    }

    // Stage 1: Enhanced mechanical evaluation
    let mechanical = MechanicalEvalResult::evaluate(
        &seed.acceptance_criteria,
        &execution.output,
    );

    // If mechanical passes perfectly, skip LLM eval
    if mechanical.all_passed
        && mechanical.criterion_results.iter().all(|r| r.reason.starts_with("substring_match=false").not())
    {
        let result = EvaluationResult {
            mechanical_pass: true,
            semantic_pass: None,
            consensus_pass: None,
            score: 1.0,
            notes: mechanical.criterion_results.iter()
                .map(|r| format!("✓ {}", r.criterion))
                .collect(),
        };
        self.eval_cache.put(seed, execution, result.clone());
        return Ok(result);
    }

    // Stage 2: Semantic evaluation via LLM (with retry)
    let mechanical_notes: String = mechanical.criterion_results.iter()
        .map(|r| format!("- {}: {} ({})", r.criterion, r.passed, r.reason))
        .collect::<Vec<_>>()
        .join("\n");

    let user_message = format!(
        "## Goal\n{}\n\n## Acceptance Criteria\n{}\n\n\
         ## Mechanical Check Results\n{}\n\n\
         ## Execution Output (first 3000 chars)\n{}\n\n\
         Evaluate whether the execution output satisfies the goal and acceptance criteria.\n\
         Produce a JSON object:\n\
         - \"mechanical_pass\": {}\n\
         - \"semantic_pass\": true/false\n\
         - \"score\": 0.0 to 1.0\n\
         - \"notes\": list of evaluation notes\n\n\
         Respond with ONLY the JSON object. No prose before or after.",
        seed.goal,
        seed.acceptance_criteria.iter().enumerate()
            .map(|(i, c)| format!("{}. {}", i + 1, c))
            .collect::<Vec<_>>()
            .join("\n"),
        mechanical_notes,
        &execution.output[..execution.output.len().min(3000)],
        mechanical.all_passed,
    );

    let raw = self.llm_complete(EVALUATE_SYSTEM_PROMPT, &user_message).await?;
    let parsed = match parse_llm_json::<EvaluationResponse>(&raw) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "Evaluation JSON parse failed, retrying...");
            // Retry once with correction prompt
            let retry_msg = retry_prompt(&user_message, &raw, &e.to_string());
            let retry_raw = self.llm_complete(EVALUATE_SYSTEM_PROMPT, &retry_msg).await?;
            parse_llm_json::<EvaluationResponse>(&retry_raw).unwrap_or_else(|e2| {
                tracing::warn!(error = %e2, "Retry also failed, using mechanical-only result");
                EvaluationResponse {
                    mechanical_pass: mechanical.all_passed,
                    semantic_pass: mechanical.all_passed,
                    score: if mechanical.all_passed { 0.7 } else { 0.3 },
                    notes: vec![format!("Evaluation parsing failed: {}", e)],
                }
            })
        }
    };

    let result = EvaluationResult {
        mechanical_pass: parsed.mechanical_pass,
        semantic_pass: Some(parsed.semantic_pass),
        consensus_pass: None,
        score: parsed.score,
        notes: parsed.notes,
    };

    self.eval_cache.put(seed, execution, result.clone());

    tracing::info!(
        seed_id = %seed.id,
        mechanical = result.mechanical_pass,
        semantic = ?result.semantic_pass,
        score = result.score,
        "Evaluation complete"
    );

    Ok(result)
}
```

#### 1.5 Apply Same Retry Pattern to Interview and Seed Generation

The same `parse_llm_json` + retry pattern should be applied to `interview()` and `generate_seed()` methods. Extract a helper:

```rust
impl OuroborosEngine {
    /// Run LLM completion, parse as JSON, retry once on failure.
    async fn llm_json<T: serde::de::DeserializeOwned>(
        &self,
        system_prompt: &str,
        user_message: &str,
    ) -> Result<T> {
        let raw = self.llm_complete(system_prompt, user_message).await?;
        match parse_llm_json::<T>(&raw) {
            Ok(parsed) => Ok(parsed),
            Err(e) => {
                tracing::warn!(error = %e, "JSON parse failed, retrying with correction");
                let retry_msg = retry_prompt(user_message, &raw, &e.to_string());
                let retry_raw = self.llm_complete(system_prompt, &retry_msg).await?;
                parse_llm_json::<T>(&retry_raw).map_err(|e2| {
                    anyhow::anyhow!("JSON parse failed after retry: {}", e2)
                })
            }
        }
    }
}
```

Then replace all `Self::parse_json(&raw).unwrap_or_else(...)` with `self.llm_json(...)`.

### File Changes

| File | Action |
|------|--------|
| `crates/oxios-ouroboros/src/parse.rs` | **New** — `parse_llm_json`, `retry_prompt` |
| `crates/oxios-ouroboros/src/eval_cache.rs` | **New** — `EvalCache` |
| `crates/oxios-ouroboros/src/evaluation.rs` | **Modify** — Add `MechanicalEvalResult` |
| `crates/oxios-ouroboros/src/ouroboros_engine.rs` | **Modify** — Add `llm_json` helper, `eval_cache` field, enhanced `evaluate()` |
| `crates/oxios-ouroboros/src/lib.rs` | **Modify** — Add `mod parse; mod eval_cache;` |

### Test Strategy

```rust
#[test]
fn test_parse_llm_json_valid() { /* valid JSON → Ok */ }
fn test_parse_llm_json_markdown_fence() { /* ```json ... ``` → Ok */ }
fn test_parse_llm_json_with_prose() { /* "Here is the JSON: {...}" → Ok */ }
fn test_parse_llm_json_invalid() { /* "not json" → Err */ }

#[test]
fn test_eval_cache_hit() { /* same seed+output → cached result */ }
fn test_eval_cache_miss() { /* different output → no cache */ }
fn test_eval_cache_eviction() { /* exceeds max_entries → FIFO eviction */ }

#[test]
fn test_mechanical_eval_no_errors() {
    let r = MechanicalEvalResult::evaluate(
        &["No errors or warnings".into()],
        &"Build succeeded.\nDone.".to_string(),
    );
    assert!(r.all_passed);
}

#[test]
fn test_mechanical_eval_all_tests_pass() {
    let r = MechanicalEvalResult::evaluate(
        &["All tests must pass".into()],
        &"test_foo ✓ passed\ntest_bar ✓ passed\n2 passed".to_string(),
    );
    assert!(r.all_passed);
}
```

### Dependencies

No new crate dependencies. All implementation uses `serde_json`, `parking_lot`, and `uuid` (already in workspace).

---

## 2. Tool System 9→10 (2 days)

### Problem

- The default Containerfile (`DEFAULT_CONTAINERFILE`) installs only basic tools: `curl git ripgrep jq sqlite3 bash python3`. No compiler toolchain.
- Agents that need to compile/test Rust, TypeScript, or other languages can't do so inside containers.
- No tool health check: agents start executing without knowing if tools are available.
- No per-program tool dependency declaration.

### Solution

#### 2.1 Containerfile Templates

```rust
// crates/oxios-kernel/src/container_manager.rs (additions)

/// Containerfile template with full dev toolchain.
const DEV_TOOLCHAIN_CONTAINERFILE: &str = r#"# Oxios Dev Containerfile
FROM debian:bookworm-slim

# Base tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl git ripgrep jq sqlite3 bash python3 ca-certificates \
    build-essential pkg-config libssl-dev \
    nodejs npm \
    && rm -rf /var/lib/apt/lists/*

# Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Common Rust tools
RUN cargo install cargo-watch cargo-audit cargo-outdated 2>/dev/null || true

WORKDIR /workspace
CMD ["/bin/bash"]
"#;

/// Containerfile template for a specific language.
fn containerfile_for_language(lang: &str) -> &'static str {
    match lang {
        "rust" => DEV_TOOLCHAIN_CONTAINERFILE,
        _ => DEFAULT_CONTAINERFILE,
    }
}
```

#### 2.2 Workspace Initialization Script

```rust
// crates/oxios-kernel/src/container_manager.rs

impl ContainerManager {
    /// Create a new container with a specific toolchain template.
    pub async fn new_container_with_toolchain(
        &self,
        name: &str,
        toolchain: &str,
    ) -> Result<()> {
        let container_dir = self.containers_base.join(name);
        if container_dir.exists() {
            bail!("Container '{}' already exists", name);
        }

        // ... same directory creation as new_container ...

        // Select Containerfile based on toolchain
        let containerfile = containerfile_for_language(toolchain);
        tokio::fs::write(container_dir.join("Containerfile"), containerfile).await?;

        // Write init script
        let init_script = r#"#!/bin/bash
# Oxios workspace initialization
set -e
echo "Initializing Oxios workspace..."

# Verify core tools
for tool in bash git curl rg jq python3; do
    if command -v "$tool" &>/dev/null; then
        echo "  ✓ $tool"
    else
        echo "  ✗ $tool (missing)"
    fi
done

# Verify language-specific tools
if command -v rustc &>/dev/null; then
    echo "  ✓ rustc $(rustc --version)"
fi
if command -v cargo &>/dev/null; then
    echo "  ✓ cargo $(cargo --version)"
fi
if command -v node &>/dev/null; then
    echo "  ✓ node $(node --version)"
fi

echo "Workspace ready."
"#;
        tokio::fs::write(
            container_dir.join("workspace").join(".oxios-init.sh"),
            init_script,
        )
        .await?;

        // Persist metadata
        let info = ContainerInfo {
            name: name.to_string(),
            image_tag: DEFAULT_IMAGE_TAG.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            running: false,
            toolchain: Some(toolchain.to_string()),
            tools_verified: false,
        };
        self.state_store
            .save_json("containers", name, &info)
            .await?;

        Ok(())
    }
}
```

#### 2.3 Tool Health Check

```rust
// crates/oxios-kernel/src/container_manager.rs

/// Result of a tool health check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolHealthReport {
    pub container: String,
    pub tools: Vec<ToolStatus>,
    pub all_healthy: bool,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStatus {
    pub name: String,
    pub available: bool,
    pub version: Option<String>,
}

impl ContainerManager {
    /// Check health of all expected tools inside a running container.
    pub async fn check_tool_health(&self, name: &str) -> Result<ToolHealthReport> {
        let tools = [
            ("bash", "bash --version"),
            ("git", "git --version"),
            ("curl", "curl --version"),
            ("ripgrep", "rg --version"),
            ("jq", "jq --version"),
            ("python3", "python3 --version"),
            ("rustc", "rustc --version"),
            ("cargo", "cargo --version"),
            ("node", "node --version"),
            ("npm", "npm --version"),
        ];

        let mut tool_statuses = Vec::new();
        for (tool_name, version_cmd) in &tools {
            let result = self
                .exec_in_container(
                    name,
                    &["bash".into(), "-c".into(), version_cmd.to_string().into()],
                    None,
                )
                .await;

            match result {
                Ok(exec_result) if exec_result.success => {
                    let version = exec_result
                        .output
                        .lines()
                        .next()
                        .map(|s| s.to_string());
                    tool_statuses.push(ToolStatus {
                        name: tool_name.to_string(),
                        available: true,
                        version,
                    });
                }
                _ => {
                    tool_statuses.push(ToolStatus {
                        name: tool_name.to_string(),
                        available: false,
                        version: None,
                    });
                }
            }
        }

        let all_healthy = tool_statuses.iter().all(|t| t.available);

        // Update container metadata
        if let Ok(Some(mut info)) = self.state_store.load_json::<ContainerInfo>("containers", name).await {
            info.tools_verified = all_healthy;
            self.state_store.save_json("containers", name, &info).await?;
        }

        Ok(ToolHealthReport {
            container: name.to_string(),
            tools: tool_statuses,
            all_healthy,
            checked_at: chrono::Utc::now().to_rfc3339(),
        })
    }
}
```

#### 2.4 Tool Health API Endpoint

```rust
// channels/oxios-web/src/routes/system.rs (additions)

/// GET /api/containers/:name/tools — Tool health check
pub(crate) async fn handle_container_tools(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<ToolHealthReport>, AppError> {
    state
        .container_manager
        .check_tool_health(&name)
        .await
        .map(Json)
        .map_err(|e| AppError::Internal(e.to_string()))
}
```

### File Changes

| File | Action |
|------|--------|
| `crates/oxios-kernel/src/container_manager.rs` | **Modify** — Add toolchain templates, `new_container_with_toolchain`, `check_tool_health` |
| `crates/oxios-kernel/src/container_manager.rs` | **Modify** — Add `ToolHealthReport`, `ToolStatus` structs |
| `channels/oxios-web/src/routes/system.rs` | **Modify** — Add `handle_container_tools` endpoint |
| `channels/oxios-web/src/routes/mod.rs` | **Modify** — Register new route |

### Test Strategy

```rust
#[tokio::test]
async fn test_new_container_with_toolchain_creates_dev_containerfile() { /* ... */ }
#[tokio::test]
async fn test_tool_health_check_on_stopped_container() { /* should return error */ }

// E2E (requires container runtime):
#[tokio::test]
#[ignore] // requires Apple Container runtime
async fn test_tool_health_check_running_container() { /* ... */ }
```

### Dependencies

No new crate dependencies.

---

## 3. Multi-Agent 7→10 (3 days)

### Problem

1. **Sequential execution**: `delegate_subtasks` iterates with a `for` loop — subtasks run one at a time.
2. **Messages never delivered**: `A2AProtocol.send_message()` stores in `message_queue` but `receive_messages()` is never called by any agent.
3. **No group status API**: No way to query group state from the web API.
4. **No capability routing**: Subtasks aren't routed to agents with matching capabilities.

### Solution

#### 3.1 Parallel Execution with `tokio::JoinSet`

```rust
// crates/oxios-kernel/src/orchestrator.rs — replace delegate_subtasks

pub async fn delegate_subtasks(
    &self,
    subtasks: Vec<SubTask>,
    parent_seed: &Seed,
) -> Result<Vec<SubTask>> {
    use crate::agent_group::AgentGroup;
    use tokio::task::JoinSet;

    let descriptions: Vec<String> = subtasks.iter().map(|st| st.description.clone()).collect();
    let group = AgentGroup::new(parent_seed, descriptions);
    let group_id = group.id;

    self.event_bus.publish(KernelEvent::AgentGroupCreated {
        group_id,
        agent_count: group.agents.len(),
    })?;

    tracing::info!(
        group_id = %group_id,
        agent_count = group.agents.len(),
        "Starting parallel multi-agent execution"
    );

    // Launch all subtasks concurrently using JoinSet
    let mut join_set: JoinSet<(usize, AgentId, Result<ExecutionResult>)> = JoinSet::new();

    for (idx, agent_entry) in group.agents.iter().enumerate() {
        let subtask_id = subtasks[idx].id;
        let child_seed = agent_entry.seed.clone();
        let agent_id = agent_entry.id;
        let lifecycle = self.lifecycle.clone();

        join_set.spawn(async move {
            let result = lifecycle.spawn_and_run(&child_seed, Priority::Normal).await;
            (idx, agent_id, result)
        });
    }

    // Collect results as they complete
    let mut completed = vec![None; subtasks.len()];
    while let Some(join_result) = join_set.join_next().await {
        match join_result {
            Ok((idx, agent_id, Ok(exec_result))) => {
                let _ = self.event_bus.publish(KernelEvent::AgentGroupMemberCompleted {
                    group_id,
                    agent_id,
                    success: exec_result.success,
                });
                completed[idx] = Some(SubTask {
                    id: subtasks[idx].id,
                    description: subtasks[idx].description.clone(),
                    required_capability: subtasks[idx].required_capability.clone(),
                    result: Some(exec_result.output.clone()),
                    success: exec_result.success,
                });
            }
            Ok((idx, agent_id, Err(e))) => {
                tracing::warn!(subtask_index = idx, error = %e, "Subtask failed");
                let _ = self.event_bus.publish(KernelEvent::AgentGroupMemberCompleted {
                    group_id,
                    agent_id,
                    success: false,
                });
                completed[idx] = Some(SubTask {
                    id: subtasks[idx].id,
                    description: subtasks[idx].description.clone(),
                    required_capability: subtasks[idx].required_capability.clone(),
                    result: Some(format!("Failed: {e}")),
                    success: false,
                });
            }
            Err(e) => {
                tracing::error!(error = %e, "JoinSet task panicked");
            }
        }
    }

    // Reap zombies after multi-agent run
    self.lifecycle.reap_zombies();

    let completed: Vec<SubTask> = completed.into_iter().flatten().collect();
    let succeeded = completed.iter().filter(|r| r.success).count();
    let total = completed.len();

    tracing::info!(
        group_id = %group_id,
        succeeded,
        total,
        "Parallel multi-agent execution complete"
    );

    Ok(completed)
}
```

**Key change**: `AgentLifecycleManager` must be `Clone`-safe. It holds `Arc` wrappers for all fields, so adding `#[derive(Clone)]` is sufficient:

```rust
#[derive(Clone)]
pub struct AgentLifecycleManager {
    supervisor: Arc<dyn Supervisor>,
    scheduler: Arc<AgentScheduler>,
    access_manager: Arc<parking_lot::Mutex<AccessManager>>,
    a2a: Arc<A2AProtocol>,
    event_bus: EventBus,
}
```

#### 3.2 Agent-to-Agent Message Delivery

The current `A2AProtocol` stores messages but never delivers them. We need a delivery mechanism:

```rust
// crates/oxios-kernel/src/a2a.rs — additions to A2AProtocol

impl A2AProtocol {
    /// Deliver all pending messages for an agent by publishing them
    /// on the event bus. Called by the lifecycle manager after fork.
    pub async fn deliver_pending_messages(
        &self,
        agent_id: AgentId,
    ) -> Result<Vec<A2ARequest>> {
        let messages = self.receive_messages(agent_id).await;
        for msg in &messages {
            self.event_bus.publish(KernelEvent::MessageReceived {
                from: msg.from,
                content: format!(
                    "[{}] {:?}",
                    msg.message.type_name(),
                    msg.request_id
                ),
            })?;
        }
        Ok(messages)
    }

    /// Send a message and wait for a response (request-response pattern).
    pub async fn send_and_wait(
        &self,
        from: AgentId,
        to: AgentId,
        message: A2AMessage,
        timeout: std::time::Duration,
    ) -> Result<A2AResponse> {
        let request_id = self.send_message(from, to, message).await?;

        // Poll for response in the sender's queue
        let start = std::time::Instant::now();
        loop {
            let messages = self.receive_messages(from).await;
            for msg in messages {
                if let A2AMessage::ResultSharing { task_id, result, summary } = &msg.message {
                    if *task_id == request_id {
                        return Ok(A2AResponse::success(
                            request_id,
                            to,
                            from,
                            result.clone(),
                        ));
                    }
                }
            }
            if start.elapsed() > timeout {
                anyhow::bail!("A2A response timeout after {:?}", timeout);
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
}
```

And hook delivery into the lifecycle manager:

```rust
// crates/oxios-kernel/src/agent_lifecycle.rs — modify spawn_and_run

pub async fn spawn_and_run(&self, seed: &Seed, priority: Priority) -> Result<ExecutionResult> {
    // 1. Fork
    let agent_id = self.supervisor.fork(seed).await?;
    // ... (existing A2A registration, permissions, scheduler) ...

    // 4.5. Deliver any pending A2A messages to this agent
    if let Err(e) = self.a2a.deliver_pending_messages(agent_id).await {
        tracing::debug!(agent_id = %agent_id, error = %e, "No pending A2A messages");
    }

    // 5. Run
    // ... (existing execution + cleanup) ...
}
```

#### 3.3 Agent Group API

```rust
// channels/oxios-web/src/routes/agent_groups.rs (new file)

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;
use std::sync::Arc;
use crate::error::AppError;
use crate::server::AppState;

#[derive(Debug, Serialize)]
pub struct AgentGroupSummary {
    pub id: String,
    pub parent_seed_id: String,
    pub agent_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub all_done: bool,
}

/// GET /api/agent-groups — List all agent groups.
pub(crate) async fn handle_agent_groups_list(
    state: State<Arc<AppState>>,
) -> Result<Json<Vec<AgentGroupSummary>>, AppError> {
    let groups = state
        .state_store
        .list_category("agent_groups")
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut summaries = Vec::new();
    for name in groups {
        if let Ok(Some(group)) = state
            .state_store
            .load_json::<oxios_kernel::AgentGroup>("agent_groups", &name)
            .await
        {
            summaries.push(AgentGroupSummary {
                id: group.id.to_string(),
                parent_seed_id: group.parent_seed_id.to_string(),
                agent_count: group.agents.len(),
                completed_count: group.count_by_status(oxios_kernel::AgentGroupStatus::Completed),
                failed_count: group.count_by_status(oxios_kernel::AgentGroupStatus::Failed),
                all_done: group.all_done(),
            });
        }
    }
    Ok(Json(summaries))
}

/// GET /api/agent-groups/:id — Get a specific agent group.
pub(crate) async fn handle_agent_group_get(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let group: Option<oxios_kernel::AgentGroup> = state
        .state_store
        .load_json("agent_groups", &id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    match group {
        Some(g) => Ok(Json(serde_json::to_value(g).unwrap())),
        None => Err(AppError::NotFound("agent group not found".into())),
    }
}
```

The orchestrator must also persist group state:

```rust
// In delegate_subtasks, after group creation:
self.state_store
    .save_json("agent_groups", &group_id.to_string(), &group)
    .await?;
```

#### 3.4 Group Lifecycle Events via SSE

Add SSE (Server-Sent Events) stream for real-time group updates:

```rust
// channels/oxios-web/src/routes/sse.rs (new file)

use axum::response::sse::{Event, Sse};
use futures::stream::{self, Stream};
use std::convert::Infallible;
use tokio_stream::StreamExt;

/// GET /api/events — SSE stream for real-time kernel events.
pub(crate) async fn handle_events(
    state: State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut event_rx = state.event_bus.subscribe();

    let stream = async_stream::stream! {
        loop {
            match event_rx.recv().await {
                Ok(kernel_event) => {
                    let data = serde_json::to_string(&kernel_event).unwrap_or_default();
                    yield Ok(Event::default().data(data));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    let data = format!("{{\"warn\":\"lagged {} events\"}}", n);
                    yield Ok(Event::default().data(data));
                }
                Err(_) => break,
            }
        }
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(30))
            .text("ping"),
    )
}
```

#### 3.5 Capability-Based Task Routing

```rust
// crates/oxios-kernel/src/orchestrator.rs — enhance split_into_subtasks

fn split_into_subtasks(seed: &Seed) -> Vec<SubTask> {
    seed.acceptance_criteria
        .iter()
        .map(|criterion| {
            let description = format!("{}: {}", seed.goal, criterion);
            let capability = infer_capability(criterion);
            SubTask::new(description).with_capability(capability)
        })
        .collect()
}

/// Infer required agent capability from acceptance criterion text.
fn infer_capability(criterion: &str) -> String {
    let lower = criterion.to_lowercase();
    if lower.contains("test") || lower.contains("spec") { "testing".into() }
    else if lower.contains("review") || lower.contains("lint") { "code-review".into() }
    else if lower.contains("refactor") || lower.contains("improve") { "refactoring".into() }
    else if lower.contains("debug") || lower.contains("fix") { "debugging".into() }
    else if lower.contains("document") || lower.contains("doc") { "documentation".into() }
    else { "general".into() }
}
```

### File Changes

| File | Action |
|------|--------|
| `crates/oxios-kernel/src/orchestrator.rs` | **Modify** — Replace sequential `delegate_subtasks` with `JoinSet`-based parallel version, add `infer_capability` |
| `crates/oxios-kernel/src/agent_lifecycle.rs` | **Modify** — Add `#[derive(Clone)]`, deliver A2A messages after fork |
| `crates/oxios-kernel/src/a2a.rs` | **Modify** — Add `deliver_pending_messages`, `send_and_wait` |
| `channels/oxios-web/src/routes/agent_groups.rs` | **New** — Group list + detail endpoints |
| `channels/oxios-web/src/routes/sse.rs` | **New** — SSE event stream |
| `channels/oxios-web/src/routes/mod.rs` | **Modify** — Register group + SSE routes |

### Test Strategy

```rust
#[tokio::test]
async fn test_delegate_subtasks_parallel() {
    // Create 3 subtasks, verify they run concurrently (wall time < sequential)
}

#[tokio::test]
async fn test_a2a_deliver_pending_messages() {
    // Send message, then deliver, verify event bus publication
}

#[tokio::test]
async fn test_a2a_send_and_wait_timeout() {
    // Send and wait with short timeout → should bail
}

#[tokio::test]
async fn test_infer_capability() {
    assert_eq!(infer_capability("All tests must pass"), "testing");
    assert_eq!(infer_capability("Code review findings addressed"), "code-review");
    assert_eq!(infer_capability("Fix the null pointer bug"), "debugging");
}
```

### Dependencies

- `tokio` (already in workspace, `JoinSet` from `tokio::task`)
- `tokio-stream` (for SSE, add to `channels/oxios-web/Cargo.toml`)
- `async-stream` (for SSE stream macro, add to `channels/oxios-web/Cargo.toml`)

---

## 4. Memory 8→10 (2 days)

### Problem

1. **In-memory only vector index**: On restart, the index must be rebuilt by scanning all stored memories — slow and O(n) at startup.
2. **No auto-curation**: Memories accumulate forever — no deduplication, no importance decay, no pruning.
3. **No memory budget**: Unbounded growth means startup time degrades over time.

### Solution

#### 4.1 Persist Vector Index to Disk

```rust
// crates/oxios-kernel/src/memory.rs — additions

use std::path::PathBuf;

/// Serialized vector index for persistence.
#[derive(Serialize, Deserialize)]
struct VectorIndexSnapshot {
    /// Version for migration compatibility.
    version: u32,
    /// Map of entry ID to TextVector.
    entries: HashMap<String, TextVector>,
    /// Timestamp of last save.
    saved_at: DateTime<Utc>,
}

impl MemoryManager {
    /// Path to the vector index snapshot file.
    fn index_snapshot_path(&self) -> PathBuf {
        self.state_store.base_path.join("memory").join("vector_index.json")
    }

    /// Save the vector index to disk.
    pub async fn save_index_snapshot(&self) -> Result<()> {
        let snapshot = {
            let index = self.vector_index.read();
            VectorIndexSnapshot {
                version: 1,
                entries: index.clone(),
                saved_at: Utc::now(),
            }
        };

        let dir = self.state_store.base_path.join("memory");
        tokio::fs::create_dir_all(&dir).await?;

        let json = serde_json::to_string(&snapshot)?;
        let path = self.index_snapshot_path();
        tokio::fs::write(&path, json).await?;

        tracing::info!(
            entries = snapshot.entries.len(),
            path = %path.display(),
            "Vector index snapshot saved"
        );
        Ok(())
    }

    /// Load the vector index from disk. Falls back to full rebuild if
    /// the snapshot is missing or corrupt.
    pub async fn load_index_snapshot(&self) -> Result<()> {
        let path = self.index_snapshot_path();
        if !path.exists() {
            tracing::info!("No vector index snapshot found, rebuilding from storage");
            return self.rebuild_index().await;
        }

        let json = tokio::fs::read_to_string(&path).await?;
        let snapshot: VectorIndexSnapshot = match serde_json::from_str(&json) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "Corrupt index snapshot, rebuilding");
                return self.rebuild_index().await;
            }
        };

        {
            let mut index = self.vector_index.write();
            *index = snapshot.entries;
        }

        tracing::info!(
            entries = self.vector_index.read().len(),
            age = ?(Utc::now() - snapshot.saved_at),
            "Vector index loaded from snapshot"
        );
        Ok(())
    }
}
```

#### 4.2 Auto-Deduplication

```rust
// crates/oxios-kernel/src/memory.rs — additions

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

impl MemoryManager {
    /// Compute a content hash for deduplication.
    fn content_hash(content: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        content.to_lowercase().trim().hash(&mut hasher);
        hasher.finish()
    }

    /// Check if a memory entry is a duplicate of an existing one.
    pub async fn is_duplicate(&self, entry: &MemoryEntry) -> Result<bool> {
        let hash = Self::content_hash(&entry.content);

        // Check against existing entries of the same type
        let existing = self.list(entry.memory_type, 100).await?;
        for existing_entry in &existing {
            let existing_hash = Self::content_hash(&existing_entry.content);
            if hash == existing_hash {
                return Ok(true);
            }
        }

        // Also check semantic similarity (near-duplicates)
        let query_vector = TextVector::from_text(&entry.content);
        let index = self.vector_index.read();
        for (id, vector) in index.iter() {
            let sim = query_vector.cosine_similarity(vector);
            if sim > 0.95 {
                // Near-identical content
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Store a memory entry with duplicate detection.
    /// Returns the entry ID (either new or existing duplicate).
    pub async fn remember_unique(&self, entry: MemoryEntry) -> Result<String> {
        if self.is_duplicate(&entry).await? {
            tracing::debug!(
                content_preview = &entry.content[..entry.content.len().min(80)],
                "Skipping duplicate memory"
            );
            // Return the ID but don't store
            return Ok(format!("dup-{}", entry.id));
        }
        self.remember(entry).await
    }
}
```

#### 4.3 Importance Decay and Memory Budget

```rust
// crates/oxios-kernel/src/memory.rs — additions

/// Memory budget configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBudget {
    /// Maximum entries per memory type.
    pub max_per_type: usize,
    /// Importance decay factor per day (0.0–1.0, higher = faster decay).
    pub decay_factor: f32,
    /// Minimum importance before pruning.
    pub prune_threshold: f32,
}

impl Default for MemoryBudget {
    fn default() -> Self {
        Self {
            max_per_type: 500,
            decay_factor: 0.01,
            prune_threshold: 0.1,
        }
    }
}

impl MemoryManager {
    /// Compute effective importance with decay based on age and access.
    pub fn effective_importance(entry: &MemoryEntry) -> f32 {
        let age_hours = (Utc::now() - entry.created_at).num_hours() as f32;
        let age_days = age_hours / 24.0;
        let recency_boost = 1.0 / (1.0 + age_days * 0.1);

        // Formula: importance * recency_weight * (1 + log(access_count + 1))
        let access_boost = (1.0 + (entry.access_count as f32 + 1.0).ln());
        entry.importance * recency_boost * access_boost
    }

    /// Run curation: decay, dedup, prune, enforce budget.
    pub async fn curate(&self, budget: &MemoryBudget) -> Result<CurationReport> {
        let mut report = CurationReport::default();

        for mt in &[
            MemoryType::Conversation,
            MemoryType::Session,
            MemoryType::Fact,
            MemoryType::Episode,
            MemoryType::Knowledge,
        ] {
            let entries = self.list(*mt, budget.max_per_type * 2).await?;
            report.total_scanned += entries.len();

            // Apply decay and identify entries to prune
            let mut to_prune = Vec::new();
            for entry in &entries {
                let eff = Self::effective_importance(entry);
                if eff < budget.prune_threshold && entry.importance < 0.3 {
                    to_prune.push(entry.id.clone());
                }
            }

            // Enforce budget: if over max_per_type, evict lowest importance
            let excess = entries.len().saturating_sub(budget.max_per_type);
            if excess > 0 {
                let mut scored: Vec<_> = entries
                    .iter()
                    .map(|e| (e.id.clone(), Self::effective_importance(e)))
                    .collect();
                scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                for (id, score) in scored.into_iter().take(excess) {
                    if !to_prune.contains(&id) {
                        to_prune.push(id);
                    }
                }
            }

            // Prune
            for id in &to_prune {
                if self.forget(id, *mt).await? {
                    report.pruned += 1;
                }
            }
        }

        // Remove pruned entries from vector index
        {
            let mut index = self.vector_index.write();
            // Rebuild from scratch to clean up orphaned entries
        }

        report.index_size_after = self.vector_index.read().len();

        // Save snapshot after curation
        self.save_index_snapshot().await?;

        tracing::info!(
            pruned = report.pruned,
            total = report.total_scanned,
            "Memory curation complete"
        );

        Ok(report)
    }
}

#[derive(Debug, Default, Serialize)]
pub struct CurationReport {
    pub total_scanned: usize,
    pub pruned: usize,
    pub index_size_after: usize,
}
```

#### 4.4 Background Curation Task

```rust
// crates/oxios-kernel/src/memory.rs — background task

impl MemoryManager {
    /// Spawn a background curation task that runs periodically.
    pub fn spawn_curation_task(
        self: &Arc<Self>,
        interval: std::time::Duration,
        budget: MemoryBudget,
    ) -> tokio::task::JoinHandle<()> {
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            loop {
                interval_timer.tick().await;
                match manager.curate(&budget).await {
                    Ok(report) => {
                        tracing::info!(
                            pruned = report.pruned,
                            index_size = report.index_size_after,
                            "Periodic memory curation"
                        );
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Memory curation failed");
                    }
                }
            }
        })
    }
}
```

#### 4.5 Integrate Snapshot into Startup

```rust
// In Kernel::builder() or wherever MemoryManager is initialized:

let memory_manager = Arc::new(MemoryManager::new(state_store.clone()).with_config(&config.memory));

// Load from snapshot first (fast), fall back to full rebuild (slow)
memory_manager.load_index_snapshot().await?;

// Spawn background curation (every 6 hours)
let budget = MemoryBudget::default();
memory_manager.spawn_curation_task(
    std::time::Duration::from_secs(6 * 3600),
    budget,
);
```

### File Changes

| File | Action |
|------|--------|
| `crates/oxios-kernel/src/memory.rs` | **Modify** — Add `VectorIndexSnapshot`, `save_index_snapshot`, `load_index_snapshot`, `is_duplicate`, `remember_unique`, `effective_importance`, `curate`, `spawn_curation_task`, `MemoryBudget`, `CurationReport` |
| `crates/oxios-kernel/src/config.rs` | **Modify** — Add `MemoryBudget` fields to `MemoryConfig` |
| `crates/oxios-kernel/src/kernel.rs` (or wherever initialized) | **Modify** — Use `load_index_snapshot` at startup, spawn curation task |

### Test Strategy

```rust
#[tokio::test]
async fn test_save_and_load_index_snapshot() {
    // Store memories → save snapshot → create new manager → load → verify index
}

#[tokio::test]
async fn test_duplicate_detection_exact() {
    // Store same content twice → second call returns dup-*
}

#[tokio::test]
async fn test_duplicate_detection_semantic() {
    // Store "Rust is a systems programming language"
    // Try storing "Rust is a programming language for systems"
    // → should be detected as near-duplicate
}

#[test]
fn test_effective_importance_fresh() {
    let entry = make_entry("test", MemoryType::Fact);
    let eff = MemoryManager::effective_importance(&entry);
    assert!(eff > 0.0);
}

#[tokio::test]
async fn test_curate_prunes_low_importance() {
    // Store entries with importance 0.05 → curate with threshold 0.1 → pruned
}

#[tokio::test]
async fn test_curate_enforces_budget() {
    // Store 600 entries, budget max_per_type=500 → curate → 500 remain
}
```

### Dependencies

No new crate dependencies.

---

## 5. Channels 7→10 (5 days)

### Problem

- No TUI, no interactive CLI, no Telegram channel
- Only `oxios-web` exists as a channel

### Solution

This section references the existing detailed designs:
- `docs/designs/loop7-cli-channel.md` — CLI interactive channel
- `docs/designs/loop7-tui-design.md` — TUI with ratatui

#### 5.1 CLI Channel (Days 13–14)

Create `channels/oxios-cli/` implementing the design from `loop7-cli-channel.md`. Key summary:

**New crate: `channels/oxios-cli/`**

```
channels/oxios-cli/
├── Cargo.toml
└── src/
    ├── lib.rs           — CliChannel, CliChannelHandle
    ├── channel.rs       — Channel trait impl
    ├── session.rs       — Session, SessionStore
    ├── interactive.rs   — InteractiveLoop with reedline
    ├── commands.rs      — MetaCommand parsing (.quit, .help, .reset, etc.)
    └── output.rs        — Phase indicators, ANSI formatting
```

**Cargo.toml additions:**
```toml
[dependencies]
oxios-gateway = { path = "../../crates/oxios-gateway" }
reedline = "0.38"
tokio = { version = "1", features = ["sync", "rt", "macros"] }
async-trait = "0.1"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
anyhow = "1"
tracing = "0.1"
```

**Core types** (from existing design, adapted to current codebase):

```rust
// channels/oxios-cli/src/channel.rs

pub struct CliChannel {
    name: String,
    incoming_tx: tokio::sync::mpsc::Sender<IncomingMessage>,
    outgoing_rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<OutgoingMessage>>,
    response_tx: tokio::sync::mpsc::Sender<OutgoingMessage>,
}

pub struct CliChannelHandle {
    outgoing_tx: tokio::sync::mpsc::Sender<OutgoingMessage>,
    response_rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<OutgoingMessage>>,
}
```

**Integration in `src/main.rs`:**
```rust
Some(Command::Chat) => {
    let kernel = Kernel::builder()
        .config_path(config_path.to_path_buf())
        .build()
        .await?;

    let (cli_channel, handle) = CliChannel::new(256);
    kernel.gateway.register(Box::new(cli_channel)).await;

    // Start web server in background if configured
    let web_handle = start_web_if_needed(&kernel).await;

    // Run interactive loop (blocks)
    let mut loop_ = oxios_cli::InteractiveLoop::new(handle, kernel);
    loop_.run().await?;

    web_handle.abort();
    Ok(())
}
```

#### 5.2 TUI Channel (Days 15–17)

Create `channels/oxios-tui/` implementing the design from `loop7-tui-design.md`. Key summary:

**New crate: `channels/oxios-tui/`**

```
channels/oxios-tui/
├── Cargo.toml
└── src/
    ├── lib.rs              — App::new(kernel).run(terminal)
    ├── app.rs              — Main App struct, event loop
    ├── tabs.rs             — Tab enum (Dashboard, Chat, Agents, Gardens, Logs)
    ├── panels/
    │   ├── mod.rs
    │   ├── dashboard.rs    — System overview
    │   ├── chat.rs         — Interactive chat (uses CliChannel internally)
    │   ├── agents.rs       — Agent list + management
    │   ├── gardens.rs      — Container management
    │   └── logs.rs         — Real-time event stream
    ├── widgets/
    │   ├── mod.rs
    │   ├── phase_indicator.rs
    │   └── agent_card.rs
    └── events.rs           — EventBus subscription → App state updates
```

**Cargo.toml:**
```toml
[dependencies]
oxios-kernel = { path = "../../crates/oxios-kernel" }
oxios-cli = { path = "../oxios-cli" }  # For CliChannel reuse in Chat panel
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["sync", "rt", "macros", "time"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1"
tracing = "0.1"
```

**Core rendering loop:**
```rust
// channels/oxios-tui/src/app.rs

pub struct App {
    kernel: Arc<Kernel>,
    state: Arc<RwLock<AppState>>,
    current_tab: Tab,
    should_quit: bool,
}

pub struct AppState {
    agents: Vec<AgentInfo>,
    containers: Vec<ContainerInfo>,
    events: Vec<KernelEvent>,
    chat_history: Vec<ChatMessage>,
    // Per-panel state
    log_filter: Option<String>,
    selected_agent: Option<usize>,
}

impl App {
    pub fn run(mut self, mut terminal: ratatui::Terminal<impl ratatui::backend::Backend>) -> Result<()> {
        let tick_rate = std::time::Duration::from_millis(250);
        let mut last_tick = std::time::Instant::now();

        loop {
            // Handle terminal events (keyboard, mouse)
            while crossterm::event::poll(std::time::Duration::from_millis(10))? {
                let event = crossterm::event::read()?;
                self.handle_event(event)?;
            }

            // Poll event bus for kernel updates
            self.poll_kernel_events()?;

            // Render
            terminal.draw(|f| self.render(f))?;

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }
}
```

**Integration:**
```rust
Some(Command::Tui { with_web }) => {
    let kernel = Kernel::builder()
        .config_path(config_path.to_path_buf())
        .build()
        .await?;

    if with_web {
        start_web_if_needed(&kernel).await;
    }

    let app = oxios_tui::App::new(kernel);
    let terminal = ratatui::init();
    let result = app.run(terminal);
    ratatui::restore();
    result?;
}
```

### File Changes

| File | Action |
|------|--------|
| `channels/oxios-cli/` | **New crate** — Complete CLI channel implementation |
| `channels/oxios-tui/` | **New crate** — Complete TUI implementation |
| `Cargo.toml` (workspace) | **Modify** — Add `oxios-cli` and `oxios-tui` to `[workspace.members]` |
| `src/main.rs` | **Modify** — Add `Chat` and `Tui` command variants |

### Test Strategy

- **CLI**: Unit tests for `MetaCommand` parsing, `Session` management. Integration test with mock gateway.
- **TUI**: Snapshot tests for rendering (using `ratatui::backend::TestBackend`). Event handling unit tests.

```rust
// CLI tests
#[test]
fn test_parse_command_quit() { assert_eq!(parse_command(".quit"), Some(MetaCommand::Quit)); }
#[test]
fn test_parse_command_model() { assert_eq!(parse_command(".model gpt-4"), Some(MetaCommand::Model("gpt-4".into()))); }
#[test]
fn test_parse_command_regular_input() { assert_eq!(parse_command("hello"), None); }

// TUI tests
#[test]
fn test_dashboard_renders() {
    let app = App::new_test();
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();
    // Verify no panic, reasonable output
}
```

### Dependencies

- `reedline` 0.38 (CLI)
- `ratatui` 0.29 + `crossterm` 0.28 (TUI)

---

## 6. Observability 7→10 (2 days)

### Problem

1. **No OpenTelemetry tracing**: All tracing is `tracing::info!` / `tracing::warn!` — no distributed trace IDs, no span propagation.
2. **No log rotation**: Logs grow unbounded.
3. **`/api/status` lacks detail**: Only shows service name, version, uptime. No component health checks.

### Solution

#### 6.1 OpenTelemetry Integration

```toml
# crates/oxios-kernel/Cargo.toml additions
[dependencies]
tracing-opentelemetry = "0.27"
opentelemetry = "0.27"
opentelemetry_sdk = { version = "0.27", features = ["rt-tokio"] }
opentelemetry-otlp = { version = "0.27", features = ["trace"] }
opentelemetry-semantic-conventions = "0.27"
```

```rust
// crates/oxios-kernel/src/telemetry.rs (new file)

use anyhow::Result;
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::runtime::Tokio;
use std::sync::Arc;

/// Telemetry configuration.
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Enable OpenTelemetry tracing.
    pub enabled: bool,
    /// OTLP endpoint (e.g., "http://localhost:4317").
    pub endpoint: Option<String>,
    /// Service name for traces.
    pub service_name: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            service_name: "oxios".into(),
        }
    }
}

/// Initialize OpenTelemetry tracing.
///
/// If `config.enabled` is false, returns a no-op guard.
/// If OTLP endpoint is configured, exports traces to the collector.
pub fn init_telemetry(config: &TelemetryConfig) -> Result<Option<opentelemetry::sdk::trace::Tracer>> {
    if !config.enabled {
        tracing::info!("OpenTelemetry tracing disabled");
        return Ok(None);
    }

    let mut builder = opentelemetry_sdk::trace::TracerProvider::builder()
        .with_simple_exporter(opentelemetry_stdout::SpanExporter::default());

    if let Some(endpoint) = &config.endpoint {
        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(endpoint)
            .build_span_exporter()?;

        builder = opentelemetry_sdk::trace::TracerProvider::builder()
            .with_simple_exporter(exporter);
    }

    let provider = builder.build();
    let tracer = provider.tracer(config.service_name.clone());

    // Bridge tracing crate → OpenTelemetry
    let opentelemetry_layer = tracing_opentelemetry::layer()
        .with_tracer(tracer.clone());

    // This should be applied to the global subscriber
    // (handled by the caller who sets up tracing_subscriber)

    tracing::info!(endpoint = ?config.endpoint, "OpenTelemetry tracing initialized");
    Ok(Some(tracer))
}
```

#### 6.2 Trace ID Propagation

Add trace spans to Ouroboros phases:

```rust
// crates/oxios-kernel/src/orchestrator.rs — add spans to handle_message

use tracing::instrument;

impl Orchestrator {
    #[instrument(
        name = "orchestrator.handle_message",
        skip(self, user_message),
        fields(session_id = %session_id.as_deref().unwrap_or("new"))
    )]
    pub async fn handle_message(
        &self,
        user_id: &str,
        user_message: &str,
        session_id: Option<&str>,
    ) -> Result<OrchestrationResult> {
        // ... existing implementation, but each phase gets a span ...

        // Phase 1: Interview
        let interview = {
            let _interview_span = tracing::info_span!("ouroboros.interview").entered();
            // ... interview logic ...
        };

        // Phase 2: Seed
        let seed = {
            let _seed_span = tracing::info_span!("ouroboros.seed").entered();
            self.ouroboros.generate_seed(&interview).await?
        };

        // Phase 3: Execute
        let exec_result = {
            let _exec_span = tracing::info_span!("ouroboros.execute").entered();
            self.lifecycle.spawn_and_run(&seed, Priority::Normal).await?
        };

        // Phase 4: Evaluate
        let evaluation = {
            let _eval_span = tracing::info_span!("ouroboros.evaluate").entered();
            self.ouroboros.evaluate(&seed, &exec_result).await?
        };

        // ... rest unchanged ...
    }
}
```

#### 6.3 Log Rotation

```toml
# Cargo.toml additions
[dependencies]
tracing-appender = "0.2"
```

```rust
// In main.rs or kernel initialization

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn init_tracing(config: &OxiosConfig) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("oxios=info"));

    // Log to file with rotation
    let file_appender = tracing_appender::rolling::daily(
        &config.kernel.data_dir,
        "oxios.log",
    );
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .with(tracing_subscriber::fmt::layer().with_writer(non_blocking).with_ansi(false));

    // Add OTel layer if configured
    // (conditional on TelemetryConfig)

    subscriber.init();
}
```

#### 6.4 Enhanced `/api/status`

```rust
// channels/oxios-web/src/routes/system.rs — enhance handle_status

#[derive(Debug, Serialize, Clone)]
pub(crate) struct StatusResponse {
    pub service: String,
    pub status: String,
    pub version: String,
    pub channels: Vec<String>,
    pub uptime: String,
    // New fields:
    pub components: ComponentHealth,
}

#[derive(Debug, Serialize, Clone)]
pub struct ComponentHealth {
    pub container_backend: ComponentStatus,
    pub state_store: ComponentStatus,
    pub event_bus: ComponentStatus,
    pub memory: MemoryHealth,
    pub agents: AgentHealth,
}

#[derive(Debug, Serialize, Clone)]
pub struct ComponentStatus {
    pub healthy: bool,
    pub detail: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct MemoryHealth {
    pub enabled: bool,
    pub index_size: usize,
    pub total_entries: usize,
}

#[derive(Debug, Serialize, Clone)]
pub struct AgentHealth {
    pub active_count: usize,
    pub total_forked: u64,
    pub total_completed: u64,
    pub total_failed: u64,
}

pub(crate) async fn handle_status(
    state: State<Arc<AppState>>,
) -> Json<StatusResponse> {
    let uptime = state.start_time.elapsed();
    let uptime_str = format!(
        "{}h {}m {}s",
        uptime.as_secs() / 3600,
        (uptime.as_secs() % 3600) / 60,
        uptime.as_secs() % 60
    );

    // Check component health
    let container_healthy = state.container_manager.is_backend_available();
    let metrics = get_metrics();

    Json(StatusResponse {
        service: "oxios".into(),
        status: "running".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        channels: vec!["web".into()],
        uptime: uptime_str,
        components: ComponentHealth {
            container_backend: ComponentStatus {
                healthy: container_healthy,
                detail: Some(state.container_manager.backend_name().to_string()),
            },
            state_store: ComponentStatus {
                healthy: true,
                detail: None,
            },
            event_bus: ComponentStatus {
                healthy: true,
                detail: None,
            },
            memory: MemoryHealth {
                enabled: true,
                index_size: 0,   // populated from MemoryManager
                total_entries: 0,
            },
            agents: AgentHealth {
                active_count: 0, // populated from Supervisor
                total_forked: 0,
                total_completed: 0,
                total_failed: 0,
            },
        },
    })
}
```

### File Changes

| File | Action |
|------|--------|
| `crates/oxios-kernel/src/telemetry.rs` | **New** — `TelemetryConfig`, `init_telemetry` |
| `crates/oxios-kernel/src/orchestrator.rs` | **Modify** — Add `#[instrument]` and phase spans |
| `crates/oxios-kernel/src/lib.rs` | **Modify** — Add `pub mod telemetry;` |
| `channels/oxios-web/src/routes/system.rs` | **Modify** — Enhanced `StatusResponse` with component health |
| `src/main.rs` (or kernel init) | **Modify** — Add `tracing-appender` log rotation, optional OTel layer |
| `crates/oxios-kernel/Cargo.toml` | **Modify** — Add `tracing-opentelemetry`, `opentelemetry*` deps |
| `Cargo.toml` (workspace) | **Modify** — Add `tracing-appender` |

### Test Strategy

```rust
#[test]
fn test_telemetry_disabled() {
    // Default config → no tracer
}

#[tokio::test]
async fn test_status_response_components() {
    // Verify StatusResponse serializes with all component fields
}

#[test]
fn test_status_response_serialization() {
    let response = StatusResponse { /* ... */ };
    let json = serde_json::to_value(&response).unwrap();
    assert!(json.get("components").is_some());
    assert!(json["components"]["container_backend"]["healthy"].is_boolean());
}
```

### Dependencies

- `tracing-opentelemetry` 0.27
- `opentelemetry` 0.27
- `opentelemetry_sdk` 0.27 (with `rt-tokio` feature)
- `opentelemetry-otlp` 0.27 (with `trace` feature)
- `tracing-appender` 0.2

---

## 7. Production 8→10 (3 days)

### Problem

1. **No OpenAPI spec**: The web API is undocumented. Consumers can't discover endpoints.
2. **No backup/restore**: No way to export/import system state.
3. **No E2E real-agent test**: Integration tests don't exercise the full Ouroboros pipeline with an LLM.

### Solution

#### 7.1 OpenAPI Spec with `utoipa`

```toml
# channels/oxios-web/Cargo.toml additions
[dependencies]
utoipa = { version = "5", features = ["axum_extras", "uuid", "chrono"] }
utoipa-swagger-ui = { version = "8", features = ["axum"] }
```

```rust
// channels/oxios-web/src/api_docs.rs (new file)

use utoipa::OpenApi;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// OpenAPI specification for Oxios Agent OS.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Oxios Agent OS",
        version = "0.1.0",
        description = "AI Agent Operating System API"
    ),
    paths(
        crate::routes::system::handle_health,
        crate::routes::system::handle_status,
        crate::routes::system::handle_agents_list,
        crate::routes::system::handle_agent_kill,
        crate::routes::system::handle_config_get,
        crate::routes::system::handle_config_put,
    ),
    components(
        schemas(
            HealthResponse,
            StatusResponse,
            AgentSummary,
            PageParams,
        )
    )
)]
pub struct ApiDoc;
```

Annotate existing handlers:

```rust
// channels/oxios-web/src/routes/system.rs — add utoipa annotations

/// Health check endpoint.
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse)
    )
)]
pub(crate) async fn handle_health(/* ... */) -> Json<serde_json::Value> { /* ... */ }

/// System status endpoint.
#[utoipa::path(
    get,
    path = "/api/status",
    responses(
        (status = 200, description = "System status", body = StatusResponse)
    )
)]
pub(crate) async fn handle_status(/* ... */) -> Json<StatusResponse> { /* ... */ }
```

Serve Swagger UI:

```rust
// channels/oxios-web/src/server.rs — add to router

use utoipa_swagger_ui::SwaggerUi;

let app = Router::new()
    // ... existing routes ...
    .merge(SwaggerUi::new("/docs").url("/docs/openapi.json", ApiDoc::openapi()));
```

#### 7.2 Backup/Restore

```rust
// crates/oxios-kernel/src/backup.rs (new file)

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

/// Backup manifest.
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupManifest {
    pub version: u32,
    pub created_at: String,
    pub oxios_version: String,
    pub sections: Vec<BackupSection>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupSection {
    pub name: String,
    pub entry_count: usize,
}

/// Create a backup of all Oxios state.
pub async fn create_backup(
    state_store: &StateStore,
    output_path: &Path,
) -> Result<BackupManifest> {
    let mut manifest = BackupManifest {
        version: 1,
        created_at: chrono::Utc::now().to_rfc3339(),
        oxios_version: env!("CARGO_PKG_VERSION").to_string(),
        sections: Vec::new(),
    };

    let mut tar_builder = tokio_tar::Builder::new(Vec::new());

    // Backup all categories
    let categories = [
        "seeds", "evals", "containers", "memory/conversations",
        "memory/sessions", "memory/facts", "memory/episodes",
        "memory/knowledge", "sessions", "agent_groups",
    ];

    for category in &categories {
        let names = state_store.list_category(category).await.unwrap_or_default();
        let mut count = 0;
        for name in &names {
            if let Ok(Some(data)) = state_store
                .load_raw(category, name)
                .await
            {
                let path = format!("{}/{}.json", category, name);
                let mut header = tokio_tar::Header::new_gnu();
                header.set_size(data.len() as u64);
                header.set_path(&path).ok();
                tar_builder.append(&header, &data[..]).await.ok();
                count += 1;
            }
        }
        if count > 0 {
            manifest.sections.push(BackupSection {
                name: category.to_string(),
                entry_count: count,
            });
        }
    }

    // Write manifest
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    let mut header = tokio_tar::Header::new_gnu();
    header.set_size(manifest_json.len() as u64);
    header.set_path("manifest.json").ok();
    tar_builder.append(&header, manifest_json.as_bytes()).await.ok();

    let archive_data = tar_builder.into_inner().await?;
    let mut file = tokio::fs::File::create(output_path).await?;
    file.write_all(&archive_data).await?;

    tracing::info!(
        path = %output_path.display(),
        sections = manifest.sections.len(),
        "Backup created"
    );

    Ok(manifest)
}

/// Restore state from a backup archive.
pub async fn restore_backup(
    state_store: &StateStore,
    backup_path: &Path,
) -> Result<BackupManifest> {
    let file_data = tokio::fs::read(backup_path).await?;
    let mut archive = tokio_tar::Archive::new(&file_data[..]);

    let mut entries = archive.entries()?;
    let mut manifest: Option<BackupManifest> = None;
    let mut restored = 0;

    while let Some(entry) = entries.next().await {
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().to_string();

        if path == "manifest.json" {
            let mut data = Vec::new();
            entry.read_to_end(&mut data).await?;
            manifest = Some(serde_json::from_slice(&data)?);
            continue;
        }

        // Parse category/name from path
        // e.g., "seeds/abc-123.json" → category="seeds", name="abc-123"
        if let Some((category, name_json)) = path.rsplit_once('/') {
            let name = name_json.trim_end_matches(".json");
            let mut data = Vec::new();
            entry.read_to_end(&mut data).await?;
            state_store.save_raw(category, name, &data).await?;
            restored += 1;
        }
    }

    let manifest = manifest.context("Backup missing manifest.json")?;
    tracing::info!(
        path = %backup_path.display(),
        restored,
        sections = manifest.sections.len(),
        "Backup restored"
    );

    Ok(manifest)
}
```

**CLI command:**

```rust
// src/main.rs — add backup/restore commands

Some(Command::Backup { output }) => {
    let kernel = Kernel::builder()
        .config_path(config_path.to_path_buf())
        .build()
        .await?;
    let output = output.unwrap_or_else(|| {
        format!("oxios-backup-{}.tar", chrono::Utc::now().format("%Y%m%d-%H%M%S"))
    });
    let manifest = oxios_kernel::backup::create_backup(
        &kernel.state_store,
        Path::new(&output),
    )
    .await?;
    println!("Backup created: {}", output);
    for section in &manifest.sections {
        println!("  {}: {} entries", section.name, section.entry_count);
    }
}

Some(Command::Restore { input }) => {
    let kernel = Kernel::builder()
        .config_path(config_path.to_path_buf())
        .build()
        .await?;
    let manifest = oxios_kernel::backup::restore_backup(
        &kernel.state_store,
        Path::new(&input),
    )
    .await?;
    println!("Backup restored from: {}", input);
    for section in &manifest.sections {
        println!("  {}: {} entries", section.name, section.entry_count);
    }
}
```

#### 7.3 E2E Integration Test with Mock LLM

```rust
// tests/e2e_full_pipeline.rs (new file)

use oxios_kernel::*;
use oxios_ouroboros::*;

/// Mock LLM provider that returns predetermined responses.
struct MockProvider {
    responses: parking_lot::Mutex<Vec<String>>,
}

impl MockProvider {
    fn new(responses: Vec<String>) -> Self {
        Self {
            responses: parking_lot::Mutex::new(responses),
        }
    }
}

#[async_trait::async_trait]
impl oxi_ai::Provider for MockProvider {
    // Implement the Provider trait to return canned responses
    // for interview, seed, evaluate, and evolve phases.
    // ...
}

#[tokio::test]
async fn test_full_ouroboros_pipeline() {
    // 1. Set up mock provider with known responses
    let mock_responses = vec![
        // Interview response
        r#"{"questions": [], "scores": {"goal_clarity": 0.9, "constraint_clarity": 0.8, "success_criteria": 0.9}}"#.into(),
        // Seed response
        r#"{"goal": "Write hello world in Rust", "constraints": ["Must compile"], "acceptance_criteria": ["Program prints 'Hello, World!'"], "ontology": []}"#.into(),
        // Evaluation response (pass)
        r#"{"mechanical_pass": true, "semantic_pass": true, "score": 1.0, "notes": ["All criteria met"]}"#.into(),
    ];

    let provider = Arc::new(MockProvider::new(mock_responses));
    let model = oxi_ai::Model { id: "mock".into(), ..Default::default() };
    let engine = Arc::new(OuroborosEngine::new(provider, model));

    // 2. Set up kernel with mock
    let temp_dir = tempfile::tempdir().unwrap();
    let state_store = Arc::new(StateStore::new(temp_dir.path().join("state")).unwrap());
    let event_bus = EventBus::new(256);

    // 3. Run full pipeline
    let result = engine.interview("Write a hello world program in Rust").await.unwrap();
    assert!(result.ready_for_seed);

    let seed = engine.generate_seed(&result).await.unwrap();
    assert_eq!(seed.goal, "Write hello world in Rust");
    assert!(!seed.acceptance_criteria.is_empty());

    let execution = ExecutionResult {
        output: "Hello, World!".into(),
        steps_completed: 1,
        success: true,
    };

    let evaluation = engine.evaluate(&seed, &execution).await.unwrap();
    assert!(evaluation.all_passed());
    assert!(evaluation.score >= 0.8);
}

#[tokio::test]
async fn test_full_ouroboros_with_evolution() {
    // Similar to above, but evaluation fails on first pass,
    // triggering evolution which then passes.
    // ...
}

#[tokio::test]
async fn test_multi_agent_pipeline() {
    // Create a seed with 3+ acceptance criteria
    // Verify it gets split into subtasks
    // Verify parallel execution (mock the lifecycle manager)
    // ...
}
```

### File Changes

| File | Action |
|------|--------|
| `channels/oxios-web/src/api_docs.rs` | **New** — `utoipa` OpenAPI spec |
| `channels/oxios-web/src/server.rs` | **Modify** — Add Swagger UI route |
| `channels/oxios-web/src/routes/system.rs` | **Modify** — Add `#[utoipa::path]` annotations |
| `channels/oxios-web/Cargo.toml` | **Modify** — Add `utoipa`, `utoipa-swagger-ui` |
| `crates/oxios-kernel/src/backup.rs` | **New** — `create_backup`, `restore_backup` |
| `crates/oxios-kernel/src/lib.rs` | **Modify** — Add `pub mod backup;` |
| `crates/oxios-kernel/Cargo.toml` | **Modify** — Add `tokio-tar` |
| `src/main.rs` | **Modify** — Add `Backup` and `Restore` command variants |
| `tests/e2e_full_pipeline.rs` | **New** — End-to-end integration tests with mock LLM |

### Test Strategy

- **OpenAPI**: Verify spec generates at build time (`utoipa::OpenApi::openapi()` doesn't panic).
- **Backup**: Unit tests for create → restore round-trip.
- **E2E**: Mock provider exercises all 5 Ouroboros phases in sequence.

```rust
#[tokio::test]
async fn test_backup_restore_roundtrip() {
    // Create state store with seeds, evals, memories
    // Backup → verify tar contents → restore to new state store → verify data
}
```

### Dependencies

- `utoipa` 5 (with `axum_extras`, `uuid`, `chrono` features)
- `utoipa-swagger-ui` 8 (with `axum` feature)
- `tokio-tar` 0.3 (for backup archives)

---

## 8. Implementation Schedule

### Phase 1 (Week 1): Production + Protocol + Observability

| Day | Area | Tasks |
|-----|------|-------|
| 1 | Protocol 8→10 | `parse.rs`, `eval_cache.rs`, enhanced `evaluate()`, `llm_json()` helper |
| 2 | Production 8→10 | `utoipa` annotations, Swagger UI, `backup.rs` |
| 3 | Production 8→10 | Backup CLI commands, E2E test with mock LLM |
| 4 | Observability 7→10 | `telemetry.rs`, OTel integration, log rotation |
| 5 | Observability 7→10 | Enhanced `/api/status`, trace spans in orchestrator |

### Phase 2 (Week 2): Multi-Agent + Memory + Tool

| Day | Area | Tasks |
|-----|------|-------|
| 6 | Multi-Agent 7→10 | `JoinSet` parallel `delegate_subtasks`, `Clone` for lifecycle |
| 7 | Multi-Agent 7→10 | A2A message delivery, `send_and_wait`, group persistence |
| 8 | Multi-Agent 7→10 | Agent group API routes, SSE endpoint, capability routing |
| 9 | Memory 8→10 | Vector index snapshot save/load, `remember_unique` dedup |
| 10 | Memory 8→10 | Importance decay, `curate()`, budget enforcement, background task |
| 11 | Tool 9→10 | Toolchain Containerfile templates, workspace init script |
| 12 | Tool 9→10 | `check_tool_health`, tool health API endpoint, tests |

### Phase 3 (Week 3–4): Channels

| Day | Area | Tasks |
|-----|------|-------|
| 13 | CLI Channel | `channels/oxios-cli/` scaffold, `CliChannel`, `Session`, `SessionStore` |
| 14 | CLI Channel | `InteractiveLoop`, `MetaCommand`, output formatting, integration |
| 15 | TUI | `channels/oxios-tui/` scaffold, `App`, tab navigation, basic layout |
| 16 | TUI | Dashboard panel, event subscription, status bar |
| 17 | TUI | Chat panel (CliChannel integration), Agents panel, Gardens panel |
| 18 | Polish | Logs panel, tests for all new code, documentation updates |

### Dependency Graph

```
Protocol (Day 1) → no blocking deps, enables better E2E tests
Production (Days 2-3) → no blocking deps
Observability (Days 4-5) → no blocking deps
Multi-Agent (Days 6-8) → no blocking deps
Memory (Days 9-10) → no blocking deps
Tool (Days 11-12) → Multi-Agent should be done (uses containers)
CLI (Days 13-14) → no blocking deps
TUI (Days 15-17) → CLI must be done (reuses CliChannel)
Polish (Day 18) → all above
```

### Total Effort: 18 days

### Risk Assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| OTel version conflicts with tokio | Medium | Use exact versions, test in CI |
| `tokio-tar` crate quality | Low | Fallback to `tar` + manual JSON walk |
| Mock Provider trait mismatch | Medium | Implement only required methods, skip optional |
| TUI rendering on exotic terminals | Low | Test with `TestBackend`, keep layout simple |
| Memory curation removing useful entries | Medium | Conservative thresholds (0.1), logging |
| JoinSet tasks panicking | Low | Catch `JoinError` in collector loop |
