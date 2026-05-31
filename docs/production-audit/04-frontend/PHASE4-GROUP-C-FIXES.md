# Phase 4: Group C — Test Infrastructure Fix

**Date:** 2026-05-31
**Errors fixed:** 22 (13 × `vi`, 3 × `beforeAll/afterEach/afterAll`, 2 × type errors in tests, plus 4 cascading)

## Problem

Vitest is configured with `globals: true` in `vitest.config.ts`, which means `vi`, `beforeAll`, `afterEach`, `afterAll`, `describe`, `it`, `expect` etc. are available at runtime without imports. However, TypeScript's `tsconfig.json` doesn't know about these globals, so `tsc --noEmit` reports:

```
TS2304: Cannot find name 'vi'.
TS2304: Cannot find name 'beforeAll'.
TS2304: Cannot find name 'afterEach'.
TS2304: Cannot find name 'afterAll'.
```

## Solution

Created `tsconfig.test.json` that extends the base config and includes Vitest global types:

```json
{
  "extends": "./tsconfig.json",
  "compilerOptions": {
    "types": ["vitest/globals"]
  },
  "include": ["src", "vite-env.d.ts"],
  "exclude": []
}
```

Modified `tsconfig.json` to exclude `src/__tests__/` from the default type-check:

```diff
  "include": ["src", "vite-env.d.ts"],
+ "exclude": ["src/__tests__"]
```

## Why this approach

1. **`tsc --noEmit`** (the production check) uses `tsconfig.json` → excludes test files → zero errors
2. **`vitest typecheck`** or IDE can use `tsconfig.test.json` → includes Vitest globals → test files type-check correctly
3. **No changes to `vitest.config.ts`** — runtime behavior unchanged
4. **No new npm packages** — `vitest/globals` comes with the existing `vitest` dev dependency

## Additional test type fixes

| File | Fix |
|------|-----|
| `memory-card.test.tsx(103)` | Cast `Element` to `HTMLElement` for `.click()` access |
| `evaluation-card.test.tsx(47)` | Added `?? null` to coalesce optional chaining result |

## Verification

```bash
cd surface/oxios-web/web && npx tsc --noEmit
# 0 errors

cd surface/oxios-web/web && npx tsc --noEmit -p tsconfig.test.json
# 0 errors (test files included with Vitest globals)
```
