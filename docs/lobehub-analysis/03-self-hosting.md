# LobeHub ↔ Oxios: Self-Hosting Architecture

> LobeHub: 9-container Docker Compose with PostgreSQL, MinIO, Casdoor, observability.
> Oxios: Single Rust binary, filesystem-only. Both have their place.

## 1. LobeHub Deployment Architecture

### Docker Compose (Production)

LobeHub's production stack spans **9 services**:

```
┌─────────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐
│  lobehub    │  │postgresql│  │  minio   │  │ casdoor  │  │ searxng  │
│  (app:3210) │  │ (pg17)   │  │  (S3)    │  │  (SSO)   │  │ (search) │
└─────────────┘  └──────────┘  └──────────┘  └──────────┘  └──────────┘
┌──────────┐  ┌───────────┐  ┌──────────┐  ┌───────────────┐
│ grafana  │  │   tempo   │  │prometheus│  │ otel-collector│
│(dashbrds)│  │ (traces)  │  │(metrics) │  │   (ingest)    │
└──────────┘  └───────────┘  └──────────┘  └───────────────┘
```

**Dockerfile** (`/tmp/lobehub/Dockerfile`):
- **Multi-stage**: `base` (node:24-slim) → `builder` (pnpm + Next.js build) → `app` (busybox) → `scratch`
- **Final image**: ~200MB (Node + Proxychains + Next.js standalone + Vite SPA)
- **Exposes**: Port 3210
- **Startup**: `node /app/startServer.js` — runs DB migrations then starts server

**Key services**:

| Service | Image | Role |
|---------|-------|------|
| `postgresql` | `paradedb/paradedb:latest-pg17` | DB with pgvector + pg_search |
| `minio` | `minio/minio` | S3-compatible object storage |
| `casdoor` | `casbin/casdoor:v2.13.0` | SSO/OAuth identity provider |
| `searxng` | `searxng/searxng` | Privacy-respecting web search |
| `grafana` | `grafana/grafana:12.2` | Observability dashboards |
| `prometheus` | `prom/prometheus` | Metrics collection |
| `tempo` | `grafana/tempo:latest` | Distributed tracing |
| `otel-collector` | `otel/opentelemetry-collector` | OTEL ingestion pipeline |

**Startup verification**: The lobe container runs a startup script that:
1. Waits for PostgreSQL health check
2. Verifies Casdoor OIDC `.well-known/openid-configuration` is reachable
3. Verifies MinIO health endpoint
4. Warns (does not fail) if any check fails — bilingual (EN/ZH) error messages

### One-Command Setup

```bash
bash <(curl -fsSL https://lobe.li/setup.sh)
```

The setup script (`docker-compose/setup.sh`, 25.8KB) handles:
- Directory creation
- `.env` file generation with secure random secrets
- Docker Compose pull + up
- Post-setup verification

### Database (PostgreSQL)

**ParadeDB** (PostgreSQL 17 + `pg_search` + `pgvector`):
- **40+ schema modules**: agent, user, workspace, message, session, file, rag, rbac, betterAuth, oidc, apiKey, aiInfra, notification, task, work, verify
- **Drizzle ORM**: Type-safe schema definitions with migrations
- **Key encryption**: `KEY_VAULTS_SECRET` for encrypting API keys at rest

### Storage (S3/MinIO)

- **MinIO**: Self-hosted S3-compatible storage
- **Supported backends**: MinIO, Cloudflare R2, AWS S3, any S3-compatible
- **Uses**: File uploads, image generation results, knowledge base documents, avatars

### Redis (Optional)

- **Uses**: Session caching, rate limiting, pub/sub for real-time updates
- **Config**: `REDIS_URL`, optional `REDIS_USERNAME`/`REDIS_PASSWORD`, TLS support

## 2. Auth & SSO

### Better Auth Integration

LobeHub uses **Better Auth** with **18+ SSO providers**:

| Provider | Type |
|----------|------|
| Google, GitHub, Microsoft, Apple | OAuth 2.0 |
| Cognito, Auth0, Authentik, Keycloak, Logto, Okta, ZITADEL | Enterprise SSO |
| Casdoor, Cloudflare Zero Trust, Authelia | Self-hosted SSO |
| Feishu, WeChat | Platform-specific |
| Generic OIDC | Any OIDC-compatible |

Auth features:
- Email/password with optional verification
- Magic link authentication
- SSO-only mode (disable email/password)
- Domain-restricted registration
- Trusted origins for CORS

## 3. Observability Stack

### OpenTelemetry Pipeline

```
App (otel SDK) → otel-collector → Prometheus (metrics) + Tempo (traces)
                                  → Grafana (dashboards)
```

