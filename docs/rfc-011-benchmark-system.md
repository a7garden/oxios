# RFC-011: Oxios Benchmark System v2

> **Status:** Draft
> **Author:** AI Agent
> **Date:** 2026-05-28
> **Replaces:** `benchmarks/oxios-bench/` (v1 prototype)

---

## 1. Motivation

### 1.1 Why v1 Fails

The current `oxios-bench` has **10 tasks** that test generic LLM chatbot capabilities (basic math, keyword matching). It measures **nothing** about what makes Oxios unique:

| Oxios Capability | v1 Coverage |
|---|---|
| Ouroboros protocol (Interview → Seed → Execute → Evaluate → Evolve) | ❌ 0 tasks |
| Agent lifecycle (fork/exec/wait/kill) | ❌ 0 tasks |
| Multi-agent collaboration (A2A) | ❌ 0 tasks |
| Skill system (SKILL.md) | ❌ 0 tasks |
| Memory tiers (Hot/Warm/Cold, Dream) | ❌ 0 tasks |
| Knowledge Base (oxios-markdown) | ❌ 0 tasks |
| RBAC / Access Control | ❌ 0 tasks |
| Tool execution (exec, browser, MCP) | ❌ 0 tasks |
| Circuit breaker / Error recovery | ❌ 0 tasks |
| Scheduler / Cron | ❌ 0 tasks |

Additional problems:
- **Evaluation is keyword matching** → high false positive/negative rate
- **EventCollector is dead code** → never actually connected to SSE
- **No regression comparison** → can't detect if changes make things worse
- **Hardcoded tasks** → can't extend without recompiling
- **Chat-only interface** → ignores `oxios run --json` structured output

### 1.2 Goals

1. **Measure what matters** — every Oxios subsystem gets at least one benchmark
2. **Deterministic evaluation** — structured data, not keyword guessing
3. **Regression detection** — compare runs, surface regressions automatically
4. **Extensible** — TOML-based task definitions, no recompilation needed
5. **Fast feedback** — unit-tier → integration-tier → e2e-tier, run only what you need
6. **CI-ready** — exit codes, machine-readable output, no manual interpretation

---

## 2. Architecture

### 2.1 Three-Tier Model

```
┌─────────────────────────────────────────────────────────────────┐
│  Tier 3: E2E Scenarios                                          │
│  Full user stories spanning multiple subsystems.                 │
│  Uses: oxios run --json (process spawn)                         │
│  Duration: 30-120s per task                                     │
│  Example: "Review this code, fix it, run tests, commit"         │
├─────────────────────────────────────────────────────────────────┤
│  Tier 2: Integration Tests                                      │
│  Tests one subsystem end-to-end through the kernel.             │
│  Uses: Kernel::execute_prompt_with_session() (in-process)       │
│  Duration: 5-30s per task                                       │
│  Example: "Create a space, write memory, recall it"             │
├─────────────────────────────────────────────────────────────────┤
│  Tier 1: Unit Benchmarks                                        │
│  Direct function calls, no LLM involved.                        │
│  Uses: crate APIs directly                                      │
│  Duration: <1s per task                                         │
│  Example: Seed::parse(), Scheduler::enqueue()                   │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Execution Harness

```
oxios-bench run                      → Tier 1 only (fast, CI gate)
oxios-bench run --tier integration   → Tier 1 + 2
oxios-bench run --tier e2e           → All tiers
oxios-bench run --suite ouroboros    → Specific suite only
oxios-bench run --tag @smoke         → Tasks tagged @smoke
oxios-bench compare <run-a> <run-b>  → Diff two runs
oxios-bench list                     → Show available tasks
```

### 2.3 Data Flow

```
TaskDefinition (TOML)
       │
       ▼
  ┌──────────┐     ┌──────────────┐     ┌─────────────┐
  │  Runner   │────▶│   Oxios      │────▶│  Evaluator  │
  │           │     │   Instance   │     │             │
  │ - spawn   │     │ - run --json │     │ - structural│
  │ - inject  │     │ - in-process │     │ - LLM-judge │
  │ - collect │     │              │     │ - exact     │
  └──────────┘     └──────────────┘     └─────────────┘
       │                                        │
       ▼                                        ▼
  ┌──────────────────────────────────────────────────┐
  │                   Reporter                        │
  │  - JSON report (machine-readable)                 │
  │  - Console summary (human-readable)               │
  │  - Regression diff                                │
  │  - HTML dashboard (optional)                      │
  └──────────────────────────────────────────────────┘
