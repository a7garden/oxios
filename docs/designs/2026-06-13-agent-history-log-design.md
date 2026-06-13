# Agent History Log — Transparent Agent Observation

> **Date:** 2026-06-13 · **Status:** Draft · **Author:** Design session
> **Storage:** Filesystem (source of truth) + SQLite (query engine)

## Problem

The Web UI currently only shows **running** agents. Agent metadata (`AgentInfo`) lives in an in-memory `HashMap<AgentId, AgentInfo>` inside `BasicSupervisor`. When the daemon restarts, all agent records are lost.

This defeats the purpose of a supervisory dashboard. Users need to see:

- What agents were executed in the past
- When they ran, what they did, how long they took, how much they cost
- Which sessions spawned them
- The full execution trace and logs

## Design Goals

| Goal | Description |
|------|-------------|
| **Permanent log** | Every agent that runs leaves a persistent record, surviving daemon restarts |
| **Full history** | Browse past agents with rich filtering, search, sorting, pagination |
| **Fast queries** | O(log n) via SQLite indexes — no O(n) filesystem scan |
| **Search** | Full-text search across agent name, error, tool names, tool outputs (FTS5) |
| **Transparent trace** | Click any agent → see its execution timeline, tool calls, logs, cost |
| **Session linking** | Navigate agent ↔ session both directions |
| **Configurable retention** | Max entries / TTL in config.toml, automatic pruning |
| **Minimal runtime cost** | Write-on-termination model — no hot-path overhead |
| **Recoverable** | Filesystem JSON is source of truth; SQLite is a rebuildable query index |

## Storage Strategy: Dual Write

```
                    ┌──────────────────────┐
                    │   Agent terminates   │
                    │  (kill / complete /  │
                    │   fail / timeout)    │
                    └──────────┬───────────┘
                               │
              ┌────────────────┼────────────────┐
              ▼                ▼                ▼
    ┌──────────────────┐ ┌───────────┐ ┌─────────────────┐
    │ ~/.oxios/state/  │ │  SQLite   │ │  in-memory      │
    │ agents/<id>.json │ │  agents   │ │  HashMap        │
    │                  │ │  table    │ │  (removed)      │
    │ source of truth  │ │ query idx │ │                 │
    └──────────────────┘ └─────┬─────┘ └─────────────────┘
                               │
                               ▼
                      ┌─────────────────┐
                      │  AgentApi.list()│
                      │  reads from     │
                      │  SQLite ONLY    │
                      └────────┬────────┘
                               │
         ┌─────────────────────┼─────────────────────┐
         ▼                     ▼                     ▼
   GET /api/agents       GET /api/agents/{id}  GET /api/agents/stats
   ?q=refactor           loads from SQLite     aggregates via SQL
   ?status=failed        falls back to JSON
   ?tool=bash            if not in SQLite
   ?sort_by=cost&...
   ?page=1&per_page=50
```

**Why both?** Filesystem JSON is the Oxios way — human-readable, backup-friendly, grep-able. SQLite is the query engine — filtering/sorting/searching/aggregating JSON files at scale is absurd when SQLite already exists as a default dependency.

SQLite DB can be rebuilt from JSON at any time: `reindex_all()` scans `agents/*.json`, upserts into SQLite.

## Schema

