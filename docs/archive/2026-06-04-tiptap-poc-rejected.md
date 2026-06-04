# Tiptap PoC — REJECTED (archived 2026-06-04)

**Date:** 2026-06-04
**Branch archived:** `exp/frontend-markdown-editor-poc`
**Verdict:** ❌ Tiptap pure WYSIWYG does NOT meet Oxios's UX requirement.

See [`2026-06-04-frontend-tech-debt-migrations.md`](../production-audit/04-frontend/2026-06-04-frontend-tech-debt-migrations.md) §D-3 / "Option C" for the full migration plan this PoC was meant to validate.

## Why rejected

The HyperMD/CM5 editor that Oxios has shipped until 2026-06-04 shows the user
a **markdown source view** with markup tokens (`**`, `#`, `[]()`, etc.) hidden
on inactive lines. This is the **Obsidian/Logseq editing experience** — users
see and edit the raw markdown, with light visual hints on the line they're
touching.

Tiptap is a pure WYSIWYG editor (Notion/Linear model). The user **never sees
the markdown source** during editing. Migrating to Tiptap would be a
regression of editing UX, not a preservation.

The PoC (V1-V4 PASS) proved Tiptap *can* represent the Oxios dialect
(`[[wiki]]`, `- [x]`, ```mermaid```, etc.), but the editing experience does
not match the requirement. "기존 기능 100% 보전" is not satisfied.

## What the PoC validated

- Tiptap v3.25 + `prosemirror-markdown` 1.13.4 can model the Oxios dialect.
- React 19 + Tiptap 3.25 + prosemirror-markdown are compatible.
- Lazy-loaded mermaid render works (no `Promise` rejection in dev server).
- Custom NodeView + click handlers work.
- Tiptap's `addStorage().markdown` hook pattern is the right extension
  integration point for round-trip.

## What was archived

The branch contained:
- `poc/markdown-editor-tiptap/` — standalone Vite app with 3 custom
  Tiptap extensions and a Playwright-driven V1-V5 verification harness.
- `docs/designs/2026-06-04-tiptap-poc-design.md` — full PoC design.

These can be revived if/until Oxios decides to switch to a WYSIWYG model.

## What replaced it (Option B, all 3 phases landed on `main`)

| PR | Phase | Status |
|----|-------|--------|
| #5 | Phase 1: `@uiw/react-codemirror` base + 5/5 features preserved | merged (a9b2dc2) |
| #6 | Phase 2: syntax highlight + code folding | merged (e60cd3c) |
| #7 | Phase 3a: mermaid widget + oneDark theme | merged (42c1a9f) |
| #8 | Phase 3b: token hiding (hmdHideToken parity) | merged (d6fabb3) |

The CM6 path restored the Obsidian-style editing UX, kept the 5/5
preserved features (auto-save, heading enforcement, ⌘B/I/Y/S, wiki/emoji
autocomplete, link click), and added 4 HyperMD features that 2026's
markdown editors do better (inline syntax highlight, code folding, Mermaid
inline render, true dark theme).
