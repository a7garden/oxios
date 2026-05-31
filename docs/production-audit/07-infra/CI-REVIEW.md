# CI Pipeline Review

**Date:** 2025-05-31  
**Files reviewed:** `.github/workflows/ci.yml`, `.github/workflows/release.yml`

---

## Overview

The CI pipeline is mature and well-structured. Two workflows:

1. **`ci.yml`** — Runs on push to `main`/`develop` and on all PRs
2. **`release.yml`** — Runs on version tag push (`v*`)

---

## CI Workflow (`ci.yml`) — Assessment

### Job Dependency Graph

```
fmt ─────┐
         ├─→ test (4 partitions × 2 OS)
frontend ┘    coverage
              audit (independent)
              release-check
                    │
                    └─→ summary
```

### What's Working Well ✅

| Aspect | Detail |
|--------|--------|
| **Platform matrix** | macOS + Linux coverage ✅ |
| **Formatter gating** | `fmt` runs on both OSes ✅ |
| **Clippy** | `--all-features` with `-D warnings` ✅ |
| **Test partitioning** | 4-way partition with `cargo nextest` ✅ |
| **Coverage** | `grcov` + Codecov upload ✅ |
| **Security audit** | `cargo audit` as standalone job ✅ |
| **Release build check** | Verifies release compilation ✅ |
| **Fail-fast disabled** | All partitions run to completion ✅ |
| **Frontend validation** | Type check + build ✅ |
| **Summary job** | `if: always()` — reports even on failure ✅ |

### Issues & Gaps

#### 🟡 1. Cache Strategy — Suboptimal Key Granularity

**Current:**
```yaml
- name: Cache target directory
  uses: actions/cache@v4
  with:
    path: target
    key: ${{ runner.os }}-target-${{ hashFiles('**/Cargo.lock') }}
    restore-keys: |
      ${{ runner.os }}-target-
```

**Problem:** The `target/` directory cache key only depends on `Cargo.lock`. When source code changes (which is every push), the cache key doesn't match and falls back to the partial restore key. This works but the target cache can grow unbounded.

**Recommendation:**
```yaml
- name: Cache target directory
  uses: actions/cache@v4
  with:
    path: target
    key: ${{ runner.os }}-target-${{ hashFiles('**/Cargo.lock') }}-${{ github.sha }}
    restore-keys: |
      ${{ runner.os }}-target-${{ hashFiles('**/Cargo.lock') }}-
```

Using `${{ github.sha }}` ensures unique keys per commit, and `restore-keys` provides partial cache hits from the same `Cargo.lock` state.

#### 🟡 2. No Test Matrix Feature Variation

**Current:** Tests run with default features only (`cargo nextest run --workspace`).

**Problem:** Feature-gated code paths (e.g., `browser`, `sqlite-memory`, `telegram`) are not tested in CI. The `release-check` job tests `web,cli,sqlite-memory` but doesn't run tests.

**Recommendation:** Add a feature-matrix test job (or expand existing):
```yaml
strategy:
  matrix:
    features:
      - "web,cli,sqlite-memory"
      - "web,cli,browser"
```

At minimum, test the same features the release workflow uses.

#### 🟡 3. `coverage` Job Uses `cargo test` Instead of `nextest`

**Current:**
```yaml
- name: Run tests with coverage
  run: cargo test --workspace
```

**Problem:** The coverage job doesn't use `cargo nextest`, which means:
- Different test execution path than the main test job
- Slower (no parallel test runner)
- Coverage and test jobs may disagree on pass/fail

**Recommendation:** Use `cargo nextest run --workspace` with the coverage `RUSTFLAGS`. Note: nextest coverage support requires `cargo-llvm-cov` or specific configuration.

#### 🟡 4. Separate Registry and Git Cache

**Current:** Two separate cache entries:
```yaml
- name: Cache cargo registry
  path: ~/.cargo/registry
- name: Cache cargo index
  path: ~/.cargo/git
```

**This is actually correct.** Separating them allows partial cache hits (registry might be valid even if git index needs refresh). No change needed.

#### 🟢 5. No Stale PR/Issue Cleanup

**Observation:** No automated cleanup for stale PRs or branches.

**Recommendation (low priority):** Add `actions/stale` workflow:
```yaml
name: Stale
on:
  schedule:
    - cron: '0 0 * * 0'  # Weekly
jobs:
  stale:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/stale@v9
        with:
          days-before-pr-stale: 30
          days-before-pr-close: 7
```

#### 🟢 6. No Nightly/Full E2E Job

**Observation:** No scheduled full E2E or nightly regression test.

**Recommendation (low priority):** Consider a weekly cron job that runs all features + benchmarks:
```yaml
on:
  schedule:
    - cron: '0 2 * * 0'  # Sunday 2 AM UTC
```

#### 🟢 7. `audit` Job Installs `cargo-audit` Every Run