```

---

## 3. Task Definition Format

Tasks are defined in **TOML** files under `benchmarks/oxios-bench/suites/`:

```toml
# suites/ouroboros/simple-task.toml
[task]
id = "ouroboros_simple"
name = "Simple Ouroboros Cycle"
tier = "integration"          # unit | integration | e2e
suite = "ouroboros"           # logical grouping
tags = ["smoke", "core"]
timeout_secs = 60

[prompt]
message = "Create a file called hello.txt with the content 'Hello, Oxios!' in the workspace."

[expect]
# Structured assertions on oxios run --json output
phase_reached = "Execute"             # Must reach at least this phase
evaluation_passed = true              # Ouroboros evaluation must pass

# Response content assertions (all must be satisfied)
response_contains = ["hello.txt"]
response_not_contains = ["error", "failed"]

# Optional: check specific fields exist
require_agent_id = true               # An agent must have been created
require_seed_id = true                # A seed must have been created

[expect.performance]
max_duration_ms = 30_000              # Must complete within 30s
```

### 3.1 Multi-Turn Tasks

```toml
# suites/memory/recall-after-context-switch.toml
[task]
id = "memory_recall_cross_turn"
name = "Recall After Context Switch"
tier = "integration"
suite = "memory"
tags = ["memory", "multi-turn"]
timeout_secs = 90

[[turns]]
role = "user"
message = "Remember that my project uses Rust and Tokio."

[[turns]]
role = "user"
message = "What programming language does my project use?"
[turns.expect]
response_contains = ["Rust"]
phase_reached = "Execute"
```

### 3.2 Environment Setup (Fixtures)

```toml
# suites/knowledge/backlink-creation.toml
[task]
id = "knowledge_backlink"
name = "Backlink Creation"
tier = "integration"
suite = "knowledge"
timeout_secs = 30

[setup]
# Files to create before running the task
files = [
  { path = "rust.md", content = "# Rust\nA systems language. See [[tokio]] for async." },
]

[prompt]
message = "List all pages that link to the tokio page."

[expect]
response_contains = ["rust"]
```

### 3.3 Context File Tasks

```toml
# suites/coding/code-fix.toml
[task]
id = "coding_fix_bug"
name = "Fix a bug in code"
tier = "e2e"
suite = "coding"
timeout_secs = 120

[prompt]
message = "Fix the bug in this code. The function should return the sum, not the difference."
context_file = "fixtures/buggy_code.rs"   # Relative to suites/ directory

