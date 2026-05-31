# Phase 3: Group B — Type Bug Fix Log

**Date:** 2026-05-31
**Errors fixed:** 15

## Changes

### 1. `stores/chat.ts(349)` — Wrong property name `project_ids` → `project_id`

**Error:** `Property 'project_ids' does not exist on type 'StreamChunk'. Did you mean 'project_id'?`

**Root cause:** The `StreamChunk` type defines `project_id?: string`, but the code used `chunk.project_ids`.

**Fix:** Changed `chunk.project_ids` to `chunk.project_id`.

```diff
- const vid = chunk.project_ids ?? null
+ const vid = chunk.project_id ?? null
```

---

### 2. `stores/chat.ts(352)` — Unintentional comparison with `boolean`

**Error:** `This comparison appears to be unintentional because the types 'string | undefined' and 'boolean' have no overlap.`

**Root cause:** `StreamChunk.evaluation_passed` is typed as `string | undefined` (the backend sends it as a string). The `=== true` comparison would never match.

**Fix:** Removed the boolean comparison. The string check `=== 'true'` is sufficient.

```diff
- const evaluationPassed = chunk.evaluation_passed === 'true' || chunk.evaluation_passed === true
+ const evaluationPassed = chunk.evaluation_passed === 'true'
```

---

### 3. `stores/chat.ts(376)` — `string | undefined` not assignable to `string` (`id`)

**Error:** `Types of property 'id' are incompatible. Type 'string | undefined' is not assignable to type 'string'.`

**Root cause:** With `noUncheckedIndexedAccess: true`, `updated[idx]` returns `ChatMessage | undefined`. The spread `...updated[idx]` carries the `undefined` through to the `id` property.

**Fix:** Extract to a non-null asserted variable before spreading.

```diff
  if (lastAssistantIdx >= 0) {
    const idx = updated.length - 1 - lastAssistantIdx
-   updated[idx] = { ...updated[idx], metadata: { ... } }
+   const existing = updated[idx]!
+   updated[idx] = { ...existing, metadata: { ... } }
  }
```

This is safe because the `findIndex` + length calculation guarantees the index is valid.

---

### 4. `routes/settings.tsx(639-640)` — Missing `LoadingCards` / `ErrorState` imports

**Error:** `Cannot find name 'LoadingCards'` / `Cannot find name 'ErrorState'`

**Root cause:** These components were used in the JSX but never imported.

**Fix:** Added the missing imports:
```typescript
import { LoadingCards } from '@/components/shared/loading'
import { ErrorState } from '@/components/shared/error-state'
```

---

### 5. `routes/settings.tsx(647,649)` — `tKeys.title` / `tKeys.subtitle` missing

**Error:** `Property 'title' does not exist on type '{ readonly engine: ... }'`

**Root cause:** The `tKeys` object literal (`as const`) didn't include `title` and `subtitle`, even though those translation keys exist in the i18n files.

**Fix:** Added to the `tKeys` object:
```typescript
const tKeys = {
  title: 'settings.title',
  subtitle: 'settings.subtitle',
  engine: 'settings.engine',
  ...
}
```

---

### 6. `components/project/*.tsx` — `ToastVariant` mismatch (3 files)

**Error:** `Argument of type '{ variant: string; }' is not assignable to parameter of type 'ToastVariant | undefined'`

**Root cause:** The `toast()` function signature is `(message: string, variant?: ToastVariant)`, but the code passed an object `{ variant: 'destructive' }` as the second argument.

**Fix:** Changed from object form to direct string argument:

```diff
- toast(msg, { variant: 'destructive' })
+ toast(msg, 'destructive')
```

Files affected: `create-project-dialog.tsx`, `delete-project-dialog.tsx`, `edit-project-dialog.tsx`

---

### 7. `components/layout/language-selector.tsx(25)` — `{ code, label }` ≠ `{ label, value }`

**Error:** `Type '{ code: string; label: string; }[]' is not assignable to type '{ label: string; value: string; }[]'.`

**Root cause:** The `Select` component expects `options: { label: string; value: string }[]`, but the languages array used `code` instead of `value`.

**Fix:** Renamed `code` to `value`:
```diff
- { code: 'en', label: 'English' },
- { code: 'ko', label: '한국어' },
+ { label: 'English', value: 'en' },
+ { label: '한국어', value: 'ko' },
```

---

### 8. `components/shared/data-table.tsx(227)` — `string | number | symbol` ≠ `string`

**Error:** `Type 'string | number | symbol' is not assignable to type 'string'.`

**Root cause:** `noUncheckedIndexedAccess` causes filter key access to return `string | number | symbol` from indexed types. The `columnKey` prop expects `string`.

**Fix:** Wrapped in `String()`:
```diff
- columnKey={f.key}
+ columnKey={String(f.key)}
```

---

### 9. `components/skills/skill-detail.tsx(49-50)` — `sd` possibly undefined

**Error:** `'sd' is possibly 'undefined'.` (3 occurrences)

**Root cause:** `STATUS_DISPLAY[skill.status]` returns `undefined` when `skill.status` doesn't match any key in the record.

**Fix:** Added fallback with default status display:
```diff
- const sd = STATUS_DISPLAY[skill.status]
+ const sd = STATUS_DISPLAY[skill.status] ?? { emoji: '⚪', label: skill.status, variant: 'default' as const }
```

---

### 10. `routes/chat.tsx(186,197)` — `Project` type not found

**Error:** `Cannot find name 'Project'.`

**Root cause:** `Project` type was used in inline type annotations but not imported.

**Fix:** Added to existing type import:
```diff
- import type { Session } from '@/types'
+ import type { Session, Project } from '@/types'
```

---

### 11. `components/engine/api-key-input.tsx(77-107)` — `t` not defined (6 uses)

**Error:** `Cannot find name 't'.` (6 occurrences)

**Root cause:** `useTranslation` was imported but never called inside the component. The destructured `t` function was missing.

**Fix:** Added `const { t } = useTranslation()` inside the component body. This was both an unused import (Group A) and a missing variable (Group B) issue.

---

## No `as any` or `@ts-ignore` used

All fixes address the root cause — either the wrong property name, wrong type, or missing import. No suppression comments were needed.
