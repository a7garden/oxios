# Progress

## Status
In Progress

## Tasks
- [x] Rewrite MarkdownEditor with HyperMD for oxios-web Knowledge UI

## Files Changed
- `channels/oxios-web/web/src/components/knowledge/markdown-editor.tsx` — Replaced plain textarea with HyperMD/CodeMirror 5 editor

## Notes
- Replaced the previous `<textarea>`-based editor with a full HyperMD CodeMirror 5 instance
- Key changes: uses `@/lib/hypermd-setup` side-effect import for CM5 module registration, `window.CodeMirror.fromTextArea()` for editor creation, custom `hmdResolveURL`/`hmdReadLink` for wiki-link navigation, `[`-triggered autocomplete via `createLinkHintFn`, auto-save with 1s debounce, first-line `# ` enforcement, formatting shortcuts (Cmd/Ctrl+B/I/Y)
