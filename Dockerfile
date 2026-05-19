# Oxios Agent OS — Dockerfile
#
# Multi-stage build for minimal runtime image.
# Browser feature is disabled in containers (no Chromium dependency).

# ── Build stage ──
FROM rust:1.85-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /oxios

# Copy manifests first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY channels/ channels/
COPY src/ src/
COPY share/ share/
COPY audit.toml .clippy.toml ./

# Build without browser (no Chromium needed in container)
RUN cargo build --release --no-default-features --features "web,cli"

# ── Runtime stage ──
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/*

# Non-root user for security
RUN groupadd -r oxios && useradd -r -g oxios -m oxios

COPY --from=builder /oxios/target/release/oxios /usr/local/bin/

# Configuration and data volume
VOLUME /home/oxios/.oxios
WORKDIR /home/oxios

USER oxios

EXPOSE 4200

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["curl", "-f", "http://localhost:4200/health"]

ENTRYPOINT ["oxios"]
CMD ["serve"]
