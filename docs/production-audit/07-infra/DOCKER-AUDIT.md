# Dockerfile Audit

**Date:** 2025-05-31  
**Status:** 🔴 Critical bug found + optimization opportunities

---

## Summary

The Dockerfile has a **critical bug** that prevents the build from succeeding, plus several optimization opportunities for build speed and image size.

---

## Critical Bug: Missing `surface/` Directory

**Severity:** 🔴 Build-breaking

The Dockerfile uses `--features "web,cli"` but does **not** copy the `surface/` directory:

```dockerfile
# Current — MISSING surface/ copy
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY channels/ channels/
COPY src/ src/
COPY share/ share/
```

The `oxios-web` workspace member lives at `surface/oxios-web/Cargo.toml`. Without it, Cargo cannot resolve the workspace and the build fails:

```
error: failed to load manifest for workspace member `surface/oxios-web`
  --> Cargo.toml
  |
  | missing: `surface/oxios-web/Cargo.toml`
```

**Fix:**

```dockerfile
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY channels/ channels/
COPY surface/ surface/
COPY src/ src/
COPY share/ share/
```

---

## `.dockerignore` Issues

### Problem 1: Frontend source excluded but needed for web dist

The `.dockerignore` excludes frontend source files:

```
surface/oxios-web/web/node_modules/
surface/oxios-web/web/src/
surface/oxios-web/web/*.json
surface/oxios-web/web/*.ts
surface/oxios-web/web/*.js
```

But the Rust `oxios-web` crate needs `surface/oxios-web/src/` (the Rust source, not the web frontend). The exclusion of `surface/oxios-web/web/src/` is fine (frontend source not needed in Docker if pre-built), but excluding `surface/oxios-web/web/*.json` will exclude `package.json` and `bunfig.toml` which may be needed if the Dockerfile ever builds the frontend.

**Current impact:** Low — the Rust crate at `surface/oxios-web/` has its source in `surface/oxios-web/src/` which is NOT excluded. The exclusions only affect the `web/` subdirectory.

### Problem 2: Overly broad `*.md` exclusion

```
*.md
!README.md
```

This excludes all `.md` files except `README.md`. If any build script references a markdown file (e.g., `SKILL.md` files in `share/`), they'll be missing.

**Current impact:** Medium — `share/default-skills/` contains `SKILL.md` files. These are runtime assets, not build-time, but if the Docker image needs to serve skills, they'll be missing from the image.

### Recommended `.dockerignore` updates

```dockerignore
# Build artifacts
target/
debug/

# Version control
.git/
.gitignore

# IDE
.idea/
.vscode/
*.swp
*.swo

# OS files
.DS_Store
Thumbs.db

# CI/CD
.github/

# Scripts
scripts/

# Benchmarks
benchmarks/

# Frontend dev dependencies (keep Rust sources!)
surface/oxios-web/web/node_modules/

# Misc
*.log
.env
.env.*
```

Key changes:
- Remove `docs/` exclusion (not needed, already not copied)
- Remove `*.md` exclusion (runtime skill files need to be included)
- Keep `node_modules/` exclusion
- Don't exclude `surface/oxios-web/web/src/` (may be needed for frontend build in Docker)

---

## Layer Caching Analysis

### Current approach

```dockerfile
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/       # ← All source + all Cargo.toml files
COPY channels/ channels/   # ← All source + all Cargo.toml files
COPY src/ src/
COPY share/ share/
```

**Problem:** Any change to any `.rs` file invalidates the entire build layer. Dependencies are re-resolved (fast, from lockfile) but recompilation starts from scratch.

### Recommended approach

Split into dependency-resolution layer and source-code layer:

```dockerfile
# ── Dependency resolution layer (cached unless Cargo.toml/Cargo.lock change) ──
COPY Cargo.toml Cargo.lock ./
COPY crates/oxios-mcp/Cargo.toml crates/oxios-mcp/
COPY crates/oxios-kernel/Cargo.toml crates/oxios-kernel/
COPY crates/oxios-markdown/Cargo.toml crates/oxios-markdown/
COPY crates/oxios-ouroboros/Cargo.toml crates/oxios-ouroboros/
COPY crates/oxios-gateway/Cargo.toml crates/oxios-gateway/
COPY surface/oxios-web/Cargo.toml surface/oxios-web/
COPY channels/oxios-cli/Cargo.toml channels/oxios-cli/
COPY channels/oxios-telegram/Cargo.toml channels/oxios-telegram/

# Create dummy source files so cargo can resolve dependencies
RUN mkdir -p src && echo "fn main(){}" > src/main.rs \
    && find crates surface channels -name "Cargo.toml" -exec bash -c ' \
       dir=$(dirname "$1"); \
       [ -f "$dir/src/lib.rs" ] || mkdir -p "$dir/src" && touch "$dir/src/lib.rs"' _ {} \;

RUN cargo build --release --no-default-features --features "web,cli" 2>/dev/null || true

# ── Source code layer (invalidated on any source change) ──
COPY crates/ crates/
COPY surface/ surface/
COPY channels/ channels/
COPY src/ src/
COPY share/ share/

RUN cargo build --release --no-default-features --features "web,cli"
```

