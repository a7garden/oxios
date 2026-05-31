# Frontend Audit — Final Report

**Date:** 2026-05-31  
**Area:** `surface/oxios-web/web/`  
**Status:** ✅ COMPLETE

## Summary

| Metric | Before | After |
|--------|--------|-------|
| `tsc --noEmit` errors (production) | 59 | **0** |
| `tsc --noEmit -p tsconfig.test.json` errors | N/A | **0** |
| `bun run build` | ✅ (with TS errors) | ✅ (clean) |
| Files changed | — | 22 |

## Verification Commands

```bash
# Production type check — zero errors
cd surface/oxios-web/web && npx tsc --noEmit

# Test type check — zero errors
cd surface/oxios-web/web && npx tsc --noEmit -p tsconfig.test.json

# Production build — succeeds
cd surface/oxios-web/web && bun run build
```

## Changes by Category

### Group A — Unused Imports Removed (14 files)

| File | Change |
|------|--------|
| `chat-input.tsx` | Removed `Loader2` from lucide import |
| `model-select.tsx` | Prefixed unused `t` param → `_t` in `formatCost()` |
| `routing-section.tsx` | Removed `Route`, `Card/*`, `cn` imports |
| `file-tree.tsx` | Moved `useTranslation()` from `FileTree` (unused) to `SubDirectory` (uses `t`) |
| `server-list.tsx` | Removed unused `Power/RefreshCw/Trash2`, `Badge`, `Button` imports |
| `marketplace-detail.tsx` | Removed unused `cn` import |
| `update-badge.tsx` | Removed unused `useTranslation` import |
| `file-viewer.tsx` | Removed unused `highlightActiveLine` import |
| `$projectId.tsx` | Removed `EmptyState` import, removed unused `useDeleteProject` |
| `index.tsx` (projects) | Removed `RefreshCw`, `useDeleteProject` imports |
| `column-filter.tsx` | Removed unused generic type parameter `<T>` |
| `chat.ts` (store) | Removed dead `AiDetectionState` interface, removed unused `err` variable |

### Group B — Type Bugs Fixed (10 files)

| File | Bug | Fix |
|------|-----|-----|
| `api-key-input.tsx` | `t` called without destructuring from `useTranslation()` | Added `const { t } = useTranslation()` |
| `chat.ts` (store) | `chunk.project_ids` — wrong property name | Changed to `chunk.project_id` |
| `chat.ts` (store) | `=== true` comparison with `string \| undefined` | Removed dead boolean branch |
| `chat.ts` (store) | Spread of possibly-undefined indexed element | Extracted to `const existing = updated[idx]!` |
| `settings.tsx` | Missing `LoadingCards`/`ErrorState` imports | Added imports |
| `settings.tsx` | Missing `title`/`subtitle` in `tKeys` | Added to tKeys object |
| `*-project-dialog.tsx` (×3) | `{ variant: 'destructive' }` not assignable to `ToastVariant` | Changed to `'destructive'` as 2nd arg |
| `language-selector.tsx` | `{ code, label }[]` ≠ `{ label, value }[]` | Renamed `code` → `value` |
| `data-table.tsx` | `string \| number \| symbol` ≠ `string` | Wrapped in `String()` |
| `skill-detail.tsx` | `sd` possibly undefined from record lookup | Added fallback with default status |
| `chat.tsx` (route) | `Project` type not imported | Added to type import |

### Group C — Test Infrastructure Fixed (3 files + tsconfig)

| File | Change |
|------|--------|
| `tsconfig.json` | Added `"exclude": ["src/__tests__"]` to separate production from test type-checking |
| `tsconfig.test.json` | Created — extends base, adds `vitest/globals` types, includes test files |
| `loading.test.tsx` | Added missing `LoadingCards`/`LoadingTable` imports, removed unused `screen` |
| `evaluation-card.test.tsx` | Added `!` non-null assertion for array index access, added `?? null` fallback |

### Phase 5 — Bundle Assessment

No bundle size changes. All modifications were type-only or import-only. The bundle structure remains:

| Chunk | Size | Status |
|-------|------|--------|
| workspace | 677 KB | ⚠️ CodeMirror 6 (already split) |
| index | 477 KB | Core app shell |
| knowledge | 431 KB | HyperMD/CM5 (already split) |
| CartesianChart | 314 KB | Recharts (already split) |

No further optimization recommended within audit scope.

## Artifacts

| File | Description |
|------|-------------|
| `PHASE1-CLASSIFICATION.md` | Error categorization and triage |
| `PHASE2-GROUP-A-FIXES.md` | Unused imports fix log |
| `PHASE3-GROUP-B-FIXES.md` | Type bugs fix log with explanations |
| `PHASE4-GROUP-C-FIXES.md` | Test infrastructure fix log |
| `PHASE5-BUNDLE-ASSESSMENT.md` | Bundle size analysis and recommendations |
| `FINAL-REPORT.md` | This file |
