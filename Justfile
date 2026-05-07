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
    cargo test --workspace

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

# Run the server
run:
    cargo run

# Build the Dioxus WASM frontend
frontend:
    cd channels/oxios-web/frontend && dx build --release

# Clean build artifacts
clean:
    cargo clean
