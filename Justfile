# Oxios development commands

# Default: build and test
default: build test

# Build the workspace
build:
    cargo build --workspace

# Build in release mode
release:
    cargo build --release

# Run all tests
test:
    cargo nextest run --workspace

# Run all tests with cargo test (legacy, slower)
test-legacy:
    cargo test --workspace

# Run tests with CI profile (retries for flaky tests)
test-ci:
    cargo nextest run --workspace --profile ci

# Run doc tests
test-doc:
    cargo test --workspace --doc

# Run Clippy with warnings
lint:
    cargo clippy -p oxios -p oxios-kernel -p oxios-ouroboros -p oxios-gateway -p oxios-web -- -D warnings

# Format code
fmt:
    cargo fmt --all

# Check formatting without changes
fmt-check:
    cargo fmt --all -- --check

# Full CI check (format + lint + test)
ci: fmt-check lint test

# Full CI pipeline (like GitHub Actions)
ci-full:
    cargo fmt --all -- --check
    cargo clippy -p oxios -p oxios-kernel -p oxios-ouroboros -p oxios-gateway -p oxios-markdown -p oxios-mcp -- -D warnings
    cargo nextest run --workspace --profile ci
    cargo test --workspace --doc

# Build with sccache (shared compilation cache)
build-sccache:
    @sccache --start-server 2>/dev/null || true
    export RUSTC_WRAPPER=sccache && cargo build --workspace

# Show sccache statistics
sccache-stats:
    sccache --show-stats

# Clear sccache statistics
sccache-zero:
    sccache --zero-stats

# Run the server
run:
    cargo run

# Build the Dioxus WASM frontend
frontend:
    cd channels/oxios-web/frontend && dx build --release

# Clean build artifacts
clean:
    cargo clean

# Clean and show disk savings
clean-full:
    @echo "Before:"
    @du -sh target/ 2>/dev/null || echo "  no target/"
    cargo clean
    @echo "After: clean."

# Show test fitness summary
fitness:
    cargo nextest run --workspace 2>&1 | tail -5

# Build performance benchmark
benchmark:
    @echo "=== Cold build ==="
    cargo clean -p oxios-kernel
    time cargo build -p oxios-kernel
    @echo ""
    @echo "=== Cached build ==="
    time cargo build -p oxios-kernel
    @echo ""
    @echo "=== nextest run ==="
    time cargo nextest run --workspace
