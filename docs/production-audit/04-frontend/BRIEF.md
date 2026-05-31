# Brief 04: Frontend — TypeScript Errors & Bundle Health

**Area:** TypeScript type safety, unused imports, bundle optimization  
**Severity:** 🟡 High  
**Estimated scope:** 60+ TS errors, 677KB largest chunk, 76 dependencies  

---

## Context

The frontend (`surface/oxios-web/web/`) is a React + TanStack Router app
with HyperMD (CodeMirror 5) for the Knowledge UI. It builds successfully
but has significant type safety issues.

**TypeScript errors** (`npx tsc --noEmit`): ~60 errors across:

| Category | Count | Examples |
|----------|-------|---------|
| Unused imports/variables (`TS6133`, `TS6192`) | ~30 | `'Loader2' is declared but never read`, unused `t`, unused `cn` |
| Missing test globals (`TS2304` `vi`, `beforeAll`) | ~15 | Test files don't have proper Vitest type config |
| Type mismatches (`TS2322`, `TS2345`) | ~8 | `ToastVariant` mismatch, `StreamChunk.project_ids` vs `project_id` |
| Possibly undefined (`TS18048`) | ~3 | `sd` possibly undefined in skill-detail |
| Missing properties (`TS2339`) | ~4 | `title`/`subtitle` missing on i18n type, `LoadingCards`/`ErrorState` not found |

**Bundle:**
- `workspace-*.js`: 677KB (exceeds 500KB recommended)
- `index-*.js`: 477KB
- `knowledge-*.js`: 421KB (HyperMD is 447KB, properly code-split ✅)

**The build succeeds** because Vite/Bun do not enforce TypeScript strictness
at build time. This means runtime errors are possible in production.

---

## Objective

1. **Zero TypeScript errors** in `tsc --noEmit`
2. **Reduce bundle size** where possible without code splitting changes
3. **Fix test type configuration** for Vitest globals

This does NOT mean:
- ❌ Rewriting components for "better architecture"
- ❌ Adding new UI libraries or state management
- ❌ Converting to a different framework pattern
- ❌ Over-optimizing bundle size by breaking up logical chunks

It DOES mean:
- ✅ Remove genuinely unused imports (if they're unused, delete them)
- ✅ Fix type mismatches (actual bugs waiting to happen)
- ✅ Fix the Vitest test setup so `vi`, `beforeAll` etc. are recognized
- ✅ Lazy-load the workspace chunk if it's straightforward

---

## Approach

### Phase 1: Classify TypeScript Errors

Read the full `tsc --noEmit` output and group errors:

**Group A — Unused imports (low risk, mechanical fix)**
- Remove unused imports
- Be careful: some "unused" imports might be needed for side effects
  (e.g., CSS imports, type-only imports used in generics)
- Verify each removal by checking if the symbol is referenced elsewhere
  in the file

**Group B — Actual type bugs (medium risk)**
- `stores/chat.ts(349)`: `project_ids` → `project_id` — this is a real
  property name mismatch. Check the API response type.
- `stores/chat.ts(352)`: comparing `string | undefined` with `boolean` —
  logic error
- `stores/chat.ts(376)`: `id` is `string | undefined` but should be `string`
  — likely needs a fallback or guard
- `routes/settings.tsx`: `title`/`subtitle` missing on i18n type — add
  the keys to the translation file
- `components/project/*.tsx`: `ToastVariant` type mismatch — fix the
  variant type or the call

**Group C — Test infrastructure (separate concern)**
- Add Vitest globals to `tsconfig.json` or test setup
- These are NOT production issues but they pollute `tsc` output
- Add a `tsconfig.test.json` that extends the base and includes Vitest
  types, OR configure Vitest properly in `vite.config.ts`

### Phase 2: Fix Group A (unused imports)

For each unused import:
1. Read the file
2. Search for usages of the imported symbol in the file
3. If truly unused, remove it
4. If it's a type import needed for inference, add `// keep: type inference`
   comment and add a `// @ts-expect-error` if needed
5. Run `tsc --noEmit` after every 5 fixes to catch cascading errors

### Phase 3: Fix Group B (type bugs)

For each actual type error:
1. Read the surrounding code
2. Understand the intent
3. Fix the type — do NOT use `as any` or `@ts-ignore` unless absolutely
   necessary and documented
4. Prefer fixing the source (API type definition, store type) over
   fixing the consumer

### Phase 4: Fix Group C (test types)

1. Check `surface/oxios-web/web/tsconfig.json`
2. Check `surface/oxios-web/web/vite.config.ts`
3. Ensure Vitest types are configured properly so `vi`, `beforeAll`,
   `afterEach`, etc. are recognized
4. This may be as simple as adding `types: ["vitest/globals"]` to
  the tsconfig for test files

### Phase 5: Bundle Assessment

1. Analyze the workspace chunk (677KB) — what's in it?
   Run `bun run build` with `--analyze` or check the rollup output
2. Identify low-hanging fruit:
   - Large dependencies that could be lazy-loaded
   - Duplicate dependencies (same lib included twice)
3. Only optimize if there's a clear win. Do NOT restructure imports
   or add dynamic imports for minor savings.

---

## Constraints

- **Do not** change the component architecture or file structure
- **Do not** add new npm packages
- **Do not** use `@ts-ignore` or `as any` as a fix (unless truly
  necessary with a comment explaining why)
- **Do not** modify the HyperMD or Knowledge UI editor code (it works)
- **Do not** change the routing structure
- **Preserve** the existing i18n setup (react-i18next)
- **Preserve** the existing TanStack Router code-splitting strategy

## Verification

1. `cd surface/oxios-web/web && npx tsc --noEmit` — zero errors
2. `cd surface/oxios-web/web && bun run build` — still builds
3. Visual check: open the app and verify nothing is visually broken
