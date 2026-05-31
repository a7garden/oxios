# Integration Test Design

**Date:** 2026-05-31  
**Principle:** Every test must protect against a real regression. No test-for-test's-sake.  
**Constraint:** No new test utility crates. No mock LLM frameworks. Use existing patterns.

---

## Existing Integration Test Infrastructure

The workspace already has strong patterns for integration testing:

| File | Pattern | Mock Strategy |
|------|---------|---------------|
| `crates/oxios-kernel/tests/e2e_test.rs` (474 lines) | `MockOuroboros` + `MockSupervisor` → `Orchestrator::with_config()` | Deterministic protocol + supervisor mocks |
| `crates/oxios-kernel/tests/integration_tests.rs` (1027 lines) | `MockOuroboros` + `MockSupervisor` + `Gateway` | Same mocks + channel routing |
| `tests/e2e_kernel.rs` (867 lines) | Direct subsystem construction | No mocks — real `StateStore`, `GitLayer`, `AuditTrail`, `BudgetManager` |

Key observation: `OxiosEngine::new("mock/model")` already creates a no-real-provider engine. The `MockOuroboros` and `MockSupervisor` patterns are proven (used in 18 passing tests). The `tempfile::tempdir()` pattern handles filesystem isolation.

**No new infrastructure is needed.** New tests follow the same patterns.

---

## Designed Tests

### Test 1: Kernel Subsystem Assembly

**What it validates:** That `Kernel::builder()` produces a kernel where all subsystems are correctly wired and can handle basic operations without panicking.

**File:** `tests/e2e_kernel.rs` (extend existing)

```rust
/// Verify all kernel subsystems initialize together and respond to basic operations.
///
/// Constructs real StateStore, AuditTrail, GitLayer, BudgetManager, ResourceMonitor.
/// Does NOT construct Orchestrator or AgentRuntime (those need engine provider).
///
/// Validates:
/// - StateStore save/load roundtrip
/// - AuditTrail append + verify chain integrity
/// - GitLayer init + commit + log
/// - BudgetManager check + deduct + remaining
/// - ResourceMonitor snapshot doesn't panic
///
/// Mock/stub needed: None — all subsystems are pure data/filesystem.
/// Network required: No.
/// Estimated effort: Low (2-3 hours).
#[tokio::test]
async fn test_kernel_subsystems_assembly() {
    // Uses existing setup() pattern from e2e_kernel.rs
    let dir = tempfile::tempdir().unwrap();

    let state_store = StateStore::new(dir.path().join("state")).unwrap();
    let audit = AuditTrail::new(dir.path().join("audit")).unwrap();
    let git = GitLayer::init(dir.path().join("repo")).unwrap();
    let budget = BudgetManager::new(BudgetLimit::Tokens(100_000));
    let monitor = ResourceMonitor::new();

    // Exercise each subsystem
    state_store.save_json("cat", "key", &json!({"v": 1})).await.unwrap();
    let loaded: Value = state_store.load_json("cat", "key").await.unwrap().unwrap();
    assert_eq!(loaded["v"], 1);

    audit.append(AuditAction::ToolCall { agent: "test".into(), tool: "read".into(), allowed: true }).await.unwrap();
    assert_eq!(audit.len(), 1);

    git.commit("initial", &[]).unwrap();
    assert_eq!(git.log(1).unwrap().len(), 1);

    assert!(budget.check(50_000).await.unwrap());
    budget.deduct(10_000, "test").await.unwrap();
    assert!(budget.remaining_tokens().await < 100_000);

    let snapshot = monitor.snapshot();
    assert!(snapshot.memory_bytes > 0); // at minimum the process itself
}
```

**Priority:** 🔴 Critical — this is the "do the pieces fit together" test.

---

### Test 2: EvalCache Roundtrip (No LLM)

**What it validates:** That `EvalCache` stores and retrieves evaluation results correctly, including cache key construction and TTL/expiry behavior.

**File:** `crates/oxios-ouroboros/tests/eval_cache_test.rs` (extend existing)

```rust
/// Verify EvalCache stores and returns cached evaluations.
///
/// The mechanical_pass evaluation path (acceptance criteria string match)
/// is deterministic and doesn't require LLM. This test was previously
/// trapped behind #[ignore] in e2e_real_pipeline.rs.
///
/// Validates:
/// - Cache miss returns None
/// - Cache hit returns stored EvaluationResult
/// - Cache key is based on seed ID + execution output hash
/// - Different executions of same seed don't collide
///
/// Mock/stub needed: None — EvalCache is a pure HashMap with TTL.
/// Network required: No.
/// Estimated effort: Low (1-2 hours).
#[test]
fn test_eval_cache_roundtrip() {
    let mut cache = EvalCache::new(Duration::from_secs(3600));

    let seed_id = Uuid::new_v4();
    let execution = ExecutionResult {
        output: "Hello, World!\n".into(),
        steps_completed: 1,
        success: true,
    };
    let result = EvaluationResult {
        mechanical_pass: true,
        semantic_pass: None,
        consensus_pass: None,
        score: 1.0,
        notes: vec![],
    };

    // Miss
    assert!(cache.get(&seed_id, &execution).is_none());

    // Store
    cache.insert(&seed_id, &execution, result.clone());

    // Hit
    let cached = cache.get(&seed_id, &execution).unwrap();
    assert_eq!(cached.score, 1.0);
    assert!(cached.mechanical_pass);

    // Different execution → miss
    let other_execution = ExecutionResult {
        output: "Different output".into(),
        steps_completed: 1,
        success: true,
    };
    assert!(cache.get(&seed_id, &other_execution).is_none());
}
```

