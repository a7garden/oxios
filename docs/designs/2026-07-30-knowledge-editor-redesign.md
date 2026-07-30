# Knowledge Editor Redesign

> Replace the current `@uiw/react-codemirror`-based markdown editor with a clean architecture built on `@atomic-editor/editor`, removing files.md-ported dead code and HyperMD-era patterns.

**Status:** Design (pre-implementation)
**Date:** 2026-07-30

---

## Motivation

The current knowledge-base markdown editor has accumulated significant technical debt:

| Problem | Impact |
|---|---|
| `@uiw/react-codemirror` wrapper | `value`+`onChange` pattern forces dirty-guard hacks (`dirtyRef`, `isSettingContent`, `_headingEnforcerSuspended` WeakSet) — ~150 lines of defensive code |
| files.md-ported code | 5 files (`katex-renderer.ts`, `emoji.ts`, `md.ts`, `similarity.ts`, `hypermd-mermaid.ts`) are dead imports — only `md.ts` and `similarity.ts` are actually referenced, and only through comment mentions |
| Overlapping extensions | `livePreviewExtension` + `tokenHideExtension` both manage inactive-line rendering — can conflict, no shared infrastructure (tree-progress, freeze-on-mouse) |
| Read-only virtual fold | `tableFoldExtension` renders tables as read-only HTML. `imageFoldExtension` replaces source lines, causing layout shift on cursor movement |
| Module-level mutable state | `wikilink-extension.ts` uses `_resolver`/`_resolverVersion` globals — fragile with multiple editor instances (split view) |

`@atomic-editor/editor` addresses all of these with a properly-engineered CM6 wrapper and extensions.

---

## Approach

**Use `@atomic-editor/editor` as an npm dependency.** Its `AtomicCodeMirrorEditor` component replaces our hand-rolled `@uiw/react-codemirror` setup. Oxios-specific features (frontmatter properties, emoji/math/mermaid folds, heading enforcement, knowledge-tree wikilink resolution) are layered as CM6 extensions via the `extensions` prop.

This is pragmatic: atomic-editor is MIT-licensed, well-tested (Playwright e2e + Vitest), and peer-depends on the same CM6 packages we already use.

---

## Changes

### Dependencies

```diff
- "@uiw/react-codemirror": "^4.25.10",
+ "@atomic-editor/editor": "^0.6.0",
```

All peer deps (`@codemirror/*`, `@lezer/*`, `react`) are already present.

### File Operations

**Modify:**
- `web/src/components/knowledge/markdown-editor.tsx` — replace with new `OxiosKnowledgeEditor` component wrapping `AtomicCodeMirrorEditor`
- `web/src/components/knowledge/editor-settings-popover.tsx` — remove toggles for features now built into atomic-editor
- `web/src/stores/editor-prefs.ts` — remove unused prefs fields

**Delete (files.md dead code):**
- `web/src/lib/katex-renderer.ts`
- `web/src/lib/emoji.ts`
- `web/src/lib/md.ts`
- `web/src/lib/similarity.ts`
- `web/src/lib/hypermd-mermaid.ts`

**Delete (replaced by atomic-editor):**
- `web/src/lib/live-preview-extension.ts`
- `web/src/lib/token-hide-extension.ts`
- `web/src/lib/table-fold-extension.ts`
- `web/src/lib/image-fold-extension.ts`
- `web/src/lib/wikilink-extension.ts`
- `web/src/lib/autocomplete-link.ts`

**Keep (Oxios-specific, adapted):**
- `web/src/lib/frontmatter.ts` — frontmatter parsing + PropertiesWidget
- `web/src/lib/emoji-fold-extension.ts` — `:shortcode:` → emoji
- `web/src/lib/emoji-shortcodes.ts` — shortcode dictionary
- `web/src/lib/math-fold-extension.ts` — KaTeX `$…$` / `$$…$$`
- `web/src/lib/mermaid-extension.ts` — mermaid diagram rendering
- `web/src/lib/wikilink-resolve.ts` — wikilink index + resolution utilities (atomic-editor wiki-links `resolve`/`suggest` callbacks consume this)