**Current:**
```yaml
- name: Security audit
  run: |
    cargo install cargo-audit --locked
    cargo audit
```

**Problem:** `cargo install cargo-audit` takes ~30-60s every run.

**Recommendation:** Use `taiki-e/install-action` (already used for `nextest` and `grcov`):
```yaml
- name: Install cargo-audit
  uses: taiki-e/install-action@cargo-audit
- name: Security audit
  run: cargo audit
```

---

## Release Workflow (`release.yml`) — Assessment

### What's Working Well ✅

| Aspect | Detail |
|--------|--------|
| **Cross-platform builds** | Linux x86_64/ARM64, macOS Intel/Apple Silicon ✅ |
| **Cross-compilation** | `aarch64-linux-gnu` with proper linker ✅ |
| **SHA256 checksums** | Generated for every binary ✅ |
| **Web assets** | Separate build + zipped distribution ✅ |
| **Smoke tests** | Verifies binary runs before release ✅ |
| **GitHub release** | Auto-created with proper naming ✅ |
| **Feature flags** | Consistent `--no-default-features --features "web,cli,sqlite-memory"` ✅ |

### Issues & Gaps

#### 🟡 1. ARM64 Linux Smoke Test Missing

**Current:** Smoke tests run on:
- `x86_64-unknown-linux-gnu` (on `ubuntu-latest`)
- `aarch64-apple-darwin` (on `macos-latest`)

**Missing:** `aarch64-unknown-linux-gnu` is not smoke-tested. The ARM64 Linux binary is built via cross-compilation on an x86_64 runner, which can produce binaries that fail at runtime due to linker issues.

**Recommendation:** Use QEMU or a native ARM64 runner for ARM64 Linux smoke testing:
```yaml
- target: aarch64-unknown-linux-gnu
  runner: ubuntu-latest
  # Use qemu for cross-architecture smoke test
```

Or use the newer `ubuntu-24.04-arm` runner type if available.

#### 🟡 2. macOS Intel Smoke Test Missing

**Current:** Only `aarch64-apple-darwin` is smoke-tested.

**Recommendation:** Add `x86_64-apple-darwin` to the smoke test matrix (runs on `macos-latest` which is ARM64, but the binary can still be executed via Rosetta 2).

#### 🟢 3. No Install Script Verification

**Current:** The release body includes:
```
curl -fsSL .../install.sh | bash
```

But no `install.sh` is generated or tested in the workflow.

**Recommendation:** Either create the install script and test it in the release workflow, or remove the curl-pipe-bash instruction from the release notes.

---

## Security Observations

### ✅ Good Practices

1. **`dtolnay/rust-toolchain@stable`** — Pinned to stable, not nightly
2. **`actions/checkout@v4`** — Using latest major version
3. **`softprops/action-gh-release@v2`** — Properly versioned
4. **`permissions: contents: write`** — Scoped to release job only
5. **`fail-fast: false`** — All matrix entries complete even if one fails

### 🟡 Improvement Areas

1. **Pin action SHAs:** Consider pinning to full commit SHA instead of version tag for supply-chain security
2. **Add `permissions` block** at workflow level to restrict default tokens:
   ```yaml
   permissions:
     contents: read
   ```
   Then override per-job as needed.

---

## Performance Observations

### Estimated CI Duration

| Job | Estimated Time | Parallel? |
|-----|---------------|-----------|
| `fmt` | 2-3 min | Yes (2 OS) |
| `frontend` | 1-2 min | Yes |
| `audit` | 1-2 min | Yes |
| `test` (8 jobs) | 5-8 min | Yes (partition × OS) |
| `coverage` | 8-12 min | Yes |
| `release-check` | 5-8 min | Yes |
| **Total wall time** | **~12-15 min** | |

This is reasonable for a Rust project of this size.

### With New Release Profile

The `release-check` job will take ~1-2 min longer due to `lto = "thin"` and `codegen-units = 1`. This is acceptable.

---

## Recommendations Summary

| Priority | Item | Effort | Impact |
|----------|------|--------|--------|
| 🟡 Medium | Test feature-gated code paths | 30 min | Catches feature-specific bugs |
| 🟡 Medium | Use `taiki-e/install-action` for `cargo-audit` | 5 min | Saves 30-60s per run |
| 🟡 Medium | Add ARM64 Linux smoke test | 1 hr | Catches cross-compilation issues |
| 🟡 Medium | Add workflow-level `permissions` | 5 min | Security hardening |
| 🟢 Low | Add stale PR cleanup | 15 min | Repo hygiene |
| 🟢 Low | Add nightly E2E job | 1 hr | Early regression detection |
| 🟢 Low | Improve target cache key | 5 min | Better cache hit rates |
| 🟢 Low | Verify/remove install.sh reference | 30 min | Release correctness |