**Benefit:** Dependency compilation (the slowest part) is cached across source-only changes. Only Cargo.toml/Cargo.lock changes invalidate the dependency layer.

---

## Build Cache Mounts

### Current: No cache mounts

### Recommended: Add `--mount=type=cache`

```dockerfile
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/oxios/target \
    cargo build --release --no-default-features --features "web,cli"
```

**Benefit:** Cargo registry and build artifacts persist across `docker build` invocations (on the same builder). Reduces rebuild time from minutes to seconds when only source changes.

**Caveat:** Requires BuildKit (`DOCKER_BUILDKIT=1`), which is default in Docker 23+.

---

## Final Image Analysis

| Metric | Current | Assessment |
|--------|---------|------------|
| Base image | `debian:bookworm-slim` | ✅ Good — minimal footprint |
| Runtime packages | `ca-certificates libssl3 curl` | ✅ Minimal |
| Non-root user | `oxios` user | ✅ Security best practice |
| Health check | `curl -f http://localhost:4200/health` | ✅ Present with proper intervals |
| Exposed port | 4200 | ✅ |
| Volume | `/home/oxios/.oxios` | ✅ Data persistence |
| Entrypoint | `oxios` with `serve` default | ✅ Foreground mode for containers |

### Estimated final image size

- `debian:bookworm-slim`: ~80 MB
- `ca-certificates libssl3 curl`: ~10 MB
- `oxios` binary: ~50 MB (Linux x86_64, `web,cli` features)
- **Total estimate: ~140 MB**

This is reasonable for a Rust web application.

---

## Additional Findings

### 1. No `COPY --chown` for config files

If `share/` contains runtime config files that the `oxios` user needs to read, consider:

```dockerfile
COPY --chown=oxios:oxios --from=builder /oxios/share/ /home/oxios/.oxios/share/
```

### 2. No `.dockerignore` entry for `Cargo.lock` alternatives

The `.dockerignore` correctly does NOT exclude `Cargo.lock`. This is correct — `Cargo.lock` should be in the image for reproducible builds.

### 3. Missing frontend build in Docker

The Dockerfile only does a Rust build. If the web frontend needs to be served from the container, a frontend build step is needed:

```dockerfile
# ── Frontend build stage (optional, if serving from container) ──
FROM oven/bun:1 AS frontend
WORKDIR /web
COPY surface/oxios-web/web/ .
RUN bun install && bun run build

# Then in the runtime stage:
# COPY --from=frontend /web/dist /home/oxios/.oxios/web/dist
```

Currently the Rust binary likely embeds or serves the frontend differently. Verify whether the container needs pre-built frontend assets.

### 4. Browser feature correctly disabled

```dockerfile
RUN cargo build --release --no-default-features --features "web,cli"
```

The browser feature (Chromium dependency) is correctly excluded. ✅

---

## Recommended Dockerfile (Complete)

```dockerfile
# Oxios Agent OS — Dockerfile
# Multi-stage build for minimal runtime image.

# ── Build stage ──
FROM rust:1.85-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /oxios

# Copy manifests first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY surface/ surface/
COPY channels/ channels/
COPY src/ src/
COPY share/ share/
COPY audit.toml .clippy.toml rusttoolchain.toml ./

# Build without browser (no Chromium needed in container)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/oxios/target \
    cargo build --release --no-default-features --features "web,cli"

# ── Runtime stage ──
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -r oxios && useradd -r -g oxios -m oxios

COPY --from=builder /oxios/target/release/oxios /usr/local/bin/

VOLUME /home/oxios/.oxios
WORKDIR /home/oxios

USER oxios

EXPOSE 4200

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["curl", "-f", "http://localhost:4200/health"]

ENTRYPOINT ["oxios"]
CMD ["serve"]
```

---

## Action Items

| Priority | Item | Effort |
|----------|------|--------|
| 🔴 Critical | Add `COPY surface/ surface/` to build stage | 1 line |
| 🟡 Medium | Update `.dockerignore` to not exclude runtime `.md` files | 5 min |
| 🟡 Medium | Add `--mount=type=cache` for cargo registry/target | 5 min |
| 🟢 Low | Split dependency and source layers for better caching | 15 min |
| 🟢 Low | Add frontend build stage if container serves static assets | 30 min |
