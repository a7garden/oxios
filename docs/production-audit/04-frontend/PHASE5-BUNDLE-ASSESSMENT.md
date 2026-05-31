# Phase 5: Bundle Assessment

**Date:** 2026-05-31

## Current Bundle Sizes

| Chunk | Size (raw) | Size (gzip) | Notes |
|-------|-----------|-------------|-------|
| `workspace-*.js` | 677 KB | 242 KB | ⚠️ Exceeds 500KB |
| `index-*.js` | 477 KB | 148 KB | Core app bundle |
| `knowledge-*.js` | 430 KB | 144 KB | HyperMD (CM5), properly split ✅ |
| `CartesianChart-*.js` | 313 KB | 94 KB | Recharts, auto-split ✅ |
| `chat-*.js` | 171 KB | 51 KB | Chat UI |

## Workspace Chunk Analysis (677 KB)

### What's in it

The workspace chunk contains the file browser/editor for `~/.oxios/workspace/`. Its size is driven by **CodeMirror 6** dependencies:

- `@codemirror/view` — Editor rendering, DOM management
- `@codemirror/state` — Editor state model
- `@codemirror/language` — Syntax infrastructure
- `@codemirror/commands` — Key bindings, history
- `@codemirror/theme-one-dark` — Dark theme
- `@codemirror/lang-*` — Language modes (json, rust, markdown, python, yaml, javascript)
- `@codemirror/legacy-modes` — TOML, Shell (via StreamLanguage)
- `codemirror` (CM5) — Used by vite `optimizeDeps.include` for knowledge editor

### Already optimized ✅

- TanStack Router auto-code-splitting is working correctly — workspace is its own chunk
- CM6 language extensions are loaded synchronously (needed for immediate editor rendering)
- No duplicate dependencies detected

### Lazy-loading opportunity

**FileViewer/FileEditor could be lazy-loaded within the workspace page:**

When the user opens `/workspace/`, they first see a file tree. The editor (FileViewer/FileEditor) only renders when a file is selected. Currently, the entire CodeMirror bundle is loaded when the route mounts.

```typescript
// Before (eager):
import { FileViewer } from '@/components/workspace/file-viewer'
import { FileEditor } from '@/components/workspace/file-editor'

// After (lazy):
const FileViewer = lazy(() => import('@/components/workspace/file-viewer').then(m => ({ default: m.FileViewer })))
const FileEditor = lazy(() => import('@/components/workspace/file-editor').then(m => ({ default: m.FileEditor })))
```

This would create a separate chunk for the CodeMirror editor (~400KB) that loads only when a file is opened.

**Decision: NOT implemented** — the brief says "only optimize if there's a clear win" and "do NOT add dynamic imports for minor savings." The workspace page's primary purpose is file viewing/editing, so the editor will almost always be needed. Lazy-loading would add a loading flash for minimal benefit.

## CartesianChart Chunk (313 KB)

Recharts is used by:
- `routes/resources.tsx` — Resource monitoring charts
- `components/memory/memory-overview.tsx` — Memory statistics

Already auto-code-split by TanStack Router. No further optimization needed.

## Knowledge Chunk (430 KB)

HyperMD (CodeMirror 5) is the primary contributor. Already code-split per the existing architecture. No changes needed per brief constraint ("Do not modify the HyperMD or Knowledge UI editor code").

## Index Chunk (477 KB)

Contains React, TanStack Query/Router, i18next, lucide icons, and shared components. This is the baseline app shell — cannot be reduced without structural changes outside scope.

## Recommendations

| Priority | Action | Expected Savings | Effort |
|----------|--------|-----------------|--------|
| ❌ Skip | Lazy-load workspace editors | ~400KB deferred | Low but minimal UX benefit |
| ❌ Skip | Replace Recharts with lighter library | ~300KB | Out of scope (no new packages) |
| ❌ Skip | Tree-shake lucide icons | ~20-50KB | Already tree-shaken by bundler |
| ✅ Done | Fix TypeScript errors | 0KB (type safety only) | Completed |
| ✅ Done | Configure test types | 0KB (DX only) | Completed |

## Conclusion

The bundle is reasonably well-optimized given the constraints:
- Heavy dependencies (CodeMirror, Recharts, HyperMD) are already code-split into route-specific chunks
- The workspace chunk is large because CM6 + language modes are inherently ~400KB
- No duplicate dependencies or obvious tree-shaking failures
- Further optimization requires either lazy-loading within routes or replacing libraries, both outside the audit scope