**Priority:** 🟢 Low — nice to have, validates cache correctness independently.

---

### Test 3: AccessGate RBAC Enforcement

**What it validates:** That `AccessGate` correctly blocks/allows operations based on agent RBAC permissions, path sandboxing, and exec allowlist.

**File:** `crates/oxios-kernel/tests/integration_tests.rs` (extend existing)

```rust
/// Verify AccessGate enforces RBAC rules for tool, path, and command access.
///
/// Constructs AccessGate with:
/// - An AccessManager with a "restricted" agent (limited tools, sandboxed paths)
/// - An ExecConfig with a binary allowlist
/// - A recording AuditSink
///
/// Validates:
/// - Restricted agent cannot use "exec" tool
/// - Restricted agent cannot access paths outside its workspace
/// - Restricted agent cannot execute non-allowlisted binaries
/// - Audit trail records all denials
/// - "operator" agent has full access
///
/// Mock/stub needed: RecordingAuditSink (5-line struct implementing AuditSink).
/// Network required: No.
/// Estimated effort: Medium (3-4 hours — AccessGate API may need exploration).
#[tokio::test]
async fn test_access_gate_rbac_enforcement() {
    let mut access = AccessManager::new();

    // Restricted agent: only "read" tool, sandboxed to /workspace/restricted/
    access.set_permissions(AgentPermissions::for_new_agent("restricted"));
    // ... restrict tools and paths ...

    // Operator agent: full access
    access.set_permissions(AgentPermissions::operator("operator"));

    let exec_config = Arc::new(ExecConfig {
        allowed_binaries: vec!["/usr/bin/ls".into()],
        ..Default::default()
    });

    let audit = Arc::new(RecordingAuditSink::new());
    let gate = AccessGate::new(Arc::new(Mutex::new(access)), exec_config, audit.clone());

    // Restricted agent → tool denied
    let ctx = CheckContext::for_agent("restricted");
    let result = gate.check(CheckRequest::Tool { context: &ctx, tool_name: "exec" }).await;
    assert!(result.is_denied());

    // Operator → tool allowed
    let ctx = CheckContext::for_agent("operator");
    let result = gate.check(CheckRequest::Tool { context: &ctx, tool_name: "exec" }).await;
    assert!(result.is_allowed());

    // Audit trail captured both checks
    assert_eq!(audit.entries().len(), 2);
}
```

**Priority:** 🟡 High — security boundary validation. This is the #1 risk area for agents.

---

### Test 4: State Persistence Roundtrip

**What it validates:** That a full session (with messages, seed, evaluation) can be saved to disk and restored with all data intact.

**File:** `tests/e2e_kernel.rs` (extend existing)

```rust
/// Verify session state survives save → load cycle with no data loss.
///
/// Creates a Session with user messages, system messages, and metadata.
/// Saves to StateStore, creates a new StateStore instance pointing to
/// the same directory, loads the session, and verifies byte-for-byte equality.
///
/// Validates:
/// - Session ID preserved
/// - Message order preserved
/// - Message metadata (timestamps, roles) preserved
/// - Seed reference preserved
/// - Evaluation result preserved
///
/// Mock/stub needed: None.
/// Network required: No.
/// Estimated effort: Low (1-2 hours).
#[tokio::test]
async fn test_session_persistence_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = StateStore::new(dir.path().to_path_buf()).unwrap();

    // Build a realistic session
    let mut session = Session::new("user-456");
    session.add_user_message("Fix the login bug");
    session.add_system_message("Agent forked for task");
    session.set_seed_id(Uuid::new_v4());
    session.set_evaluation_passed(true);
    session.set_phase_reached("Evaluate");

    let original_id = session.id.clone();
    store.save_session(&session).await.unwrap();

    // New store instance → same directory
    let store2 = StateStore::new(dir.path().to_path_buf()).unwrap();
    let loaded = store2.load_session(&original_id).await.unwrap().unwrap();

    assert_eq!(loaded.id, original_id);
    assert_eq!(loaded.messages.len(), 2);
    assert_eq!(loaded.messages[0].role, "user");
    assert_eq!(loaded.messages[0].content, "Fix the login bug");
    assert_eq!(loaded.messages[1].role, "system");
    assert!(loaded.evaluation_passed);
    assert_eq!(loaded.phase_reached, "Evaluate");
}
```

**Priority:** 🟡 High — state persistence is critical for crash recovery.

---