```sql
-- Agents core metadata
CREATE TABLE IF NOT EXISTS agents (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    status          TEXT NOT NULL CHECK(status IN ('starting','running','idle','stopped','failed','completed')),
    created_at      TEXT NOT NULL,          -- ISO8601
    started_at      TEXT,
    completed_at    TEXT,
    session_id      TEXT,
    seed_id         TEXT,
    project_id      TEXT,
    model_id        TEXT NOT NULL DEFAULT '',
    error           TEXT,
    steps_completed INTEGER NOT NULL DEFAULT 0,
    steps_total     INTEGER,
    tokens_input    INTEGER NOT NULL DEFAULT 0,
    tokens_output   INTEGER NOT NULL DEFAULT 0,
    cost_usd        REAL NOT NULL DEFAULT 0.0,
    duration_secs   INTEGER                 -- derived: completed_at - started_at
);

-- Tool calls
CREATE TABLE IF NOT EXISTS agent_tool_calls (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id    TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    seq         INTEGER NOT NULL,
    tool_name   TEXT NOT NULL,
    input       TEXT NOT NULL DEFAULT '',
    output      TEXT NOT NULL DEFAULT '',
    duration_ms INTEGER NOT NULL DEFAULT 0,
    is_error    INTEGER NOT NULL DEFAULT 0,
    timestamp   TEXT,
    tool_call_id TEXT NOT NULL DEFAULT ''
);

-- Perf indexes
CREATE INDEX IF NOT EXISTS idx_agents_status_created ON agents(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_agents_session     ON agents(session_id);
CREATE INDEX IF NOT EXISTS idx_agents_project     ON agents(project_id);
CREATE INDEX IF NOT EXISTS idx_agents_seed        ON agents(seed_id);
CREATE INDEX IF NOT EXISTS idx_agents_model       ON agents(model_id);
CREATE INDEX IF NOT EXISTS idx_agents_cost        ON agents(cost_usd);
CREATE INDEX IF NOT EXISTS idx_agents_duration    ON agents(duration_secs);
CREATE INDEX IF NOT EXISTS idx_agents_name        ON agents(name);
CREATE INDEX IF NOT EXISTS idx_tool_calls_agent   ON agent_tool_calls(agent_id, seq);
CREATE INDEX IF NOT EXISTS idx_tool_calls_name    ON agent_tool_calls(tool_name);

-- Full-text search on tool outputs
CREATE VIRTUAL TABLE IF NOT EXISTS agent_tool_calls_fts USING fts5(
    tool_name,
    input,
    output,
    content='agent_tool_calls',
    content_rowid='id'
);
```

## Data Model

### Rust types (unchanged from current)

```rust
// types.rs — AgentInfo with session linkage added
pub struct AgentInfo {
    pub id: AgentId,
    pub name: String,
    pub status: AgentStatus,
    pub created_at: DateTime<Utc>,
    pub seed_id: Option<uuid::Uuid>,
    pub project_id: Option<uuid::Uuid>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub steps_completed: usize,
    pub steps_total: Option<usize>,
    pub tool_calls: Vec<ToolCallRecord>,
    pub tokens_input: u64,
    pub tokens_output: u64,
    pub cost_usd: f64,
    pub model_id: String,

    /// NEW: session linkage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}
```

No new structs needed. `AgentInfo` is the single representation — it serializes to JSON for filesystem and maps to SQL columns for SQLite.

## Persistence: 3 writes on termination

```rust
// supervisor.rs — BasicSupervisor
impl BasicSupervisor {
    async fn persist_terminated_agent(&self, id: AgentId) {
        let info = match self.agents.read().get(&id).cloned() {
            Some(info) => info,
            None => return,
        };

        // 1. Filesystem JSON (source of truth)
        if let Some(ref store) = self.state_store {
            let _ = store.save_json("agents", &id.to_string(), &info).await;
        }

        // 2. SQLite (query index)
        if let Some(ref db) = self.agent_log_db {
            let _ = db.upsert_agent(&info).await;
        }

        // 3. Prune (async, non-blocking)
        if let (Some(ref db), ref config) = (self.agent_log_db.as_ref(), &self.agent_log_config) {
            let db = db.clone();
            let cfg = config.clone();
            tokio::spawn(async move {
                if let Err(e) = db.prune(&cfg).await {
                    tracing::warn!(error = %e, "Agent log pruning failed");
                }
            });
        }

        // Remove from in-memory map
        self.agents.write().remove(&id);
    }
}
```

## Query Engine: AgentLogDb

