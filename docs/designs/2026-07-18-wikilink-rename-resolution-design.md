# Wikilink Resolution & Rename-Proofing — Design

**Date:** 2026-07-18
**Status:** Proposed
**Depends on:** `note_move` cross-file `[text](path)]` rewriting (shipped 2026-07-18 in `crates/oxios-markdown/src/knowledge.rs`)
**Blocks:** none

## 1. Problem

The knowledge editor treats a note's first `# Heading` as its title and renames
the file when the heading changes. That rename path now rewrites inbound
`[text](path)]` markdown links in other notes. **`[[wikilinks]]` are not covered**,
and on closer inspection they have three independent defects:

| # | Defect | Evidence |
|---|---|---|
| D1 | **Click is broken for the common case.** `[[Rust]]` dispatches `openFile("Rust")`, which 404s because the file is `Rust.md`. Only `[[Rust.md]]` works today. | `web/src/lib/wikilink-extension.ts:72` dispatches the bare target; contrast `markdown-editor.tsx:212` which appends `.md` for markdown links. |
| D2 | **Wikilinks are invisible to the index.** `BacklinkIndex` (via `extract_markdown_links`) only matches `\[..\]\(..\)`. `[[X]]` is never tracked → no backlinks, no graph edges. | `crates/oxios-markdown/src/parser.rs:250`, `crates/oxios-markdown/src/backlinks.rs:79`. |
| D3 | **Rename orphans inbound wikilinks.** `note_move` rewrites `[text](old)]`→`[text](new)]` in referencing files but does not touch `[[old]]`. | `crates/oxios-markdown/src/knowledge.rs` `note_move` (post-2026-07-18). |

Note: the backend already has a wikilink *normalizer* — `tgtxt::extract_text_imgs_links`
canonicalizes `[[X]]` → `X.md` (`tgtxt.rs:117` `split_link_content`). It is used
only for Telegram text extraction, not by the backlink index. So the parsing
logic exists; it is just not wired into the link graph.

## 2. Goals / Non-Goals

**Goals**
- G1: Clicking `[[X]]` opens the note it refers to (resolve bare names, paths, aliases).
- G2: Wikilinks appear in the backlink index and the link graph (parity with `[text](path)`).
- G3: Renaming a note rewrites inbound `[[…]]` links in other notes (parity with the markdown-link rewrite in `note_move`).
- G4: Alias form `[[target|alias]]` keeps its alias through a rewrite.

