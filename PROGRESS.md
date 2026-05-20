# Progress

## Status
In Progress

## Tasks
- [x] Rewrite MarkdownEditor with HyperMD for oxios-web Knowledge UI
- [x] Update editor-toolbar with split/info/save buttons + ⌘S shortcut

## Files Changed
- `channels/oxios-web/web/src/components/knowledge/markdown-editor.tsx` — Replaced plain textarea with HyperMD/CodeMirror 5 editor; added `knowledge:save` custom event listener for manual save
- `channels/oxios-web/web/src/components/knowledge/editor-toolbar.tsx` — Added Save (⌘S), Split view (Columns2), Close split (X), and Info panel toggle (PanelRight) buttons with Tooltip wrappers

## Notes
- Replaced the previous `<textarea>`-based editor with a full HyperMD CodeMirror 5 instance
- Key changes: uses `@/lib/hypermd-setup` side-effect import for CM5 module registration, `window.CodeMirror.fromTextArea()` for editor creation, custom `hmdResolveURL`/`hmdReadLink` for wiki-link navigation, `[`-triggered autocomplete via `createLinkHintFn`, auto-save with 1s debounce, first-line `# ` enforcement, formatting shortcuts (Cmd/Ctrl+B/I/Y)
- Toolbar now has: back/forward nav, file name, Save button (⌘S shortcut), Split view toggle (Columns2/X), Info panel toggle (PanelRight)
- Save dispatches a `knowledge:save` custom DOM event; MarkdownEditor listens for it and flushes content immediately
- Verified `Tooltip` component API: `content` prop + `children` wrapper (no Provider/Root needed)
- No new tsc errors introduced; pre-existing codemirror type errors unchanged