```rust
// agent_log_db.rs — new module
pub struct AgentLogDb {
    conn: rusqlite::Connection,  // or pooled
}

impl AgentLogDb {
    /// Open or create the database.
    pub fn open(path: &Path) -> Result<Self>;

    /// Run schema migrations.
    pub fn migrate(&self) -> Result<MigrationReport>;

    /// Insert or update an agent record + its tool calls.
    pub fn upsert_agent(&self, info: &AgentInfo) -> Result<()>;

    /// Query with full filter/search/sort/paginate support.
    pub fn query(&self, filter: &AgentListFilter) -> Result<QueryResult>;

    /// Global stats (unfiltered).
    pub fn stats(&self) -> Result<AgentStats>;

    /// Load a single agent by ID.
    pub fn get(&self, id: &str) -> Result<Option<AgentInfo>>;

    /// Load an agent's tool calls.
    pub fn get_tool_calls(&self, agent_id: &str) -> Result<Vec<ToolCallRecord>>;

    /// Delete an agent + its tool calls.
    pub fn delete(&self, id: &str) -> Result<bool>;

    /// Prune old records per config.
    pub fn prune(&self, config: &AgentLogConfig) -> Result<usize>;

    /// Rebuild entire SQLite DB from filesystem JSON.
    pub fn reindex_all(&self, state_store: &StateStore) -> Result<RebuildReport>;
}
```

### Query builder

The `query()` method builds SQL dynamically from filter fields. No ORM — raw SQL with bound params.

```rust
pub struct AgentListFilter {
    pub q: Option<String>,           // full-text search
    pub search_field: SearchField,   // all | name | error | tool_name | tool_output
    pub status: Option<AgentStatus>,
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub seed_id: Option<String>,
    pub model_id: Option<String>,    // substring match
    pub tool: Option<String>,        // tool name substring match
    pub has_error: Option<bool>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub cost_min: Option<f64>,
    pub cost_max: Option<f64>,
    pub tokens_min: Option<u64>,
    pub tokens_max: Option<u64>,
    pub duration_min: Option<u64>,
    pub duration_max: Option<u64>,
    pub sort_by: SortBy,             // created_at | cost | duration | tokens | name
    pub sort_dir: SortDir,
    pub page: u32,
    pub per_page: u32,
}

pub enum SearchField { All, Name, Error, ToolName, ToolOutput }
pub enum SortBy { CreatedAt, Cost, Duration, Tokens, Name }
pub enum SortDir { Asc, Desc }

pub struct QueryResult {
    pub items: Vec<AgentInfo>,       // tool_calls stripped for listing
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
    pub stats: FilteredStats,
}

pub struct FilteredStats {
    pub total_cost_usd: f64,
    pub total_tokens: u64,
    pub avg_duration_secs: f64,
    pub count_running: u64,
    pub count_completed: u64,
    pub count_failed: u64,
}
```

### SQL generation example

When user requests: `?q=refactor&status=failed&date_from=2026-06-01&sort_by=cost&sort_dir=desc&page=2&per_page=20`

```sql
-- Count query
SELECT COUNT(*) FROM agents
WHERE status = 'failed'
  AND created_at >= '2026-06-01T00:00:00Z'
  AND (name LIKE '%refactor%' OR error LIKE '%refactor%');

-- Data query
SELECT * FROM agents
WHERE status = 'failed'
  AND created_at >= '2026-06-01T00:00:00Z'
  AND (name LIKE '%refactor%' OR error LIKE '%refactor%')
ORDER BY cost_usd DESC
LIMIT 20 OFFSET 20;

-- Stats for filtered set
SELECT SUM(cost_usd), SUM(tokens_input + tokens_output), AVG(duration_secs),
       SUM(CASE WHEN status='running' THEN 1 ELSE 0 END),
       SUM(CASE WHEN status IN ('completed','stopped') THEN 1 ELSE 0 END),
       SUM(CASE WHEN status='failed' THEN 1 ELSE 0 END)
FROM agents WHERE ...;
```

When `search_field = tool_output` or `tool_name`:

```sql
-- Join with tool_calls + FTS
SELECT DISTINCT a.* FROM agents a
JOIN agent_tool_calls tc ON a.id = tc.agent_id
WHERE a.status = 'failed'
  AND agent_tool_calls_fts MATCH 'refactor'
ORDER BY a.cost_usd DESC
LIMIT 20 OFFSET 20;
```

## API Changes