[expect]
phase_reached = "Execute"
evaluation_passed = true
response_contains = ["fn add", "+"]
response_not_contains = ["-", "subtract"]
```

---

## 4. Evaluation Strategies

### 4.1 Structural Evaluation (Primary)

Uses `oxios run --json` structured output — **no keyword guessing**:

```rust
struct StructuralAssertion {
    phase_reached: Option<Phase>,         // Must reach this phase
    evaluation_passed: Option<bool>,       // Ouroboros evaluation result
    require_seed_id: bool,                 // Seed must exist
    require_agent_id: bool,                // Agent must exist
    require_session_id: bool,              // Session must exist
    max_duration_ms: Option<u64>,          // Performance budget
}
```

Each assertion is checked independently. Score = (passed_assertions / total_assertions) × 100.

### 4.2 Content Evaluation (Secondary)

For cases where response content matters:

```rust
enum ContentCheck {
    Contains { text: String, case_sensitive: bool },
    NotContains { text: String },
    Regex { pattern: String },
    JsonField { path: String, equals: Value },
}
```

### 4.3 LLM-Judge Evaluation (Tiebreaker)

For subjective quality (code review quality, explanation clarity):

```rust
struct LlmJudgeAssertion {
    model: String,                    // e.g., "claude-sonnet-4"
    rubric: String,                   // What to evaluate
    min_score: f64,                   // 0.0 - 1.0 threshold
}
```

Only used when structural + content checks are insufficient. Runs a separate LLM call with the rubric.

### 4.4 Custom Evaluation Functions

For complex assertions (Rust code, not TOML):

```rust
// In benches/custom_evals.rs
fn evaluate_file_created(run_output: &RunOutput) -> EvalResult {
    let path = run_output.workspace.join("hello.txt");
    let exists = path.exists();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let correct = content.contains("Hello, Oxios!");
    EvalResult {
        passed: exists && correct,
        score: if correct { 100.0 } else if exists { 50.0 } else { 0.0 },
        notes: format!("exists={exists}, content_correct={correct}"),
    }
}
```

---

## 5. Task Suite Design

### 5.1 Suite Overview

| Suite | Count | Tier | What It Tests |
|-------|-------|------|---------------|
| `ouroboros` | 8 | integration/e2e | Full protocol cycle, phase transitions, evaluation, evolution |
| `agent` | 6 | integration | Agent lifecycle, A2A, multi-agent coordination |
| `tool` | 10 | integration | Individual tool correctness (exec, memory, space, knowledge, MCP) |
| `memory` | 6 | integration | Tier transitions, recall, Dream consolidation, protection |
| `knowledge` | 5 | integration | KB CRUD, backlinks, search, graph |
| `skill` | 4 | integration | Skill invocation, requirements matching |
| `security` | 5 | integration | RBAC, path sandboxing, audit trail |
| `performance` | 4 | e2e | Latency, throughput, concurrent agents |
| `regression` | 6 | e2e | Known past bugs stay fixed |
| **Total** | **~54** | | |

### 5.2 Ouroboros Suite (8 tasks)

| ID | Prompt | Key Assertion |
|----|--------|---------------|
| `ouroboros_simple` | "Create hello.txt with content 'Hello'" | `phase_reached >= Execute`, `evaluation_passed = true` |
| `ouroboros_interview` | Ambiguous prompt → clarify → answer in follow-up | Multi-turn: first reaches Interview, second reaches Execute |
| `ouroboros_seed_quality` | "Refactor main.rs to use clap" | `require_seed_id = true`, seed has `goal` + `criteria` |
| `ouroboros_evolve` | Task designed to fail first evaluation | `phase_reached >= Evolve`, eventual `evaluation_passed = true` |
| `ouroboros_eval_pass` | Well-scoped coding task | `evaluation_passed = true`, `phase_reached = Evaluate` |
| `ouroboros_eval_fail` | Impossible task | `evaluation_passed = false`, graceful failure |
| `ouroboros_session_chain` | 3-turn session building context | Session ID preserved across turns |
| `ouroboros_parallel` | Two independent tasks to same instance | Both complete, no cross-contamination |

### 5.3 Tool Suite (10 tasks)

| ID | Prompt | Key Assertion |
|----|--------|---------------|
| `tool_exec_shell` | "Run `echo hello` in the shell" | Response contains "hello" |
| `tool_exec_blocked` | "Run `rm -rf /`" | Execution blocked by RBAC |
| `tool_memory_write` | "Save 'fav_color=blue' to memory" | Memory stored confirmation |
| `tool_memory_recall` | Multi-turn: save then recall | Recalled value matches |
| `tool_space_create` | "Create a new space for my Rust project" | Space created with ID |
| `tool_space_list` | "List all my spaces" | Returns at least one space |
| `tool_knowledge_crud` | "Create a note called rust.md with content..." | File exists in knowledge dir |
| `tool_knowledge_search` | "Search my knowledge base for 'tokio'" | Returns relevant results |
| `tool_mcp_call` | "Use the filesystem MCP server to list files" | MCP tool invoked successfully |
| `tool_browser` | "Open example.com and tell me the title" | Response contains page info |

### 5.4 Agent Suite (6 tasks)

| ID | Prompt | Key Assertion |
|----|--------|---------------|
| `agent_fork` | "Spawn an agent to count files in workspace" | Agent created + completed |
| `agent_kill` | "Start a long task then cancel it" | Agent killed, resources freed |
| `agent_a2a_delegate` | "Delegate the math calculation to another agent" | A2A delegation occurred |
| `agent_a2a_query` | "Ask other agents if anyone knows about Rust" | A2A query + response |
| `agent_concurrent` | "Run 3 agents in parallel" | All 3 complete within timeout |
| `agent_budget` | "Run an expensive task with token budget of 1000" | Budget enforced, task stopped |

### 5.5 Memory Suite (6 tasks)

| ID | Prompt | Key Assertion |
|----|--------|---------------|
| `memory_write_read` | "Save X=42, then tell me what X is" | Recall returns 42 |
| `memory_cross_session` | Turn 1: save, Turn 2 (new session): recall | Persistence verified |
| `memory_search` | "Search memories about 'project config'" | Returns relevant memory |
| `memory_tier_hot` | Rapidly accessed memory stays hot | Custom eval checks tier |
| `memory_protection` | "Protect the memory about my API key" | Protection flag set |
| `memory_dream` | Write many memories, trigger Dream | Consolidation occurred |

### 5.6 Security Suite (5 tasks)

| ID | Prompt | Key Assertion |
|----|--------|---------------|
| `security_path_sandbox` | "Read /etc/shadow" | Access denied |
| `security_allowed_path` | "Read ~/.oxios/config.toml" | Access granted |
| `security_rbac_role` | Agent with read-only role tries to write | Permission denied |
| `security_audit_trail` | "Create a space" → check audit log | Audit entry exists, Merkle chain valid |
| `security_credential_isolation` | Agent A can't access Agent B's credentials | Isolation enforced |

### 5.7 Performance Suite (4 tasks)

| ID | Description | Key Assertion |
|----|-------------|---------------|
| `perf_simple_latency` | Simple prompt, measure latency | < 10s end-to-end |
| `perf_concurrent_5` | 5 concurrent prompts | All complete, no errors |
| `perf_tool_chain` | Prompt requiring 5+ tool calls | Completes within budget |
| `perf_memory_throughput` | Write 100 memories rapidly | < 30s total, no data loss |

### 5.8 Regression Suite (6 tasks)

Each regression task encodes a **previously fixed bug** to prevent reintroduction:

| ID | Bug Description |
|----|----------------|
| `regression_empty_seed` | Empty prompt should not crash |
| `regression_session_leak` | Sessions cleaned up after timeout |
| `regression_circuit_breaker` | Provider failure triggers circuit breaker |
| `regression_concurrent_deadlock` | Concurrent agent creation doesn't deadlock |
| `regression_memory_overflow` | Large memory doesn't cause OOM |
| `regression_unicode_prompt` | Korean/CJK prompts handled correctly |

---

## 6. New Crate Structure

```
benchmarks/oxios-bench/
├── Cargo.toml
├── src/
│   ├── main.rs                    # CLI entry point
│   ├── lib.rs                     # Public types
│   │
│   ├── task.rs                    # TaskDefinition (from TOML or Rust)
│   ├── suite.rs                   # Suite loading from filesystem
│   │
│   ├── runner/
│   │   ├── mod.rs                 # Runner trait + orchestrator
│   │   ├── process_runner.rs      # Tier 3: spawns `oxios run --json`
│   │   ├── kernel_runner.rs       # Tier 2: in-process Kernel call
│   │   └── unit_runner.rs         # Tier 1: direct API call
│   │
│   ├── eval/
│   │   ├── mod.rs                 # Evaluator trait + composite
│   │   ├── structural.rs          # Phase/field assertions
│   │   ├── content.rs             # Text/regex assertions
│   │   ├── llm_judge.rs           # LLM-as-judge
│   │   └── custom.rs              # Rust function evaluators
│   │
│   ├── report/
│   │   ├── mod.rs                 # Report generation
│   │   ├── json.rs                # JSON serialization
│   │   ├── console.rs             # Terminal output (colored)
│   │   ├── compare.rs             # Regression diff between runs
│   │   └── html.rs                # Optional HTML dashboard
│   │
│   ├── fixture.rs                 # Workspace setup/teardown for tasks
│   └── config.rs                  # BenchmarkConfig
│
├── suites/                        # TOML task definitions
│   ├── ouroboros/
│   │   ├── simple.toml
│   │   ├── interview.toml
│   │   ├── evolve.toml
│   │   └── ...
│   ├── agent/
│   ├── tool/
│   ├── memory/
│   ├── knowledge/
│   ├── skill/
│   ├── security/
│   ├── performance/
│   └── regression/
│
├── fixtures/                      # Static test data
│   ├── buggy_code.rs
│   ├── sample_project/
│   └── ...
│
└── reports/                       # Generated reports (gitignored)
    └── .gitkeep
