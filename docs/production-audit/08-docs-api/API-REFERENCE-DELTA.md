# API Reference Delta — Staleness Audit

**Date:** 2026-05-31  
**Source:** `docs/api-reference.md` (3028 lines, dated 2026-05-17, API Version 0.1.2)  
**Compared against:** `surface/oxios-web/src/routes/mod.rs` (live route registration)

---

## Critical Issues

### 1. Section "9. Programs" — Entirely Removed from Code ❌

The entire Programs section (lines 972–1208, 7 endpoints, 7 summary-table entries) documents routes that **no longer exist** in the codebase. Per RFC-009, Programs were unified into the Skills system. The `/api/programs/*` routes are gone.

| Route | Status |
|-------|--------|
| `GET /api/programs` | ❌ Not in code |
| `POST /api/programs` | ❌ Not in code |
| `GET /api/programs/{name}` | ❌ Not in code |
| `DELETE /api/programs/{name}` | ❌ Not in code |
| `POST /api/programs/{name}/enable` | ❌ Not in code |
| `POST /api/programs/{name}/disable` | ❌ Not in code |
| `GET /api/programs/{name}/host-requirements` | ❌ Not in code |

**Action:** Remove Section 9 entirely. Update Table of Contents and endpoint summary table.

### 2. Section "22. Host Tools" — Route Missing from Code ❌

`GET /api/host-tools` (line 2813) is documented but **not registered** in `routes/mod.rs`. No matching route handler exists.

**Action:** Remove or mark as planned/removed.

---

## Missing Sections — Entire Route Groups Undocumented

These are active routes in the codebase with **no documentation** in the API reference:

### 3. Engine Management (10 routes) ⚠️

| Route | Method |
|-------|--------|
| `/api/engine/providers` | GET |
| `/api/engine/models` | GET |
| `/api/engine/config` | GET |
| `/api/engine/model` | PUT |
| `/api/engine/api-key` | PUT |
| `/api/engine/provider-options` | PUT |
| `/api/engine/validate-key` | POST |
| `/api/engine/routing` | PUT |
| `/api/engine/routing/stats` | GET |
| `/api/engine/routing/fallbacks` | GET |

**Action:** Add new section (e.g., "Engine Management") documenting all 10 endpoints.

### 4. Knowledge Base (31 routes) ⚠️

The entire Knowledge UI backend is undocumented. Routes include:

| Route Group | Routes |
|-------------|--------|
| `/api/knowledge/tree` | GET |
| `/api/knowledge/file/{*path}` | GET/PUT/DELETE |
| `/api/knowledge/file/{*path}/history` | GET |
| `/api/knowledge/file/{*path}/restore` | POST |
| `/api/knowledge/search` | POST |
| `/api/knowledge/backlinks` | GET |
| `/api/knowledge/graph` | GET |
| `/api/knowledge/copilot` | POST |
| `/api/knowledge/chat/*` | 7 routes (append, delete, messages, move) |
| `/api/knowledge/checklist/*` | 4 routes (add, complete, items, remove) |
| `/api/knowledge/habits` | GET + sub-routes |
| `/api/knowledge/journal/*` | 3 routes |
| `/api/knowledge/convert/html` | POST |
| `/api/knowledge/stats/*` | 2 routes |
| `/api/knowledge/config` | GET/PUT |
| `/api/knowledge/emoji` | GET |
| `/api/knowledge/worker/*` | 2 routes |

**Action:** Add comprehensive Knowledge API section. This is the largest gap.

### 5. Projects (8 routes) ⚠️

| Route | Method |
|-------|--------|
| `/api/projects` | GET, POST |
| `/api/projects/{id}` | GET, PUT, DELETE |
| `/api/projects/{id}/memories` | GET, POST |
| `/api/projects/{id}/memories/{memoryId}` | DELETE |

**Action:** Add Projects section.

### 6. MCP Servers (5 routes) ⚠️

| Route | Method |
|-------|--------|
| `/api/mcp/servers` | GET |
| `/api/mcp/servers/{name}` | GET |
| `/api/mcp/servers/{name}/refresh` | POST |
| `/api/mcp/servers/{name}/toggle` | POST |
| `/api/mcp/tools` | GET |

**Action:** Add MCP section.

### 7. A2A (Agent-to-Agent) (4 routes) ⚠️

| Route | Method |
|-------|--------|
| `/api/a2a/agents` | GET |
| `/api/a2a/agents/{id}` | GET |
| `/api/a2a/messages` | POST |
| `/api/a2a/topology` | GET |

**Action:** Add A2A section.

### 8. Marketplace (4 routes) ⚠️

| Route | Method |
|-------|--------|
| `/api/marketplace/search` | GET |
| `/api/marketplace/updates` | GET |
| `/api/marketplace/skills/{slug}` | GET |
| `/api/marketplace/skills/{slug}/install` | POST |

**Action:** Add Marketplace section.

---

## Partially Documented Sections

### 9. Budget — Missing `GET /api/budget` (list all)

Docs only have `/api/budget/{agent_id}` routes. Code also has `GET /api/budget` (list all budgets).

**Action:** Add listing endpoint to Budget section.

### 10. Skills — Missing Endpoints

Docs have: `GET /api/skills`, `GET /api/skills/{name}`, `POST /api/skills`, `DELETE /api/skills/{name}`.

Code also has:
- `POST /api/skills/{name}/enable`
- `POST /api/skills/{name}/disable`
- `GET /api/skills/{name}/content`

**Action:** Add the 3 missing skill endpoints.

### 11. Chat — Missing Ticket Endpoint

Code has `POST /api/chat/ticket` which is undocumented.

**Action:** Add ticket endpoint.

### 12. Memory — Missing Endpoints

Code has additional memory endpoints not documented:
- `GET /api/memory/stats`
- `GET /api/memory/dream/reports`
- `GET /api/memory/dream/status`
- `PUT /api/memory/{id}/pin`
- `DELETE /api/memory/{id}`

**Action:** Add missing memory endpoints.

### 13. Sessions — Missing Endpoint

Code has `POST /api/sessions/prune` and `GET /api/sessions/{id}/tool-calls` which are undocumented.

**Action:** Add missing session endpoints.

### 14. Security/Permissions — Separate Route

Code has `GET /api/security/permissions` which is different from the documented `GET /api/permissions/{agent}`.

**Action:** Verify if both routes exist and document accordingly.

---

## Summary

| Category | Count |
|----------|-------|
| **Removed routes still documented** | 8 (Programs + host-tools) |
| **Undocumented route groups** | 6 (Engine, Knowledge, Projects, MCP, A2A, Marketplace) |
| **Missing individual endpoints** | ~15 (Budget list, Skills enable/disable/content, Chat ticket, Memory stats/dream/pin/delete, Sessions prune/tool-calls) |
| **Total documentation gap** | ~62 routes undocumented, 8 routes ghost-documented |
| **Doc is ~35% complete** relative to actual routes |

The API reference was last updated 2026-05-17 but many new route groups were added since then. The document is well-structured and accurate for what it covers — it just needs significant expansion.