### `GET /api/agents`

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `q` | string | — | Full-text search |
| `search_field` | `all` \| `name` \| `error` \| `tool_name` \| `tool_output` | `all` | Fields to search |
| `status` | `all` \| `running` \| `completed` \| `failed` \| `stopped` | `all` | Status filter |
| `session_id` | string | — | Filter by session |
| `project_id` | string | — | Filter by project |
| `seed_id` | string | — | Filter by seed |
| `model_id` | string | — | Substring match on model |
| `tool` | string | — | Agents that used this tool |
| `has_error` | bool | — | `true` = failed, `false` = success |
| `date_from` | ISO8601 | — | `created_at >=` |
| `date_to` | ISO8601 | — | `created_at <=` |
| `cost_min` | f64 | — | `cost_usd >=` |
| `cost_max` | f64 | — | `cost_usd <=` |
| `tokens_min` | u64 | — | `tokens_total >=` |
| `tokens_max` | u64 | — | `tokens_total <=` |
| `duration_min` | u64 | — | Seconds `>=` |
| `duration_max` | u64 | — | Seconds `<=` |
| `sort_by` | `created_at` \| `cost` \| `duration` \| `tokens` \| `name` | `created_at` | Sort field |
| `sort_dir` | `asc` \| `desc` | `desc` | Sort direction |
| `page` | u32 | 1 | Page number |
| `per_page` | u32 | 50 | Items per page (max 200) |

Response:
```json
{
  "items": [ /* AgentInfo without tool_calls */ ],
  "total": 1432,
  "page": 1,
  "per_page": 50,
  "total_pages": 29,
  "stats": {
    "total_cost_usd": 42.73,
    "total_tokens": 2850000,
    "avg_duration_secs": 12.3,
    "count_running": 2,
    "count_completed": 1400,
    "count_failed": 30
  }
}
```

> `stats` always reflects the **filtered** set. For global stats, use `GET /api/agents/stats`.

### `GET /api/agents/stats`

```json
{
  "total_agents": 1432,
  "running": 2,
  "completed": 1400,
  "failed": 30,
  "total_cost_usd": 42.73,
  "total_tokens": 2850000,
  "total_duration_secs": 18000,
  "avg_duration_secs": 12.6,
  "avg_cost_usd": 0.03,
  "total_sessions": 891,
  "oldest_agent_at": "2026-01-15T09:30:00Z",
  "newest_agent_at": "2026-06-13T14:22:00Z"
}
```

### `GET /api/agents/{id}` + `GET /api/agents/{id}/trace` + `GET /api/agents/{id}/logs`

Existing endpoints. Load from SQLite first; if not found (e.g. agent still in memory, not yet persisted), fall back to in-memory `Supervisor::list()`. If still not found, fall back to filesystem JSON.

### `POST /api/agents/reindex` — Admin-only

Rebuilds the SQLite agent index from filesystem JSON. Used after DB corruption, migration, or manual JSON edits.

### `DELETE /api/agents/prune` — Manual cleanup

Optional `?before=<ISO8601>`. Falls back to config.toml values.

## Config (`config.toml`)

```toml
[agent_log]
# Path to the SQLite database file (relative to ~/.oxios/)
db_path = "state/agent_log.db"

# Maximum agent records in SQLite + filesystem (0 = unlimited)
max_entries = 10000

# TTL in hours (0 = unlimited)
ttl_hours = 720  # 30 days

# Max tool_calls to store per agent (0 = unlimited)
# Older calls truncated from both JSON and SQLite
max_tool_calls_per_agent = 500

# How many agents to prune per cycle (prevents long locks)
prune_batch_size = 100
```

### Pruning behavior

```
Pruning runs async after each agent save:
  1. DELETE FROM agents WHERE created_at < now - ttl_hours
     → also deletes from filesystem
  2. If remaining > max_entries:
     DELETE oldest N agents
     → also deletes from filesystem
  3. Bound by prune_batch_size per cycle
  4. VACUUM every 100 prune cycles (reclaim disk)
```

## Frontend

### Agents List Page (`/agents`)