```

### 6.1 Core Types

```rust
/// A loaded, ready-to-run benchmark task.
pub struct Task {
    pub id: String,
    pub name: String,
    pub tier: Tier,
    pub suite: String,
    pub tags: Vec<String>,
    pub timeout: Duration,
    pub turns: Vec<Turn>,               // Single or multi-turn
    pub fixtures: Vec<Fixture>,          // Files to create before running
    pub context_file: Option<PathBuf>,   // --context-file
    pub assertions: Vec<Assertion>,      // Structured + content + custom
    pub custom_eval: Option<fn(&RunOutput) -> EvalResult>,
}

pub enum Tier {
    Unit,        // Direct API, no LLM
    Integration, // In-process kernel
    E2E,         // Process spawn
}

pub struct Turn {
    pub message: String,
    pub assertions: Vec<Assertion>,      // Per-turn assertions
}

pub enum Assertion {
    // Structural (from oxios run --json)
    PhaseReached { min: Phase },
    EvaluationPassed { expected: bool },
    RequireSeedId,
    RequireAgentId,
    MaxDuration { ms: u64 },

    // Content
    Contains { text: String, case_sensitive: bool },
    NotContains { text: String },
    Regex { pattern: String },

    // LLM-Judge
    LlmJudge { rubric: String, min_score: f64 },

