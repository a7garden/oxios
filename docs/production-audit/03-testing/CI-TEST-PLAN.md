# CI Test Plan

**Date:** 2026-05-31  
**Based on:** Phase 4 of Brief 03 — Testing & E2E Coverage

---

## 1. Current CI State

The existing CI (`.github/workflows/ci.yml`) has 4 jobs:

| Job | What it runs | Frequency |
|-----|--------------|-----------|
| `fmt` | `cargo fmt --check`, `cargo clippy --workspace` | Every PR |
| `frontend` | `bun typecheck`, `bun build` | Every PR |
| `test` (×4 partitions) | `cargo nextest run --workspace --partition …/4` | Every PR |
| `coverage` | `cargo test --workspace` + grcov → Codecov | Every PR |
| `audit` | `cargo audit` | Every PR |
| `release-check` | `cargo build --release --no-default-features` | Every PR |

**Total test execution:** ~1,233 unit tests across 28 test binaries, partitioned into 4 CI runners.

---

## 2. What the Current CI Does NOT Cover

The current CI covers **unit tests only**. These gaps exist:

| Gap | Risk | Mitigation |
|-----|------|------------|
| **Doc-tests** | `cargo test --workspace --doc` is NOT run in CI | **Add doc-test job** to CI |
| **E2E with real LLM** (`tests/e2e_real_pipeline.rs`) | Zero automated LLM integration coverage | Separate manual workflow (see §5) |
| **Integration tests** (kernel subsystems together) | Orchestrator + supervisor wired, but not full Kernel assembly | New test files (see INTEGRATION-TEST-DESIGN.md) |
| **Embedding model tests** (2 GGUF tests, 329MB model) | Real embedding validation | Separate scheduled job or manual |

---

## 3. Recommended CI Changes

### 3.1 Add Doc-Test Job (Immediately)

Add to `.github/workflows/ci.yml`:

```yaml
  doc-test:
    name: doc tests
    runs-on: ubuntu-latest
    needs: [fmt]
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - uses: actions/checkout@v4
        with:
          repository: a7garden/oxi
          path: oxi
          ref: v0.4.4

      - name: Run doc tests
        run: cargo test --workspace --doc

      - name: Verify all doc-examples compile
        run: |
          # Ensure zero `ignore` doc-tests (except intentionally-marked ones)
          # Doc-tests with `ignore` markers are intentional for:
          #   - Examples requiring real credentials (engine.rs, state_store.rs)
          #   - Examples requiring pub(crate) internals (capability/resolve.rs)
          echo "Doc-tests: run 'cargo test --doc -p oxios-kernel' locally to verify"
```

**Impact:** Catches doc-example drift before merge. Zero runtime cost (compile-only).

### 3.2 Keep Existing Test Partitioning

The 4-way partition is appropriate. Increasing to 5+ partitions adds overhead without proportional benefit for the current test count. Re-evaluate when test count exceeds 2,000.

### 3.3 Nightly Integration Test Job

Add a scheduled workflow for integration tests that require longer execution time:

```yaml
# .github/workflows/integration-tests.yml
name: integration tests

on:
  schedule:
    # Run weekly — Sundays at 02:00 UTC
    - cron: '0 2 * * 0'
  workflow_dispatch:  # Manual trigger via GitHub Actions UI

jobs:
  kernel-integration:
    name: kernel subsystem integration
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          repository: a7garden/oxi
          path: oxi
          ref: v0.4.4

      - uses: dtolnay/rust-toolchain@stable

      - uses: actions/checkout@v4

      - name: Run kernel integration tests
        run: cargo test -p oxios-kernel --test integration_tests --test e2e_test --test e2e_kernel

      - name: Run ouroboros integration tests
        run: cargo test -p oxios-ouroboros --test eval_cache_test

  gateway-integration:
    name: gateway integration
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          repository: a7garden/oxi
          path: oxi
          ref: v0.4.4

      - uses: dtolnay/rust-toolchain@stable

      - uses: actions/checkout@v4

      - name: Run gateway tests
        run: cargo test -p oxios-gateway --test gateway_test
```

---