**SDK** (`packages/observability-otel/src/node.ts`):
- Auto-instrumentation: HTTP + PostgreSQL
- OTLP metric exporter (periodic)
- OTLP trace exporter
- Environment-aware attributes (Vercel + Node.js)

**Config** (docker-compose env vars):
```bash
OTEL_EXPORTER_OTLP_METRICS_PROTOCOL=http/protobuf
OTEL_EXPORTER_OTLP_METRICS_ENDPOINT=http://localhost:4318/v1/metrics
OTEL_EXPORTER_OTLP_TRACES_PROTOCOL=http/protobuf
OTEL_EXPORTER_OTLP_TRACES_ENDPOINT=http://localhost:4318/v1/traces
```

## 4. Multi-Platform Deployment

LobeHub supports **6 deployment platforms**, each with dedicated docs:

| Platform | Method | Complexity |
|----------|--------|------------|
| **Docker** | `docker compose up -d` | Medium |
| **Vercel** | One-click deploy button | Low |
| **Zeabur** | One-click deploy | Low |
| **Sealos** | One-click deploy | Low |
| **Dokploy** | Git-based deploy | Medium |
| **RepoCloud** | One-click deploy | Low |

## 5. Oxios Deployment Model

### Current: Single Binary

```bash
oxios serve    # Starts daemon on localhost:9876
```

- **Zero dependencies**: No PostgreSQL, no Redis, no S3
- **State**: Filesystem-based (`~/.oxios/`)
- **Metrics**: Built-in Prometheus endpoint (`/metrics`)
- **Health**: `/health` endpoint
- **Web UI**: Embedded React SPA served by the daemon

### Strengths of Oxios's Model

1. **Zero-config startup**: `oxios serve` and you're running
2. **No Docker required**: Works on macOS, Linux, Windows
3. **Local-first**: All data stays on the user's machine
4. **Low resource**: Single process, minimal memory
5. **Simple maintenance**: No database migrations, no service orchestration

### Gaps vs LobeHub

| Aspect | LobeHub | Oxios |
|--------|---------|-------|
| Multi-user | Workspaces with RBAC | Single user |
| Persistence | PostgreSQL (durable, queryable) | Filesystem (JSON/Markdown) |
| Scalability | Horizontally scalable | Single process |
| Auth | Better Auth + 18 SSO providers | None (local daemon) |
| File storage | S3/MinIO (shared) | Filesystem (~/.oxios) |
| Monitoring | Grafana + Prometheus + Tempo | Prometheus /metrics only |
| One-click deploy | `curl \| bash` setup script | `cargo install oxios` |
| Backup | DB dumps + S3 snapshots | Filesystem copy |

## 6. Design Recommendations for Oxios

### Phase 1: Docker Compose for Self-Hosters

Create `docker-compose/production/` with:

```yaml
services:
  oxios:
    image: ghcr.io/a7garden/oxios:latest
    ports: ["9876:9876"]
    volumes:
      - oxios_data:/root/.oxios
    environment:
      - OXIOS_ANTHROPIC_API_KEY=${OXIOS_ANTHROPIC_API_KEY}
      - OXIOS_OPENAI_API_KEY=${OXIOS_OPENAI_API_KEY}

  # Optional PostgreSQL for persistence
  postgres:
    image: postgres:17
    # Only needed if Oxios gains DB backend

  # Optional Redis for caching
  redis:
    image: redis:7-alpine
    # Only needed if Oxios gains Redis backend
```

### Phase 2: Optional Persistence Backend

Add PostgreSQL support to Oxios:
- Feature gate: `oxios serve --db postgres://...`
- Store: sessions, agent state, chat history, API keys
- Migration: SQLx or refinery migrations
- Fallback: Filesystem when no DB configured

### Phase 3: Optional Team Mode

For teams wanting shared Oxios:
- Add SSO via OAuth2 (Google, GitHub)
- Workspace-level isolation
- Shared knowledge base

### Phase 4: Observability

- OpenTelemetry tracing in the Rust daemon
- Structured logging with trace IDs
- Optional Grafana dashboard for self-hosters

### What NOT to Adopt

1. **Casdoor**: Overkill for Oxios. Use simple OAuth2 if team mode is added.
2. **9-container compose**: Oxios should work with just the daemon. Add services optionally.
3. **ParadeDB**: Standard PostgreSQL is sufficient. pgvector can be a separate extension.
4. **SearXNG**: Oxios agents can use web search tools directly; no need for a search service.
5. **Tempo + otel-collector**: Use `tracing-opentelemetry` with direct OTLP export to Grafana Cloud or self-hosted collector.