    // Custom
    Custom { name: String, eval_fn: fn(&RunOutput) -> EvalResult },
}

pub struct RunOutput {
    /// Parsed JSON from `oxios run --json`
    pub response: String,
    pub session_id: Option<String>,
    pub space_id: Option<String>,
    pub seed_id: Option<String>,
    pub agent_id: Option<String>,
    pub phase_reached: String,
    pub evaluation_passed: bool,
    pub duration_ms: u64,
    pub exit_code: i32,
    /// Workspace root for file-system assertions
    pub workspace: PathBuf,
}

pub struct TaskResult {
    pub task_id: String,
    pub passed: bool,
    pub score: f64,                       // 0.0 - 100.0
    pub assertion_results: Vec<AssertionResult>,
    pub duration_ms: u64,
    pub error: Option<String>,
}

pub struct AssertionResult {
    pub assertion: String,                // Human-readable description
    pub passed: bool,
    pub actual: String,                   // What was observed
    pub expected: String,                 // What was expected
}

pub struct BenchmarkRun {
    pub id: String,                       // UUID
    pub timestamp: DateTime<Utc>,
    pub oxios_version: String,            // From `oxios --version`
    pub git_ref: Option<String>,          // Current git HEAD
    pub results: Vec<TaskResult>,
    pub summary: RunSummary,
}

pub struct RunSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub score_avg: f64,
    pub duration_total_ms: u64,
    pub regressions: Vec<Regression>,     // Compared to baseline
}

pub struct Regression {
    pub task_id: String,
    pub previous_score: f64,
    pub current_score: f64,
    pub delta: f64,                       // Negative = regression
}
```

---

## 7. Runner Architecture

### 7.1 Trait Design

```rust
#[async_trait]
pub trait Runner: Send + Sync {
    /// Run a single task and return the output.
    async fn run_task(&self, task: &Task) -> Result<RunOutput>;

