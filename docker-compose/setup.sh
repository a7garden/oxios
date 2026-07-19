#!/usr/bin/env bash
# Oxios Self-Hosting Setup Script
# One-command installer: curl -fsSL https://oxios.dev/setup.sh | bash
#
# Creates docker-compose environment, generates .env with secure secrets,
# and starts the Oxios daemon with optional PostgreSQL + Redis.

set -euo pipefail

OXIOS_DIR="${OXIOS_DIR:-$HOME/oxios-docker}"
OXIOS_PORT="${OXIOS_PORT:-9876}"

echo ""
echo "  ⬡ Oxios Self-Hosting Setup"
echo "  ─────────────────────────"
echo ""

# ── Check prerequisites ──────────────────────────────────────────
command -v docker >/dev/null 2>&1 || {
  echo "  ❌ Docker is required but not installed."
  echo "     Install: https://docs.docker.com/get-docker/"
  exit 1
}

docker compose version >/dev/null 2>&1 || {
  echo "  ❌ docker compose is required but not available."
  echo "     Docker Compose is included with Docker Desktop."
  exit 1
}

# ── Create directory ─────────────────────────────────────────────
mkdir -p "$OXIOS_DIR"
cd "$OXIOS_DIR"

# ── Generate .env ─────────────────────────────────────────────────
if [ -f .env ]; then
  echo "  ⚠ .env already exists — skipping generation."
  echo "    Delete $OXIOS_DIR/.env to regenerate."
else
  POSTGRES_PASSWORD=$(openssl rand -hex 16 2>/dev/null || echo "oxios-change-me")

  cat > .env << ENVEOF
# ── Provider API keys (at least one required) ──
OXIOS_ANTHROPIC_API_KEY=
OXIOS_OPENAI_API_KEY=
OXIOS_GOOGLE_API_KEY=
OXIOS_DEEPSEEK_API_KEY=
OXIOS_GROQ_API_KEY=
OXIOS_OPENROUTER_API_KEY=
OXIOS_MISTRAL_API_KEY=

# ── Server ──
OXIOS_PORT=${OXIOS_PORT}
OXIOS_LOG_LEVEL=info

# ── Optional: PostgreSQL (uncomment to enable) ──
# POSTGRES_URL=postgres://oxios:${POSTGRES_PASSWORD}@postgres:5432/oxios
# POSTGRES_PASSWORD=${POSTGRES_PASSWORD}

# ── Optional: Redis (uncomment to enable) ──
# REDIS_URL=redis://redis:6379
ENVEOF

  echo "  ✓ .env generated at $OXIOS_DIR/.env"
  echo "    Edit it to add your provider API keys."
fi

# ── Write docker-compose.yml ──────────────────────────────────────
cat > docker-compose.yml << 'COMPOSEEOF'
name: oxios

services:
  oxios:
    image: ghcr.io/a7garden/oxios:latest
    container_name: oxios
    restart: unless-stopped
    ports:
      - '${OXIOS_PORT:-9876}:9876'
    volumes:
      - oxios_data:/root/.oxios
      - ${HOME:-/root}:/host-home:ro
    environment:
      - OXIOS_ANTHROPIC_API_KEY=${OXIOS_ANTHROPIC_API_KEY:-}
      - OXIOS_OPENAI_API_KEY=${OXIOS_OPENAI_API_KEY:-}
      - OXIOS_GOOGLE_API_KEY=${OXIOS_GOOGLE_API_KEY:-}
      - OXIOS_DEEPSEEK_API_KEY=${OXIOS_DEEPSEEK_API_KEY:-}
      - OXIOS_GROQ_API_KEY=${OXIOS_GROQ_API_KEY:-}
      - OXIOS_OPENROUTER_API_KEY=${OXIOS_OPENROUTER_API_KEY:-}
      - OXIOS_MISTRAL_API_KEY=${OXIOS_MISTRAL_API_KEY:-}
      - DATABASE_URL=${POSTGRES_URL:-}
      - REDIS_URL=${REDIS_URL:-}
      - RUST_LOG=${OXIOS_LOG_LEVEL:-info}
    healthcheck:
      test: ['CMD', 'curl', '-f', 'http://localhost:9876/health']
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s

volumes:
  oxios_data:
    driver: local
COMPOSEEOF

echo "  ✓ docker-compose.yml written."

# ── Pull and start ────────────────────────────────────────────────
echo ""
echo "  Pulling Oxios image..."
docker compose pull oxios 2>&1 | tail -1

echo ""
echo "  Starting Oxios..."
docker compose up -d oxios

# ── Wait for health ───────────────────────────────────────────────
echo ""
echo "  Waiting for Oxios to be ready..."
for i in $(seq 1 30); do
  if curl -sf "http://localhost:${OXIOS_PORT}/health" >/dev/null 2>&1; then
    echo ""
    echo "  ✅ Oxios is running!"
    echo ""
    echo "  Dashboard: http://localhost:${OXIOS_PORT}"
    echo "  Logs:      docker compose logs -f oxios"
    echo "  Stop:      docker compose down"
    echo ""
    echo "  ⚠ Don't forget to add your API keys to $OXIOS_DIR/.env"
    echo "     then restart: docker compose restart oxios"
    echo ""
    exit 0
  fi
  sleep 1
done

echo ""
echo "  ⚠ Oxios started but health check timed out."
echo "    Check logs: docker compose logs oxios"
echo "    Dashboard:  http://localhost:${OXIOS_PORT}"
