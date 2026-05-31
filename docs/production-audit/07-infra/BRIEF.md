# Brief 07: Infrastructure — Release Profile & Deployment Hardening

**Area:** Build optimization, Docker, CI/CD, deployment readiness  
**Severity:** 🟡 High  
**Estimated scope:** No release profile, Dockerfile exists, CI is mature  

---

## Context

**Build & CI:**
- CI runs on GitHub Actions: fmt + clippy + test (4-way partition) + frontend
- Cross-platform: macOS + Linux
- Uses `cargo-nextest` for parallel test execution ✅
- Frontend: `bun install && bun run build` ✅
- Cache: cargo registry + git index ✅

**Critical gap: No `[profile.release]` configuration.**

This means:
- No LTO (Link-Time Optimization) → larger binary, slower startup
- No codegen-units tuning → suboptimal runtime performance
- No strip → debug symbols in production binary (huge)
- No panic = abort → unnecessary unwinding overhead

**Docker:**
- Multi-stage build exists (`Dockerfile`)
- Browser feature disabled in containers ✅
- Based on `rust:1.85-bookworm` + `debian:bookworm-slim` ✅
- No `.dockerignore` optimization audit

**Daemon:**
- PID file management ✅
- launchd (macOS) / systemd (Linux) support ✅
- Foreground mode for containers ✅

---

## Objective

1. **Add a production-ready `[profile.release]`** to root `Cargo.toml`
2. **Optimize the Dockerfile** for size and build speed
3. **Audit CI pipeline** for gaps
4. **Create deployment checklist**

This does NOT mean:
- ❌ Setting up Kubernetes or orchestration
- ❌ Adding CD (continuous deployment) pipelines
- ❌ Creating a homebrew formula or package manager distribution
- ❌ Over-optimizing binary size at the cost of debuggability

It DOES mean:
- ✅ Adding `[profile.release]` with LTO, strip, and panic handling
- ✅ Ensuring the Docker build is efficient (layer caching)
- ✅ Documenting the release process

---

## Approach

### Phase 1: Release Profile

Add to root `Cargo.toml`:

```toml
[profile.release]
lto = "thin"          # Good balance of build time vs binary size
codegen-units = 1     # Better optimization, slower compile
strip = true          # Remove debug symbols
panic = "abort"       # Smaller binary, no unwinding
opt-level = 3         # Maximum speed (default for release, but explicit)
```

Then:
1. `cargo build --release` — verify it compiles
2. Compare binary size before/after
3. Run `cargo test --workspace --release` — verify tests pass
4. Document the trade-offs in `docs/production-audit/07-infra/RELEASE-PROFILE.md`

**Note:** `panic = "abort"` means `catch_unwind` won't work. Verify
that no code relies on panic catching. The Circuit Breaker and graceful
shutdown should be unaffected (they use `Result`, not panic catching).

### Phase 2: Dockerfile Optimization

Audit the existing Dockerfile:

1. Check `.dockerignore` — are unnecessary files excluded?
2. Verify layer caching order:
   - Cargo.toml/Cargo.lock first (dependency layer)
   - Source code second (code layer)
3. Consider adding `--mount=type=cache` for cargo registry
4. Check if the final image size is reasonable
5. Verify the health check is present in the Dockerfile
6. Write findings to `docs/production-audit/07-infra/DOCKER-AUDIT.md`

### Phase 3: CI Pipeline Review

Review `.github/workflows/*.yml`:

1. Is the matrix comprehensive enough? (macOS + Linux ✅, Windows N/A)
2. Are there race conditions in the partition strategy?
3. Is the cache strategy optimal?
4. Is there a stale PR/branch cleanup?
5. Should there be a nightly/full E2E job?
6. Write recommendations to `docs/production-audit/07-infra/CI-REVIEW.md`

### Phase 4: Release Checklist

Create `docs/production-audit/07-infra/RELEASE-CHECKLIST.md`:

```markdown
# Release Checklist

## Pre-release
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo fmt --all -- --check` clean
- [ ] Frontend: `cd surface/oxios-web/web && bun run build` succeeds
- [ ] Frontend: `npx tsc --noEmit` zero errors
- [ ] `cargo audit` no critical/high vulnerabilities
- [ ] Version bumped in all Cargo.toml files
- [ ] CHANGELOG.md updated
- [ ] Docker image builds successfully

## Build
- [ ] `cargo build --release` succeeds
- [ ] Binary size is reasonable (< 50MB?)
- [ ] Smoke test: `./target/release/oxios --version`
- [ ] Smoke test: `./target/release/oxios doctor`

## Post-release
- [ ] Git tag created (v0.x.0)
- [ ] GitHub release published
- [ ] Docker image pushed to registry
```

---

## Constraints

- **Do not** change the CI workflow structure (it works)
- **Do not** add new CI jobs (only recommend)
- **Do not** create deployment automation
- **Do not** modify the daemon management code
- **Preserve** the current feature gate strategy in Docker builds

## Verification

1. `cargo build --release` — succeeds with new profile
2. `cargo test --workspace --release` — all tests pass
3. Binary size comparison documented
4. `docker build .` — succeeds with existing Dockerfile