**Non-Goals**
- Bidirectional auto-rename (renaming a note does NOT rewrite its own outbound links' targets beyond self-refs).
- Wikilink autocomplete/suggestions in the editor (separate feature).
- Migrating existing user notes to a canonical form — rewrites are form-preserving (see §5).

## 3. Resolution Model (the foundational decision)

A wikilink target can be one of four forms:

| Form | Example | Meaning |
|---|---|---|
| Bare stem | `[[Rust]]` | Any note whose filename stem is "Rust" |
| Relative path | `[[brain/Rust]]` | `brain/Rust.md` |
| Full path | `[[brain/Rust.md]]` | exact |
| With alias | `[[brain/Rust\|Rusty]]` | target is left of `\|`, alias is display-only |

**Decision: basename resolution with a directory hint (Obsidian-flavored).**

1. **Full path** (`[[brain/Rust.md]]`) → exact match.
2. **Path without extension** (`[[brain/Rust]]`) → append `.md`, exact match.
3. **Bare stem** (`[[Rust]]`) → look up the set of files whose stem equals `Rust`:
   - Unique match → resolved.
   - Multiple matches → prefer the one in the **same directory as the source note**; if still ambiguous → **unresolved** (rendered distinct, no navigation). We do not guess.
   - Zero matches → unresolved.

This is the model that makes `[[Rust]]` "just work" in a flat personal vault while
staying predictable when names collide.

## 4. Where Resolution Lives

Resolution is needed in **two** places with different constraints:

| Consumer | Needs | Recommendation |
|---|---|---|
| Frontend click handler | target → file path, fast, no round-trip | Client-side, build `Map<stemLower, path[]>` from the already-loaded recursive tree (`useKnowledgeRecursiveTree`). |
| Backend `note_move` / `BacklinkIndex` | target → canonical path for matching | Backend resolver, same algorithm, so the index keys on canonical paths. |

**Why both:** the frontend already has the whole tree in memory (the file-tree
fetches `?recursive=true`). A click should not pay a network round-trip. The
backend needs resolution at index time so it can store the canonical path in
the backward index — which is what makes rename matching exact.

The resolution algorithm is identical in both places and small (~30 lines);
duplicating it is cheaper than inventing a new endpoint and worth the parity.

## 5. Canonicalization vs. Form Preservation

When `note_move` rewrites a `[[…]]`, should it canonicalize to `[[full/path.md]]`
or preserve the user's original form?

**Decision: preserve form, rewrite minimally.** If a user wrote `[[Rust]]` and
`Rust.md` renames to `Rust Lang.md`, rewrite to `[[Rust Lang]]` (stem swapped,
structure preserved). If they wrote `[[brain/Rust]]`, rewrite to
`[[brain/Rust Lang]]`. Aliases are preserved verbatim:
`[[Rust\|Rusty]]` → `[[Rust Lang\|Rusty]]`.

Rationale: canonicalizing would rewrite prose the user did not ask us to touch,
and would balloon diffs. Preserve-form costs a few more match patterns but
respects the user's writing.

Rewrite patterns (all must be matched, in this order — longest/most-specific
first to avoid partial matches):

| `old_path` | `new_path` | Pattern in source note | Replacement |
|---|---|---|---|
| `brain/Rust.md` | `brain/Rust Lang.md` | `[[brain/Rust.md]]` | `[[brain/Rust Lang.md]]` |
| | | `[[brain/Rust]]` | `[[brain/Rust Lang]]` |
| | | `[[Rust]]` (only if unambiguous — see §6) | `[[Rust Lang]]` |
| | | `[[brain/Rust.md\|alias]]` | `[[brain/Rust Lang.md\|alias]]` |
| | | `[[brain/Rust\|alias]]` | `[[brain/Rust Lang\|alias]]` |
| | | `[[Rust\|alias]]` (unambiguous) | `[[Rust Lang\|alias]]` |

The bare-stem forms (`[[Rust]]`, `[[Rust|alias]]`) are only rewritten when the
resolver can prove the link pointed at `old_path` uniquely (see §6).

## 6. Ambiguity Handling on Rename

The dangerous case: two notes share a stem (`brain/Rust.md` and `lang/Rust.md`).
A bare `[[Rust]]` in a third note is ambiguous — we cannot know which one it
meant. If we rewrite on rename of one of them, we may corrupt the user's intent.

**Decision: conservative.** For each source file referencing `[[Rust]]`:
- If the resolver (run against the source file's directory context) returns a
  **unique** match that equals `old_path` → rewrite.
- If ambiguous or resolved to a different file → **skip** (leave the link; the
  user will see it as unresolved/ambiguous in the UI and can fix it manually).

Path-style links (`[[brain/Rust]]`, `[[brain/Rust.md]]`) are unambiguous by
construction and are always rewritten.

This means a rename never silently retargets a link the system isn't sure about.

## 7. Implementation Plan (phased)

Each phase is independently shippable and improves the status quo.

### Phase 1 — Frontend click resolution (fixes D1, no backend change)
- New `web/src/lib/wikilink-resolve.ts`: `buildWikilinkIndex(tree) → Map<stemLower, path[]>`
  and `resolveWikilink(target, sourcePath, index) → string | null`.
- `wikilink-extension.ts`: read the index + source path from module-level refs
  (set via a new `configureWikilinkResolver(...)` called from `markdown-editor.tsx`
  on tree/path changes). On click, resolve; if resolved, dispatch
  `openFile(resolvedPath)`, else no-op + console.warn.
- Style: unresolved targets get a distinct class (`cm-wikilink-unresolved`).
- **Value:** existing `[[X]]` links start working immediately. Zero backend risk.

### Phase 2 — Backend indexing parity (fixes D2)
- Rename `extract_markdown_links` → `extract_links` (or add `extract_wikilinks`).
  Extract `[[target|alias?]]` with the same regex `tgtxt` already uses.
- `BacklinkIndex::index_file`: index both link kinds. For wikilinks, run the
  resolver to canonicalize the target; store under canonical path. Mark
  unresolved targets under a separate bucket (`unresolved: HashSet<raw>`) so
  they're tracked but don't pollute the graph.
- Graph edges now include wikilinks.
- **Value:** backlinks panel and link graph reflect reality.

### Phase 3 — Rename parity (fixes D3)
- Extend `note_move`: after the existing `[text](old)]`→`[text](new)]` rewrite
  pass, run a wikilink rewrite pass over each referencing source. Patterns from
  §5, ambiguity rule from §6.
- New helper `parser::rewrite_wikilink_targets(content, old_path, new_path,
  resolver) -> (String, count)` — mirrors `rewrite_link_targets`.
- **Value:** H1-rename and F2-rename now leave `[[…]]` intact, matching the
  markdown-link behavior shipped 2026-07-18.

### Phase 4 — Polish
- Unresolved-link styling in light/dark themes.
- Info-panel "Unresolved links" section (notes that point at nothing).
- Optional: one-time reindex job on daemon boot to populate the new wikilink
  entries for existing notes (only needed if Phase 2 lands without a full
  re-scan trigger).

## 8. Testing

- **Frontend** (`web/src/lib/wikilink-resolve.test.ts`):
  - unique stem resolves; path+ext exact; alias strips; ambiguous → null;
    same-dir preference; zero matches → null.
- **Backend** (`crates/oxios-markdown/src/`):
  - `parser::rewrite_wikilink_targets`: each form in §5, alias preservation,
    no-op when targets equal, regex-escaping of special chars in stems.
  - `BacklinkIndex::index_file`: wikilinks appear in `sources_for(target)`.
  - `note_move`: `test_note_move_rewrites_wikilinks_bare` (unique → rewritten),
    `_ambiguous` (two same-stem files → bare link skipped, path link rewritten),
    `_with_alias`, `_path_form`, `_full_path_form`.

## 9. Risks & Open Questions

1. **Backend resolver needs the file tree at index time.** `BacklinkIndex`
   doesn't currently own a basename index. Add a `HashMap<stemLower, Vec<path>>`
   built lazily from the fs and invalidated on writes, OR pass a resolver
   closure into `index_file`. Lean toward the closure — keeps the index pure.
2. **Stale index after out-of-band edits** (e.g. user edits `.md` files
   directly on disk, bypassing the daemon). The existing git-reconciliation
   path (`list_all_md_files`) already detects drift; a wikilink reindex should
   piggyback on it.
3. **Form-preservation bookkeeping** is more regex than the markdown-link case.
   Risk of under-matching (leaving a link) is acceptable; risk of
   over-matching (corrupting prose) is not. Err on conservative.
4. **Performance** of building the basename index on every recursive-tree
   refetch: fine for personal KBs (hundreds of files). Memoize on the tree
   reference so it's rebuilt only when the tree changes.
5. **Do we want Obsidian-style "shortest path" disambiguation** instead of
   same-directory preference? Same-dir is simpler and matches the user's
   mental model in a personal vault. Revisit if multi-folder vaults complain.

## 10. Scope Summary

| Phase | Ships | Defect fixed | Risk |
|---|---|---|---|
| 1 | Frontend resolver + click fix | D1 | Low (frontend-only) |
| 2 | Backend wikilink indexing | D2 | Medium (touches `BacklinkIndex`, needs reindex) |
| 3 | `note_move` wikilink rewrite | D3 | Medium (extends a core path; well-tested) |
| 4 | UI polish, reindex | — | Low |

Phases 1 and 3 are the highest value-per-effort: Phase 1 makes existing
wikilinks usable; Phase 3 closes the rename-correctness gap that motivated this
design. Phase 2 is the right thing for index parity but is the largest change.
