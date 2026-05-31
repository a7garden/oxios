# Phase 1: TypeScript Error Classification

**Date:** 2026-05-31
**Total errors:** 59 across 27 files

## Group A — Unused Imports (22 errors, low risk, mechanical)

| File | Symbol | Fix |
|------|--------|-----|
| `components/chat/chat-input.tsx` | `Loader2` | Remove from import |
| `components/engine/api-key-input.tsx` | `useTranslation` (imported but not destructured) | Add `const { t } = useTranslation()` inside component |
| `components/engine/model-select.tsx` | `t` parameter in `formatCost` | Prefix with `_` |
| `components/engine/routing-section.tsx` | `Route` | Remove import line |
| `components/engine/routing-section.tsx` | `import type { RoutingConfig }` | Remove import line |
| `components/engine/routing-section.tsx` | `cn` | Remove from import |
| `components/knowledge/file-tree.tsx` | `t` | Remove destructured `t` |
| `components/mcp/server-list.tsx` | `{ Power, RefreshCw, Trash2 }` | Remove import line |
| `components/mcp/server-list.tsx` | `Badge` | Remove import |
| `components/mcp/server-list.tsx` | `Button` | Remove import |
| `components/skills/marketplace-detail.tsx` | `cn` | Remove from import |
| `components/skills/update-badge.tsx` | `useTranslation` | Remove import |
| `components/workspace/file-viewer.tsx` | `highlightActiveLine` | Remove from import |
| `routes/projects/$projectId.tsx` | `EmptyState` | Remove import |
| `routes/projects/$projectId.tsx` | `deleteProject` | Check usage; remove if unused |
| `routes/projects/index.tsx` | `RefreshCw` | Remove from import |
| `routes/projects/index.tsx` | `useDeleteProject` | Remove import |
| `stores/chat.ts` | `AiDetectionState` | Remove interface |
| `stores/chat.ts` | `err` in error handler | Prefix with `_` |
| `components/shared/column-filter.tsx` | `T` generic | Remove unused generic param |
| `__tests__/components/shared/loading.test.tsx` | `screen` | Remove from import |

## Group B — Actual Type Bugs (15 errors, medium risk)

| File | Error | Fix |
|------|-------|-----|
| `stores/chat.ts(349)` | `project_ids` doesn't exist → should be `project_id` | Fix property name |
| `stores/chat.ts(352)` | `string \| undefined === true` comparison | Remove `=== true` branch |
| `stores/chat.ts(376)` | `string \| undefined` not assignable to `string` (`id`) | Non-null assertion (guarded by index check) |
| `routes/settings.tsx(639-640)` | `LoadingCards` / `ErrorState` not found | Add missing imports |
| `routes/settings.tsx(647,649)` | `tKeys.title` / `tKeys.subtitle` missing | Add to tKeys object |
| `components/project/create-project-dialog.tsx(67)` | `{ variant: 'destructive' }` not assignable to `ToastVariant` | Change to `'destructive'` as 2nd arg |
| `components/project/delete-project-dialog.tsx(33)` | Same ToastVariant issue | Same fix |
| `components/project/edit-project-dialog.tsx(71)` | Same ToastVariant issue | Same fix |
| `components/layout/language-selector.tsx(25)` | `{ code, label }[]` ≠ `{ label, value }[]` | Rename `code` → `value` |
| `components/shared/data-table.tsx(227)` | `string \| number \| symbol` ≠ `string` | Cast with `String()` |
| `components/skills/skill-detail.tsx(49-50)` | `sd` possibly undefined | Add fallback with default status |
| `routes/chat.tsx(186,197)` | `Project` type not found | Add to type import |
| `components/engine/api-key-input.tsx(77-107)` | `t` not defined (6 uses) | Add `const { t } = useTranslation()` |

## Group C — Test Infrastructure (22 errors, separate concern)

| File | Error | Fix |
|------|-------|-----|
| 9 test files + setup.ts | `vi` / `beforeAll` / `afterEach` / `afterAll` not found | Add vitest globals types to tsconfig |
| `memory-card.test.tsx(103)` | `Element` has no `click` | Cast to `HTMLElement` |
| `evaluation-card.test.tsx(47)` | Object possibly undefined | Add null guard |