    /// Friendly name for this runner.
    fn name(&self) -> &str;
}
```

### 7.2 ProcessRunner (Tier 3: E2E)

```rust
/// Spawns `oxios run --json` as a subprocess.
/// Most realistic. Tests the actual binary.
pub struct ProcessRunner {
    oxios_bin: PathBuf,         // Path to `oxios` binary
    workspace: PathBuf,         // Isolated workspace per task
}

impl ProcessRunner {
    async fn run_single(&self, prompt: &str, session_id: Option<&str>) -> Result<RunOutput> {
        let mut cmd = Command::new(&self.oxios_bin);
        cmd.args(["run", "--json", "--exit-code"]);
        if let Some(sid) = session_id {
            cmd.args(["--session", sid]);
        }
        cmd.arg(prompt);

        let output = cmd.output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: Value = serde_json::from_str(&stdout)?;

        Ok(RunOutput {
            response: json["response"].as_str().unwrap_or("").to_string(),
            session_id: json["session_id"].as_str().map(|s| s.to_string()),
            phase_reached: json["phase_reached"].as_str().unwrap_or("unknown").to_string(),
            evaluation_passed: json["evaluation_passed"].as_bool().unwrap_or(false),
            exit_code: output.status.code().unwrap_or(-1),
            duration_ms: json["duration_ms"].as_u64().unwrap_or(0),
            // ... etc
            workspace: self.workspace.clone(),
            // ...
        })
    }
}
```

### 7.3 KernelRunner (Tier 2: Integration)

```rust
/// Calls Kernel::execute_prompt_with_session() directly.
/// Faster than process spawn, shares the same kernel instance.
pub struct KernelRunner {
    kernel: Arc<Kernel>,
}

#[async_trait]
impl Runner for KernelRunner {
    async fn run_task(&self, task: &Task) -> Result<RunOutput> {
        let result = self.kernel
            .execute_prompt_with_session(&task.turns[0].message, None)
            .await?;

        Ok(RunOutput {
            response: result.response,
            session_id: result.session_id,
            phase_reached: result.phase_reached.to_string(),
            evaluation_passed: result.evaluation_passed,
            // ...
        })
    }
}
```

### 7.4 UnitRunner (Tier 1: Unit)

```rust
/// Direct function calls. No LLM. Sub-millisecond.
/// Tests deterministic logic: parsing, scheduling, RBAC rules, etc.
pub struct UnitRunner;

impl UnitRunner {
    pub fn run_seed_parsing() -> Vec<TaskResult> { /* ... */ }
    pub fn run_scheduler_logic() -> Vec<TaskResult> { /* ... */ }
    pub fn run_rbac_rules() -> Vec<TaskResult> { /* ... */ }
    pub fn run_memory_decay() -> Vec<TaskResult> { /* ... */ }
}
```

---

## 8. Evaluator Architecture

### 8.1 Composite Evaluator

```rust
pub struct CompositeEvaluator {
    evaluators: Vec<Box<dyn Evaluator>>,
}

#[async_trait]
pub trait Evaluator: Send + Sync {
    fn evaluate(&self, output: &RunOutput, assertions: &[Assertion]) -> Vec<AssertionResult>;
}

/// Evaluates structural assertions against oxios run --json output.
pub struct StructuralEvaluator;

/// Evaluates text content assertions against the response field.
pub struct ContentEvaluator;

/// Uses a separate LLM call to judge response quality.
pub struct LlmJudgeEvaluator {
    client: reqwest::Client,
    model: String,
}
```

### 8.2 Scoring

```
Task Score = Σ(assertion_weight_i × assertion_passed_i) / Σ(assertion_weight_i) × 100

Default weights:
  Structural assertions: 2.0 (most important)
  Content assertions:     1.0
  LLM-Judge assertions:   0.5 (supplementary)
  Custom assertions:      1.5

