# Knowledge Editor Redesign — Implementation Plan

> **Goal:** Replace `@uiw/react-codemirror` with `@atomic-editor/editor`, delete files.md-ported dead code, remove replaced custom extensions, and simplify editor prefs.

**Architecture:** Single file replacement of `markdown-editor.tsx` (749-line monolith → ~200-line `AtomicCodeMirrorEditor` wrapper). Core editing features (inline preview, tables, image blocks, wiki links) come from atomic-editor's built-in extensions. Oxios-specific features (frontmatter, heading enforcer, emoji/math/mermaid folds) remain as CM6 extensions passed via the `extensions` prop.

**Tech Stack:** TypeScript, React 19, CodeMirror 6, @atomic-editor/editor 0.6.x

## Global Constraints

- All imports from `@uiw/react-codemirror` must be removed
- All dead-code files must be deleted (not left as empty stubs)
- Existing API hooks (`use-knowledge.ts`), stores (`knowledge.ts`, `editor-prefs.ts`), and non-editor components (toolbar, status bar, note-title, split-view) must remain functional
- TypeScript `tsc --noEmit` must pass
- Biome `lint` must pass
- Vite build must succeed
- Dark/light theme switching must continue working

---

### Task 1: Install dependency and remove old one

**Files:**
- Modify: `web/package.json`

- [ ] **Step 1: Install @atomic-editor/editor and uninstall @uiw/react-codemirror**

```bash
cd /Volumes/MERCURY/PROJECTS/oxios/web
npm install @atomic-editor/editor@^0.6.2
npm uninstall @uiw/react-codemirror
```

- [ ] **Step 2: Verify package.json and lockfile**

```bash
grep -c '@uiw/react-codemirror' package.json  # expected: 0
grep '@atomic-editor/editor' package.json       # expected: present
```

---

### Task 2: Rewrite markdown-editor.tsx

**Files:**
- Modify: `web/src/components/knowledge/markdown-editor.tsx` (full rewrite)

The new component wraps `AtomicCodeMirrorEditor` with Oxios-specific extensions. Key design:

- Imports from `@atomic-editor/editor` instead of `@uiw/react-codemirror`
- Uses `documentId={filePath}` for clean document switching (no `editorSessionId` key needed)
- Routes `onMarkdownChange` through existing debounce+save logic
- Heading enforcer as a standalone CM6 extension (no WeakSet suspension needed)
- Wiki links wired to knowledge tree via atomic-editor's `wikiLinks()` config
- Oxios-specific fold extensions (emoji, math, mermaid) passed via `extensions` prop
- Stats tracker as `EditorView.updateListener`
- Image URL resolution (relative paths → backend asset route) via `onLinkClick` and custom handler

- [ ] **Step 1: Delete current markdown-editor.tsx contents**

- [ ] **Step 2: Write new markdown-editor.tsx** (see spec architecture for structure)
- [ ] **Step 3: Delete 12 files** (7 replaced extensions + 5 dead-code files)

```bash
rm web/src/lib/live-preview-extension.ts
rm web/src/lib/token-hide-extension.ts
rm web/src/lib/table-fold-extension.ts
rm web/src/lib/image-fold-extension.ts
rm web/src/lib/wikilink-extension.ts
rm web/src/lib/autocomplete-link.ts
rm web/src/lib/katex-renderer.ts
rm web/src/lib/emoji.ts
rm web/src/lib/md.ts
rm web/src/lib/similarity.ts
rm web/src/lib/hypermd-mermaid.ts
rm web/src/lib/emoji.ts
```

---

### Task 3: Update editor prefs store

**Files:**
- Modify: `web/src/stores/editor-prefs.ts`

Remove prefs that atomic-editor makes always-on: `lineNumbers`, `bracketMatching`, `livePreview`, `tokenHiding`, `imageFold`, `tableFold`, `activeLineHighlight`, `foldGutter`. Keep typography, heading colors, marker/link colors, emoji/math/mermaid folds, status bar toggle.

- [ ] **Step 1: Remove deprecated prefs from interface and defaults**

---

### Task 4: Update editor settings popover

**Files:**
- Modify: `web/src/components/knowledge/editor-settings-popover.tsx`

Remove UI controls for now-built-in features: line numbers, active line highlight, fold gutter, bracket matching, live preview, token hiding, image fold, table fold.

- [ ] **Step 1: Remove deprecated toggle rows from settings popover**

---

### Task 5: Verify

- [ ] **Step 1: TypeScript check**

```bash
cd /Volumes/MERCURY/PROJECTS/oxios/web && bun run typecheck
```

- [ ] **Step 2: Lint check**

```bash
cd /Volumes/MERCURY/PROJECTS/oxios/web && bun run lint
```

- [ ] **Step 3: Build**

```bash
cd /Volumes/MERCURY/PROJECTS/oxios/web && bun run build
```
