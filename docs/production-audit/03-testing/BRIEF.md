# Brief 03: Testing — E2E Coverage & Test Health

**Area:** Test coverage, E2E pipeline, test quality  
**Severity:** 🟡 High  
**Estimated scope:** 0 E2E pipeline tests, 17 ignored tests, 0% LLM integration coverage  

---

## Context

**Current state:**
- **1,233 tests** — all passing ✅
- **671** in oxios-kernel (unit)
- **200** in oxios-markdown (unit)
- **42** in oxios-ouroboros (unit)
- **70** in integration tests
- **17 ignored** tests (including `#[ignore]` for LLM-requiring tests)

**Critical gaps:**

1. **`tests/e2e_real_pipeline.rs`** — Contains 2 tests, both `#[ignore]`.
   These require `OXIOS_E2E=1` env var and a real API key. They have
   likely **never been run** in CI.

2. **`crates/oxios-ouroboros/tests/scenario_test.rs`** — Also `#[ignore]`,
   requires real LLM. Never run in CI.

3. **No integration test exists for the full kernel execution path:**
   `Kernel::execute_prompt_with_session()` → Orchestrator →
   AgentRuntime → Tool calls → Response. This is the primary user-facing
   code path and it has **zero automated coverage**.

4. **11 doc-tests ignored** in oxios-kernel — these represent public API
   documentation that cannot compile or run.

The existing tests are excellent for unit coverage, but the system has
no automated verification that the pieces work together correctly.

---

## Objective

1. **Audit and classify** all ignored/skipped tests
2. **Create a test infrastructure** for integration tests that don't
   require real LLM calls
3. **Design (not implement)** E2E pipeline tests
4. **Fix or remove** broken doc-tests

This does NOT mean:
- ❌ Adding a mock LLM framework or creating elaborate test doubles
- ❌ Achieving arbitrary coverage percentages
- ❌ Writing tests for tests' sake — every test must protect against a
  real regression
- ❌ Restructuring the test directory layout

It DOES mean:
- ✅ Using the existing `oxi_sdk::Oxi` builder with a mock/no-op provider
  if available, or designing a minimal test harness
- ✅ Writing tests that exercise the **kernel subsystems together**:
  StateStore + AuditTrail + BudgetManager + GitLayer + Scheduler
- ✅ Documenting which doc-tests are broken and why
- ✅ Creating a CI plan for running integration tests

---

## Approach

### Phase 1: Test Audit (read-only)

1. Find all `#[ignore]` tests across the workspace
2. For each, document:
   - File and test name
   - Why it's ignored (LLM? external service? flaky?)
   - Whether it can be un-ignored with mocking
   - Whether it should be removed
3. Find all doc-test `ignore` annotations
4. Write results to `docs/production-audit/03-testing/AUDIT-TESTS.md`

### Phase 2: Integration Test Design

Design tests for these critical paths (write test signatures + comments,
not full implementations):

1. **Kernel assemble + execute** — `Kernel::builder().build()` creates
   all subsystems, then execute a no-op prompt
2. **Agent lifecycle** — fork → register → schedule → run → cleanup
   (with mock engine)
3. **Ouroboros evaluate** — Seed → evaluate → evolve cycle with cached
   evaluation (no LLM needed for the caching logic)
4. **State persistence** — Save session → reload → verify roundtrip
5. **Access gate chain** — Verify AccessGate correctly blocks/rejects
   based on RBAC rules

For each test, specify:
- What it validates
- What mock/stub is needed (if any)
- Whether it can run without network
- Estimated effort

Write the design to `docs/production-audit/03-testing/INTEGRATION-TEST-DESIGN.md`

### Phase 3: Doc-test Fixes

1. Read each ignored doc-test
2. Determine why it's ignored:
   - Missing imports → fix the doc example
   - Requires external state → add setup code or mark as `no_run`
   - Broken API → fix the doc to match current API
3. Fix what can be fixed in-place
4. For doc-tests that genuinely need external resources, change to
   `ignore` → `no_run` (compile-check but don't execute)
5. Run `cargo test --doc -p oxios-kernel` to verify

### Phase 4: CI Integration Plan

Write `docs/production-audit/03-testing/CI-TEST-PLAN.md`:
- Which tests run on every PR (existing unit tests)
- Which tests run nightly/weekly (integration tests)
- How to handle the `OXIOS_E2E` tests (separate workflow with secrets)
- Test partitioning strategy (current 4-way split is good)

---

## Constraints

- **Do not** create new test utility crates or shared test fixtures
  across crate boundaries (use per-crate `tests/` as currently done)
- **Do not** add dev-dependencies that are large or controversial
- **Do not** modify existing passing tests
- **Do not** attempt to reach a coverage number — quality over quantity
- **Do not** write E2E tests that call real LLM APIs (those stay ignored
  and manual-only)

## Verification

1. `cargo test --workspace` — all existing tests still pass
2. `cargo test --doc -p oxios-kernel` — doc-tests improved
3. New integration test files compile: `cargo test --no-run -p oxios-kernel`