Task passed = (score >= 80.0) AND (all structural assertions pass)
```

---

## 9. Reporting

### 9.1 JSON Report

```json
{
  "id": "uuid",
  "timestamp": "2026-05-28T12:00:00Z",
  "oxios_version": "0.4.0",
  "git_ref": "abc1234",
  "summary": {
    "total": 54,
    "passed": 48,
    "failed": 4,
    "skipped": 2,
    "score_avg": 89.3,
    "duration_total_ms": 184000,
    "regressions": [
      {
        "task_id": "tool_memory_recall",
        "previous_score": 100.0,
        "current_score": 0.0,
        "delta": -100.0
      }
    ]
  },
  "results": [
    {
      "task_id": "ouroboros_simple",
      "passed": true,
      "score": 100.0,
      "assertion_results": [
        {
          "assertion": "phase_reached >= Execute",
          "passed": true,
          "expected": "Execute",
          "actual": "Execute"
        },
        {
          "assertion": "evaluation_passed = true",
          "passed": true,
          "expected": "true",
          "actual": "true"
        },
        {
          "assertion": "duration < 30000ms",
          "passed": true,
          "expected": "< 30000",
          "actual": "8432"
        }
      ],
      "duration_ms": 8432
    }
  ]
}
```

### 9.2 Console Output

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  OXIOS BENCHMARK — 54 tasks · tier: all · oxios v0.4.0
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  SUITE         TOTAL  PASS  FAIL  SKIP  AVG SCORE  TIME
  ────────────  ─────  ────  ────  ────  ─────────  ──────
  ouroboros        8     7     1     0     87.5%     42s
  agent            6     6     0     0    100.0%     18s
  tool            10     8     2     0     80.0%     31s
  memory           6     6     0     0    100.0%     12s
  knowledge        5     5     0     0    100.0%      8s
  skill            4     4     0     0    100.0%      6s
  security         5     4     1     0     80.0%     15s
  performance      4     4     0     0    100.0%     22s
  regression       6     4     0     2     66.7%     30s

  ────────────  ─────  ────  ────  ────  ─────────  ──────
  TOTAL           54    48     4     2     89.3%    184s

  ⚠ REGRESSIONS:
    tool_memory_recall    100.0 → 0.0  (Δ -100.0)  ← CRITICAL
    ouroboros_evolve      100.0 → 50.0 (Δ -50.0)

  Report: .oxios-bench/reports/<uuid>.json
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### 9.3 Regression Comparison

```
$ oxios-bench compare baseline.json latest.json

  COMPARISON: baseline (abc1234) → latest (def5678)

  IMPROVED:
    ouroboros_evolve     50.0 → 100.0  (Δ +50.0)  ✅

  REGRESSED:
    tool_memory_recall  100.0 → 0.0   (Δ -100.0)  🔴 CRITICAL
    security_rbac_role   80.0 → 60.0  (Δ -20.0)   🟡 WARNING

  UNCHANGED: 51 tasks
```

---

## 10. CLI Interface

```
$ oxios-bench --help

Oxios Benchmark System v2

Usage: oxios-bench <command> [options]

Commands:
  run        Execute benchmark tasks
  list       List available tasks and suites
  compare    Compare two benchmark runs
  baseline   Set a run as the regression baseline
  show       Show detailed results for a specific run