## 4. E2E with Real LLM — Separate Manual Workflow

`tests/e2e_real_pipeline.rs` and `crates/oxios-ouroboros/tests/scenario_test.rs` require real API keys and must NOT run in public CI.

Create a **separate workflow with secrets**:

```yaml
# .github/workflows/e2e-llm.yml
name: E2E LLM integration (manual + scheduled)

on:
  schedule:
    # Run bi-weekly — every other Saturday at 03:00 UTC
    - cron: '0 3 */14 * *'
  workflow_dispatch:
    inputs:
      model:
        description: 'Model ID (e.g. anthropic/claude-sonnet-4-20250514)'
        required: true
        default: 'anthropic/claude-sonnet-4-20250514'
      provider:
        description: 'Provider name'
        required: true
        default: 'anthropic'

env:
  OXIOS_E2E: '1'
  OXIOS_MODEL: ${{ inputs.model }}
  ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}

jobs:
  e2e-real-pipeline:
    name: E2E real pipeline (${{ inputs.model }})
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          repository: a7garden/oxi
          path: oxi
          ref: v0.4.4

      - uses: dtolnay/rust-toolchain@stable

      - uses: actions/checkout@v4

      - name: Run E2E pipeline tests
        run: cargo test -p oxios --test e2e_real_pipeline -- --ignored --nocapture

  ouroboros-scenarios:
    name: Ouroboros interview scenarios (${{ inputs.model }})
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          repository: a7garden/oxi
          path: oxi
          ref: v0.4.4

      - uses: dtolnay/rust-toolchain@stable

      - uses: actions/checkout@v4

      - name: Run scenario tests
        run: cargo test -p oxios-ouroboros --test scenario_test -- --ignored --nocapture
```

**Secrets required:** `ANTHROPIC_API_KEY` (GitHub repo secrets).

---

## 5. E2E Tests That Should NOT Be Automated

The following are intentionally left as `#[ignore]` and run only manually:

| File | Test | Reason |
|------|------|--------|
| `tests/e2e_real_pipeline.rs` | `test_full_interview_to_seed` | Real LLM API calls — expensive and provider-dependent |
| `tests/e2e_real_pipeline.rs` | `test_evaluate_with_cache` | Real LLM API calls + tests LLM-specific behavior |
| `crates/oxios-ouroboros/tests/scenario_test.rs` | `test_interview_scenarios` | LLM classification accuracy benchmark |
| `crates/oxios-kernel/src/embedding/gguf/mod.rs` | `test_embed_produces_dense_vector` | 329MB model download |
| `crates/oxios-kernel/src/embedding/gguf/mod.rs` | `test_embed_korean` | 329MB model download |

**Rationale:** These test LLM behavior, not code behavior. LLM behavior varies across versions, making them flaky in CI.

---

## 6. Summary: What Changes to Make

| Priority | Change | Effort |
|----------|--------|--------|
| 🔴 **Do now** | Add `cargo test --workspace --doc` to CI (fmt dependency) | 30 min |
| 🟡 **Next sprint** | Add `integration-tests.yml` weekly job | 2 hours |
| 🟢 **Future** | Add `e2e-llm.yml` with secrets for bi-weekly run | 4 hours |
| 🟢 **Future** | Implement integration tests from INTEGRATION-TEST-DESIGN.md | 9-14h |

### Files to Modify

1. `.github/workflows/ci.yml` — add `doc-test` job
2. Create `.github/workflows/integration-tests.yml`
3. Create `.github/workflows/e2e-llm.yml` (with secrets configuration documentation)

### What NOT to Change

- The 4-way test partitioning (still appropriate)
- Test file locations (current layout is good)
- Existing test patterns (`MockOuroboros`, `MockSupervisor`)

---

## 7. Verification Checklist

- [ ] `cargo test --workspace` — all 1,233 tests pass
- [ ] `cargo test --doc -p oxios-kernel` — 16 tests, 0 failed
- [ ] `cargo build --workspace` — compiles clean
- [ ] `.github/workflows/ci.yml` includes `--doc` step (after merge)
- [ ] Integration test files compile: `cargo test --no-run -p oxios-kernel`