### Test 5: Ouroboros Evaluate with Mechanical Pass (No LLM)

**What it validates:** That the mechanical_pass evaluation path (acceptance criteria string matching) works correctly without any LLM call, and that EvalCache correctly caches the result.

**File:** `crates/oxios-ouroboros/tests/eval_cache_test.rs` (extend existing)

```rust
/// Verify mechanical_pass evaluation + cache integration.
///
/// The mechanical_pass path checks if execution output satisfies all
/// acceptance criteria by simple string matching. No LLM needed.
/// This test was extracted from the ignored test_evaluate_with_cache
/// in tests/e2e_real_pipeline.rs.
///
/// Validates:
/// - When execution output satisfies all criteria → mechanical_pass=true, score=1.0
/// - When execution output fails criteria → mechanical_pass=false
/// - Cache hit on repeated evaluation returns same result
/// - Cache miss when execution output changes
///
/// Mock/stub needed: None — use OuroborosEngine::evaluate_mechanical() directly,
///   or construct the evaluation logic manually if mechanical_pass is a free function.
/// Network required: No.
/// Estimated effort: Medium (2-3 hours — depends on whether mechanical_pass is
///   accessible without an OuroborosEngine instance).
#[tokio::test]
async fn test_mechanical_eval_pass() {
    let seed = Seed {
        id: Uuid::new_v4(),
        goal: "Hello world".into(),
        acceptance_criteria: vec!["Output contains 'Hello, World!'".into()],
        ..Default::default()
    };

    let execution = ExecutionResult {
        output: "Hello, World!\n".into(),
        steps_completed: 1,
        success: true,
    };

    // Mechanical pass: output contains the criteria string
    let result = evaluate_mechanical(&seed, &execution);
    assert!(result.mechanical_pass);
    assert_eq!(result.score, 1.0);

    // Failing case
    let fail_execution = ExecutionResult {
        output: "Goodbye!".into(),
        steps_completed: 1,
        success: true,
    };
    let fail_result = evaluate_mechanical(&seed, &fail_execution);
    assert!(!fail_result.mechanical_pass);
}
```

**Priority:** 🟢 Medium — valuable for cache correctness, but not a critical path.

---

### Test 6: EventBus Phase Event Sequence

**What it validates:** That the Orchestrator publishes `PhaseStarted` and `PhaseCompleted` events in the correct order during execution.

**Note:** This test already exists as `test_phase_events_published` in `e2e_test.rs`. No new test needed — the existing test covers this adequately.

---

## Test Effort Summary

| Test | Validates | Mock Needed | Network | Effort | Priority |
|------|-----------|-------------|---------|--------|----------|
| 1. Kernel Subsystem Assembly | All subsystems init + basic ops | None | No | 2-3h | 🔴 Critical |
| 2. EvalCache Roundtrip | Cache store/retrieve/TTL | None | No | 1-2h | 🟢 Low |
| 3. AccessGate RBAC Enforcement | Security boundary | RecordingAuditSink | No | 3-4h | 🟡 High |
| 4. State Persistence Roundtrip | Crash recovery | None | No | 1-2h | 🟡 High |
| 5. Mechanical Eval Pass | Evaluation correctness | None | No | 2-3h | 🟢 Medium |

**Total estimated effort:** 9-14 hours  
**Recommended implementation order:** 1 → 4 → 3 → 5 → 2

---

## What NOT to Test

These are explicitly out of scope per the brief:

1. **Full `Kernel::builder().build()` → `execute_prompt_with_session()`** — Requires `OxiBuilder` with a real or mock provider. The current `OxiosEngine::new("mock/model")` will panic on provider resolution. Building a mock provider requires either `oxi_sdk` support or a new `EngineProvider` mock — both violate the "no elaborate test doubles" constraint. The Orchestrator-level tests with `MockOuroboros` adequately cover this path.

2. **AgentRuntime tool dispatch** — Requires `oxi_agent` internals which are in `oxi-sdk` (external crate). Testing this is `oxi-sdk`'s responsibility.

3. **WASM sandbox execution** — Requires compiling WASM modules. Not a regression risk for current features.

4. **Memory tier consolidation (Dream)** — Background process with time-based triggers. Testing it requires either time mocking or very long waits. The unit tests cover individual tier transitions.

---

## Pattern Reference

All tests should follow these established patterns:

```rust
// Filesystem isolation
let dir = tempfile::tempdir().unwrap();

// Subsystem construction
let store = StateStore::new(dir.path().to_path_buf()).unwrap();

// Mock protocol (for orchestrator tests)
struct MockOuroboros { /* atomic counters */ }
#[async_trait]
impl OuroborosProtocol for MockOuroboros { /* deterministic responses */ }

// Mock supervisor (for agent lifecycle tests)
struct MockSupervisor { /* HashMap<AgentId, AgentInfo> */ }
#[async_trait]
impl Supervisor for MockSupervisor { /* in-memory tracking */ }

// Event observation
let rx = event_bus.subscribe();
// ... run async test ...
tokio::select! { evt = rx.recv() => { /* match */ } }
```