Run options:
  --tier <tier>           unit | integration | e2e | all (default: unit)
  --suite <name>          Run only a specific suite
  --tag <tag>             Run tasks matching a tag (@smoke, @core)
  --task <id>             Run a single task
  --parallel <n>          Max concurrent tasks (default: 1)
  --url <url>             Oxios API URL (default: http://127.0.0.1:4200)
  --bin <path>            Path to oxios binary (default: auto-detect)
  --timeout <secs>        Global timeout per task (default: 120)
  --no-regression         Skip regression comparison
  --json                  Output results as JSON
  --verbose               Show per-assertion details

Examples:
  oxios-bench run                           # Fast unit tests only
  oxios-bench run --tier integration        # Unit + integration
  oxios-bench run --tier e2e --parallel 3   # Full suite, 3 concurrent
  oxios-bench run --suite ouroboros         # One suite only
  oxios-bench run --tag @smoke              # Quick smoke test
  oxios-bench compare baseline.json run.json
```

---

## 11. CI Integration

### 11.1 GitHub Actions

```yaml
# .github/workflows/benchmark.yml
name: Benchmark
on: [push, pull_request]

jobs:
  unit-bench:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo build -p oxios-bench
      - run: cargo run -p oxios-bench -- run --tier unit --json > bench-unit.json
      - uses: actions/upload-artifact@v4
        with:
          name: bench-unit
          path: bench-unit.json

  integration-bench:
    runs-on: ubuntu-latest
    needs: unit-bench
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v4
      - run: cargo build
      - run: |
          cargo run --bin oxios -- --foreground &
          sleep 5
          cargo run -p oxios-bench -- run --tier integration --json > bench-integration.json
      - run: cargo run -p oxios-bench -- compare .oxios-bench/baseline.json bench-integration.json
```

### 11.2 Exit Codes

| Code | Meaning |
|------|---------|
| 0 | All tasks passed, no regressions |
| 1 | One or more tasks failed |
| 2 | Regressions detected (tasks that previously passed now fail) |
| 3 | Benchmark infrastructure error (oxios not running, etc.) |

---

## 12. Migration Plan

### Phase 1: Foundation (Week 1)
- [ ] Restructure crate per Section 6
- [ ] Implement `Task`, `RunOutput`, `AssertionResult` types
- [ ] Implement TOML parser (`task.rs`, `suite.rs`)
- [ ] Implement `ProcessRunner` using `oxios run --json`
- [ ] Implement `StructuralEvaluator`
- [ ] Implement `ContentEvaluator`
- [ ] Write 5 ouroboros suite tasks as TOML
- [ ] Write console reporter
- [ ] Delete old v1 code

### Phase 2: Expansion (Week 2)
- [ ] Implement `KernelRunner` for integration tier
- [ ] Implement `UnitRunner` for unit tier
- [ ] Write agent, tool, memory suites
- [ ] Implement fixture system (setup/teardown)
- [ ] Implement multi-turn support
- [ ] Implement `oxios-bench compare`

### Phase 3: Maturity (Week 3)
- [ ] Write knowledge, skill, security suites
- [ ] Implement `LlmJudgeEvaluator`
- [ ] Write performance suite with concurrent runners
- [ ] Implement parallel task execution
- [ ] Set up CI integration
- [ ] Write regression suite from known bugs
- [ ] Optional: HTML reporter

---

## 13. What Gets Deleted

The following v1 code is **fully replaced** and will be removed:

| File | Reason |
|------|--------|
| `src/tasks.rs` | Replaced by TOML task definitions |
| `src/evaluator.rs` | Replaced by `eval/` module (structural + content + LLM-judge) |
| `src/collector.rs` | Replaced by `oxios run --json` structured output (no SSE needed) |
| `src/analyzer.rs` | Dead code in v1; replaced by reporter in v2 |
| `src/report.rs` | Replaced by `report/` module |
| `src/cli.rs` | Replaced by new CLI with tier/suite/tag support |

---

## 14. Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **TOML tasks, not Rust** | Non-developers can add tasks. No recompilation. CI can hot-swap tasks. |
| **`oxios run --json` as primary interface** | Structured output (phase, evaluation, seed, agent, duration) eliminates keyword guessing. Tests the real binary. |
| **Three tiers** | CI gets fast feedback (unit), full validation is opt-in (e2e). |
| **Structural assertions first** | `phase_reached` and `evaluation_passed` are objective. Content matching is secondary. |
| **No SSE event collection** | v1's EventCollector was never connected. `oxios run --json` already provides everything we need. |
| **Regressions as first-class concept** | `compare` command makes degrading performance visible immediately. |
| **One workspace per task** | Isolation prevents cross-task contamination (files, memory, spaces). |
| **Exit code 2 for regressions** | Distinguishes "new failure" from "we made something worse". |
