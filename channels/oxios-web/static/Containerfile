# Oxios Garden Containerfile
#
# Multi-stage build for the Oxios garden execution environment.
# Stage 1: builder — compiles the oxios binary
# Stage 2: runtime — minimal image with tools and oxios binary

# Stage 1: builder
FROM rust:1-slim AS builder

WORKDIR /app

# Copy workspace files
COPY . .

# Build oxios in release mode
RUN cargo build --release -p oxios

# Stage 2: runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl git ripgrep jq sqlite3 bash python3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary from the builder stage
COPY --from=builder /app/target/release/oxi /usr/local/bin/oxi

WORKDIR /workspace

CMD ["/bin/bash"]
