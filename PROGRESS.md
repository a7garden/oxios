# Progress

## Status
In Progress

## Tasks

- [x] Cross-crate dependency analysis — analyzed all 9 workspace crates, traced imports, wrote report
- [x] Kernel directory module analysis — analyzed all 9 subdirectories under `crates/oxios-kernel/src/`, 72 .rs files, ~24K LOC

## Files Changed

- `analysis/cross-crate-deps.md` — Full cross-crate dependency report
- `analysis/dir-modules.md` — Detailed kernel directory module analysis with extraction candidates

## Notes

- CLI and Telegram channels are well-isolated (gateway-only dependency)
- Web channel is the most coupled (imports from 4 workspace crates directly)
- No circular dependencies in the workspace
- Leaf crates: ouroboros, markdown (no workspace deps)
- 5 modules are strong extraction candidates: workers (0 deps), access_manager (1 dep), program (1 dep), mcp (1 dep), capability (2 deps)
- memory/ is the largest module (6.8K LOC) with clean internal architecture — extractable with trait abstraction
- tools/ and kernel_handle/ are not extractable by design (facade and hands of the kernel)

## Frontend-Backend API Type Mismatch Audit (2026-05-23)

- [x] Audited all 13 frontend routes against their backend handlers
- [x] Report written to `/tmp/api-mismatch-report.md`

### Summary of findings:
- **12 of 13 routes have mismatches** (only agent-groups is clean)
- **6 routes**: frontend expects raw array but backend returns paginated `{ items, total, page, limit }`
- **4 routes**: backend wraps array in object (`{ jobs }`, `{ entries }`, `{ tags }`) but frontend expects raw array
- **1 route**: frontend expects `{ items }` wrapper but backend returns raw array (approvals)
- **2 missing endpoints**: `GET /api/budget` (list all) and `GET /api/scheduler` (combined)
- **2 wrong URLs**: `/api/security/audit` should be `/api/audit`, `/api/security/permissions` doesn't exist
- **1 protocol mismatch**: `/api/events` is SSE but frontend tries REST GET
- **1 single-vs-array**: `/api/resources` returns single snapshot but frontend expects array for chart
- **Multiple field name mismatches**: `active`/`enabled` (personas), `command`/`goal` (cron create), `agent_id`/`subject` (approvals)
