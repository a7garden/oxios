# Phase 2: Group A — Unused Imports Fix Log

**Date:** 2026-05-31
**Errors fixed:** 22

## Changes

| # | File | Change | Verified |
|---|------|--------|----------|
| 1 | `components/chat/chat-input.tsx` | Removed `Loader2` from lucide import | ✅ |
| 2 | `components/engine/api-key-input.tsx` | `useTranslation` was imported but `t` was never destructured — added `const { t } = useTranslation()` inside the component | ✅ |
| 3 | `components/engine/model-select.tsx` | Prefixed unused `t` param in `formatCost()` with underscore: `_t` | ✅ |
| 4 | `components/engine/routing-section.tsx` | Removed unused `Route` import from `@tanstack/react-router` | ✅ |
| 5 | `components/engine/routing-section.tsx` | Removed unused `Card, CardContent, CardHeader, CardTitle` import | ✅ |
| 6 | `components/engine/routing-section.tsx` | Removed unused `cn` import from `@/lib/utils` | ✅ |
| 7 | `components/knowledge/file-tree.tsx` | Removed unused destructured `t` from `useTranslation()` | ✅ |
| 8 | `components/mcp/server-list.tsx` | Removed unused `{ Power, RefreshCw, Trash2 }` from lucide import | ✅ |
| 9 | `components/mcp/server-list.tsx` | Removed unused `Badge` import | ✅ |
| 10 | `components/mcp/server-list.tsx` | Removed unused `Button` import | ✅ |
| 11 | `components/skills/marketplace-detail.tsx` | Removed unused `cn` import from `@/lib/utils` | ✅ |
| 12 | `components/skills/update-badge.tsx` | Removed unused `useTranslation` import (component has no translated text) | ✅ |
| 13 | `components/workspace/file-viewer.tsx` | Removed unused `highlightActiveLine` from `@codemirror/view` import | ✅ |
| 14 | `routes/projects/$projectId.tsx` | Removed unused `EmptyState` import | ✅ |
| 15 | `routes/projects/$projectId.tsx` | Removed unused `useDeleteProject` import + `deleteProject` variable | ✅ |
| 16 | `routes/projects/index.tsx` | Removed unused `RefreshCw` from lucide import | ✅ |
| 17 | `routes/projects/index.tsx` | Removed unused `useDeleteProject` import | ✅ |
| 18 | `stores/chat.ts` | Removed unused `AiDetectionState` interface (dead stub) | ✅ |
| 19 | `stores/chat.ts` | Removed unused `err` variable in error handler | ✅ |
| 20 | `components/shared/column-filter.tsx` | Removed unused generic type parameter `<T>` from `ColumnFilterProps` and `ColumnFilter` | ✅ |
| 21 | `__tests__/components/shared/loading.test.tsx` | Removed unused `screen` import | ✅ |
| 22 | `components/engine/routing-section.tsx` | Kept `import type { RoutingConfig }` — it IS used in `Partial<RoutingConfig>` | ✅ |

## Side-effect imports preserved

- All CSS imports untouched
- Type-only imports preserved where used in type annotations
- `Server` lucide icon kept in `server-list.tsx` (used in EmptyState)

## Verification

```bash
cd surface/oxios-web/web && npx tsc --noEmit
# 0 errors
```