```
┌──────────────────────────────────────────────────────────────────┐
│  Agents                                    2 running · $42.73   │
│                                                                  │
│  ┌───────────────────────────────────────────────────────────┐   │
│  │ 🔍 refactor...               [status▼] [date▼] [model▼]  │   │
│  │                                                           │   │
│  │ [failed ✕] [bash ✕] [last 7d ✕] [>$0.10 ✕]  clear all   │   │
│  └───────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─[All 1432]──[Running 2]──[Completed 1400]──[Failed 30]──┐   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
│  Sort: [created_at ▼]                   1,432 filtered          │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 🤖 Fix build error      ✘ failed   5m ago   12s   $0.05 │   │
│  │    Session: "빌드 오류" · model: claude-sonnet-4          │   │
│  ├──────────────────────────────────────────────────────────┤   │
│  │ 🤖 Review PR #42        ✔ ok       5m ago   45s   $0.12 │   │
│  │    Session: "코드 리뷰" · 12 tool calls                  │   │
│  ├──────────────────────────────────────────────────────────┤   │
│  │ 🤖 Write tests          ● running  now      —     $0.02 │   │
│  │    Session: "테스트 작성" · 3 steps                       │   │
│  ├──────────────────────────────────────────────────────────┤   │
│  │ ...                                                      │   │
│  ├──────────────────────────────────────────────────────────┤   │
│  │              ← 1  2  3  ...  29 →     50 per page        │   │
│  └──────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────┘
```

| Element | Behavior |
|---------|----------|
| Search bar | 300ms debounce → `?q=`. Searches name + error; if `search_field=all`, also FTS5 on tool outputs |
| Filter chips | Active filters as removable tags below search bar |
| "···" dropdown | Less-common: model, tool, cost range, tokens range, duration range |
| Date picker | Quick: Today, Last 7d, Last 30d, Custom |
| Status tabs | Sets `status=` param. Badge counts update live |
| Sort dropdown | Changes `sort_by` + `sort_dir` |
| Row click | → `/agents/$agentId` |
| URL state | All filters in query string. Back/forward works. Copy-paste reproduces view. |

### Agent Detail Page (`/agents/$agentId`)

- Works for both running and historical agents
- Clear banner: green ✓ for completed, red ✗ for failed, blue ● for running
- Duration: "Ran for 2m 34s"
- Session link → `/sessions/$sessionId`
- Trace tab: loads tool_calls from SQLite
- Logs tab: synthesized from lifecycle events

### Session Detail Page (`/sessions/$sessionId`)

New section:

```
┌─ Agents (3) ────────────────────────────────────────────────┐
│ 🤖 Review PR #42        ✔ ok      45s    $0.12   View →    │
│ 🤖 Fix lint errors      ✔ ok      12s    $0.03   View →    │
│ 🤖 Run tests            ✘ failed   8s    $0.01   View →    │
└──────────────────────────────────────────────────────────────┘
```

### Dashboard

Existing "Active Agents" card stays. New: "Agent Activity" sparkline (agent count per hour, last 24h) from SQLite aggregation.

## AgentApi Changes

```rust
// agent_api.rs — updated
impl AgentApi {
    /// Unified query: in-memory running agents + SQLite historical agents.
    /// Running agents always appear first (they're not in SQLite until terminated).
    pub async fn query(&self, filter: &AgentListFilter) -> Result<QueryResult> {
        // 1. Get running agents from supervisor (in-memory)
        let running = self.supervisor.list().await?
            .into_iter()
            .filter(|a| matches!(a.status, AgentStatus::Running | AgentStatus::Starting | AgentStatus::Idle));

        // 2. Query historical agents from SQLite
        let mut sqlite_result = self.agent_db.query(filter).await?;

        // 3. Prepend running agents that match the filter
        for agent in running {
            if filter.matches_summary(&agent) {
                sqlite_result.items.insert(0, agent);
                sqlite_result.total += 1;
            }
        }

        // 4. Re-paginate to account for prepended running agents
        Ok(sqlite_result)
    }

    /// Global stats (unfiltered).
    pub async fn stats(&self) -> Result<AgentStats> {
        self.agent_db.stats().await
    }
}
```

## Recovery