---

## Component Architecture

### New `markdown-editor.tsx`

```
OxiosKnowledgeEditor
├── AtomicCodeMirrorEditor (from @atomic-editor/editor)
│   ├── Built-in: inline-preview, WYSIWYG tables, image blocks,
│   │             wiki-links, edit-helpers, ==highlight==, read-only, search
│   └── extensions (via prop):
│       ├── headingEnforcer      ← Oxios: keep H1 as first line
│       ├── frontmatterExtension ← Oxios: PropertiesWidget for YAML
│       ├── wikiLinks(config)    ← atomic-editor API, wired to Oxios tree
│       ├── emojiFoldExtension   ← Oxios: :shortcode: -> emoji
│       ├── mathFoldExtension    ← Oxios: $..$ -> KaTeX
│       ├── mermaidExtension    ← Oxios: ```mermaid -> diagram
│       ├── oxiosAutocomplete   ← Oxios: knowledge tree + emoji
│       ├── statsTracker        ← Oxios: word/char/line/cursor stats
│       └── oxiosKeymap         ← Oxios: ⌘B/⌘I/⌘Y/⌘S
├── EditorToolbar (unchanged)
├── EditorStatusBar (unchanged)
├── EditorSettingsPopover (reduced toggles)
└── NoteTitle (unchanged)
```

### Key Simplifications vs. Current Code

| Current | New | Why |
|---|---|---|
| `isSettingContent` ref | Removed | atomic-editor's `documentId` prop handles clean swap |
| `dirtyRef` + dirty guard | Removed | `onMarkdownChange` is a simple fire-and-forget callback |
| `_headingEnforcerSuspended` WeakSet | Removed | Heading enforcer is a standalone CM6 extension, atomic-editor doesn't fight it |
| Mount-time cursor reposition hack | Removed | atomic-editor has `initialRevealText` and `initialSearchText` props |
| Blur save handler | Removed | On-blur saving is built into the debounce pattern |
| DOM event bus (`knowledge:save`, `knowledge:set-title`) | Kept | Used by toolbar → editor communication; compatible with atomic-editor via `document.dispatchEvent` |

---

## Editor Prefs Changes

**Removed prefs** (atomic-editor built-in, always-on):
- `livePreview`
- `tokenHiding`
- `tableFold`
- `imageFold`
- `bracketMatching`
- `lineNumbers` (atomic-editor hides gutters by default)
- `activeLineHighlight`
- `foldGutter`

**Kept prefs:**
- `fontSize`, `lineHeight`, `fontFamily` — CSS custom properties
- `showStatusBar` — Oxios component
- `headingColorsEnabled`, `headingColors[1-6]` — Oxios HighlightStyle
- `markerColor`, `linkColor` — Oxios HighlightStyle
- `emojiFold`, `mathFold`, `mermaidFold` — Oxios-specific extensions

---

## Storage & Save Flow

Same logic, fewer defense layers:

```
[User types] → onMarkdownChange(md) → debounce 1s → onSave(content)
[⌘S / Save button] → document.dispatch('knowledge:save') → flushDebounce → onSave(content)
[NoteTitle commit] → editorRef → view.dispatch(H1 rewrite) → onMarkdownChange → onSave(new content)
```

`isDirty` state removed from the editor component; the toolbar's `Save` button always dispatches the save event (debounce ensures no redundant writes). If the user has concerns about unsaved changes, we re-add `isDirty` via a lightweight state field in the CM6 editor state instead of a React state.

---

## Verification

1. `cargo build --workspace` (Rust backend must compile)
2. `bun run build` (web frontend must build)
3. `bun run typecheck` (TypeScript type-safety)
4. `bun run lint` (Biome compliance)
5. Browser smoke test: open a note, type, save, switch notes, split view, title edit, dark mode
6. Confirm no missing import errors on deleted files
