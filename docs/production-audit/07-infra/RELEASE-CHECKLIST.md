# Release Checklist

Use this checklist for every release. Complete all items before publishing.

---

## Pre-release

- [ ] `cargo test --workspace` passes (all platforms)
- [ ] `cargo clippy --workspace --all-features -- -D warnings` clean
- [ ] `cargo fmt --all -- --check` clean
- [ ] Frontend: `cd surface/oxios-web/web && bun install && bun run build` succeeds
- [ ] Frontend: `cd surface/oxios-web/web && npx tsc --noEmit` zero errors
- [ ] `cargo audit` — no critical/high vulnerabilities
- [ ] Version bumped in **all** `Cargo.toml` files:
  - [ ] Root `Cargo.toml` (`version = "0.x.0"`)
  - [ ] `crates/oxios-kernel/Cargo.toml`
  - [ ] `crates/oxios-mcp/Cargo.toml`
  - [ ] `crates/oxios-markdown/Cargo.toml`
  - [ ] `crates/oxios-ouroboros/Cargo.toml`
  - [ ] `crates/oxios-gateway/Cargo.toml`
  - [ ] `surface/oxios-web/Cargo.toml`
  - [ ] `channels/oxios-cli/Cargo.toml`
  - [ ] `channels/oxios-telegram/Cargo.toml`
  - [ ] `benchmarks/oxios-bench/Cargo.toml` (if versioned)
- [ ] `CHANGELOG.md` updated with version, date, and changes
- [ ] `Cargo.lock` updated (`cargo update -p oxios` or full rebuild)
- [ ] Docker image builds: `docker build .`
- [ ] All CI checks pass on the release branch

## Build

- [ ] `cargo build --release` succeeds
- [ ] Binary size is reasonable (≤ 50 MB target; current: ~37 MB arm64)
- [ ] Smoke test: `./target/release/oxios --version` outputs correct version
- [ ] Smoke test: `./target/release/oxios --help` exits cleanly
- [ ] Cross-platform build verified (CI release-check job)

## Docker

- [ ] `docker build .` succeeds
- [ ] Container starts: `docker run --rm -p 4200:4200 oxios`
- [ ] Health check passes: `curl http://localhost:4200/health`
- [ ] Image size is reasonable (≤ 150 MB target)
- [ ] Non-root user verified: `docker run --rm oxios whoami` → `oxios`

## Tag & Publish

- [ ] Git tag created: `git tag -a v0.x.0 -m "Release v0.x.0"`
- [ ] Tag pushed: `git push origin v0.x.0` (triggers release workflow)
- [ ] GitHub Actions release workflow succeeds
- [ ] GitHub release published with all binaries + checksums
- [ ] Docker image pushed to registry (if applicable)
- [ ] Verify SHA256 checksums match between CI output and manual download

## Post-release

- [ ] Verify GitHub release page is complete with all 4 platform binaries
- [ ] Verify web assets are included in the release
- [ ] Test download + run on a clean machine (or Docker container)
- [ ] Update documentation with new version number
- [ ] Announce release (if applicable)
- [ ] Create follow-up issue for next release planning

---

## Emergency Hotfix Process

For critical fixes that need immediate release:

1. Branch from the release tag: `git checkout -b hotfix/v0.x.1 v0.x.0`
2. Apply the minimal fix
3. Run abbreviated checklist (test + clippy + build only)
4. Tag and push: `git tag v0.x.1 && git push origin v0.x.1`
5. Verify release workflow completes
6. Merge hotfix back to `main`

## Version Scheme

Follow [Semantic Versioning](https://semver.org/):

- **Patch** (`0.6.x`): Bug fixes, security patches — no API changes
- **Minor** (`0.x.0`): New features, non-breaking API additions
- **Major** (`x.0.0`): Breaking API changes (pre-1.0, minor version indicates breaking)

Current version: **0.6.0**
