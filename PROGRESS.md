# Progress

## Status
In Progress

## Tasks

### ✅ Completed: Full Static API Analysis
- Analyzed all 31 frontend route files, hooks, types, and layout components
- Compared every API call against backend handler return shapes
- Wrote comprehensive report to `/tmp/static-analysis-report.md`

### Findings Summary (30 issues total)
- 🔴 CRITICAL: 7 — runtime crashes (missing endpoints, shape mismatches on core pages)
- 🟠 HIGH: 10 — wrong/empty data displays
- 🟡 MEDIUM: 8 — partial data loss, minor display issues
- 🔵 LOW: 5 — code quality

### Key Critical Issues
1. **Dashboard** (`/`) — `agents_running`, `agents_total`, `spaces_active`, `uptime_ms` all missing from backend response
2. **Agent Detail** (`/agents/:id`) — No `GET /api/agents/:id` endpoint; restart endpoint doesn't exist
3. **Workspace** (`/workspace`) — Wrong URL (`/api/workspace` vs `/api/workspace/tree`); expects tree structure, gets flat array
4. **Sessions** — Frontend expects `agent_id`, backend returns `user_id`; messages endpoint doesn't exist
5. **Seeds** — Frontend expects `name`/`phase`, backend returns `goal`/`constraints_count`
6. **Programs** — Toggle uses wrong endpoint; install uses wrong URL and body
7. **Events SSE** — Connects to `/api/events/stream`, backend serves at `/api/events`

## Files Changed
- `/tmp/static-analysis-report.md` — Full analysis report (written)

## Notes
- Previously fixed routes (host-tools, programs, personas, skills, budget, cron-jobs, resources, git, memory, security, scheduler, events, approvals) had array-vs-object wrapping fixes
- This audit covers ALL remaining mismatches including missing endpoints, wrong field names, wrong URLs, and type shape differences