```rust
impl AgentLogDb {
    /// Rebuild entire SQLite DB from filesystem JSON.
    /// Safe to run anytime — idempotent (UPSERT semantics).
    pub fn reindex_all(&self, state_store: &StateStore) -> Result<RebuildReport> {
        let json_files = state_store.list_category("agents").await?;
        let mut report = RebuildReport::default();

        for name in &json_files {
            if let Some(info) = state_store.load_json::<AgentInfo>("agents", name).await? {
                self.upsert_agent(&info).await?;
                report.reindexed += 1;
            }
        }

        // Clean up SQLite entries with no matching JSON file
        let json_ids: HashSet<String> = json_files.into_iter().collect();
        report.orphaned = self.delete_orphaned(&json_ids).await?;

        report
    }
}
```

## Implementation Plan

### Phase 1: Persistence (kernel)

1. Create `crates/oxios-kernel/src/agent_log_db.rs` — `AgentLogDb` with schema, upsert, query, stats, prune, reindex
2. Add `AgentLogDb` to `BasicSupervisor` (optional, behind `agent_log` config)
3. Modify `BasicSupervisor` termination paths: save JSON + upsert SQLite + spawn prune
4. Add `session_id` to `AgentInfo`, populated by `AgentLifecycleManager`
5. Add `AgentLogConfig` to `OxiosConfig`
6. Add `[agent_log]` to `share/default-config.toml`
7. Unit tests: upsert, query with all filter combos, prune, reindex

### Phase 2: API

1. Add `AgentListFilter`, `SearchField`, `SortBy`, `SortDir` to `types.rs`
2. Implement `AgentApi::query(filter)` — merge running + SQLite
3. Add `AgentApi::stats()` — global aggregates
4. Add all query params to `handle_agents_list`
5. `handle_agent_get` → SQLite first, memory fallback, JSON fallback
6. `handle_agent_trace` → load tool_calls from SQLite
7. Add `GET /api/agents/stats`
8. Add `POST /api/agents/reindex` (admin)
9. Add `DELETE /api/agents/prune`
10. Integration tests for each endpoint with filter combinations

### Phase 3: Frontend

1. Update `types/agent.ts` — `AgentListFilter`, `AgentStats`
2. Build `routes/agents/index.tsx` — search, filter chips, tabs, pagination, stats bar
3. Build `components/agents/agent-filter-bar.tsx`
4. Build `components/agents/agent-stats-bar.tsx`
5. Update `routes/agents/$agentId.tsx` — session link, historical agent support
6. Update `routes/sessions/$sessionId.tsx` — spawned agents section
7. Update `components/dashboard/agents-activity-card.tsx` — 24h sparkline
8. e2e: filter combos, search, pagination, empty states, deep-linking

## Pitfalls

| Risk | Mitigation |
|------|-----------|
| SQLite ↔ JSON drift | JSON is source of truth. `reindex_all()` fixes divergence. |
| Schema migration | Simple `ALTER TABLE` migrations in `AgentLogDb::migrate()`. Additive only. |
| Large tool_calls | Cap at `max_tool_calls_per_agent`. Truncation happens at write time for both JSON and SQLite. |
| Running agents not in SQLite | Expected. `AgentApi::query()` prepends them from in-memory. They enter SQLite on termination. |
| Old agents lack session_id | Show "—". Acceptable for pre-feature agents. |
| Pruning while querying | SQLite WAL mode handles concurrent reads. Pruning runs in `tokio::spawn`, non-blocking. |
| DB corruption | Delete the DB file, call `POST /api/agents/reindex` → rebuilt from JSON. |
| `rusqlite` not compiled in | Feature-gated: if `sqlite-memory` feature is off, agents fall back to filesystem-only scan mode. Degraded but functional. |

## Alternatives Considered

| Option | Verdict |
|--------|---------|
| Filesystem only | Rejected. O(n) scan for every list/search/sort is unacceptable with filtering requirements. |
| SQLite only | Rejected. Loses Oxios filesystem-native identity. No recovery if DB corrupts. |
| **Dual write (chosen)** | Filesystem JSON = source of truth. SQLite = query index. Best of both. |
| Tantivy / Meilisearch | Rejected. Overkill. `rusqlite` + FTS5 already compiled in. |
