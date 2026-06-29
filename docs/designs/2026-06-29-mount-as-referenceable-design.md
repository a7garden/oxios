# Mount as the Referenceable Concept — Design

> **Status:** Design (pre-implementation)
> **Date:** 2026-06-29
> **Amends/Completes:** RFC-025 (Mount + Project System) — the migration it specified was never finished
> **Related:** RFC-011 (project-system, superseded), RFC-003 (knowledge-separation), RFC-008 (memory consolidation)

## Summary

Three problems the user reports are one root cause: **RFC-025's Mount↔Project split was never completed.**

1. A Project still carries its own `paths` *and* a `mount_ids` list — two parallel ways to attach a filesystem path.
2. Mounts are not `@`-referenceable from chat, so they don't feel like "one concept registered once, usable everywhere."
3. Mounts cannot be attached to a Project by drag-and-drop; the create-flow doesn't even expose mounts.

This design completes RFC-025 along three pillars, and reframes **Mount** and the **Knowledge Base** as the system's two user-facing "memories" — both addressable everywhere via a single unified `@` picker.

---

## Problem & Root Cause

### The duplication is real, and it is structural

RFC-025 §"Data Model" explicitly stated what to remove from `Project`:

> **Removed** from RFC-011's `Project`: `description`, `tags`, `emoji`, `memory_visible`, `paths` (→ `Mount`).

None of that removal happened. At every layer, `Project` still carries the old fields alongside the new `mount_ids`:

| Layer | File | Evidence |
|---|---|---|
| Backend struct | `crates/oxios-kernel/src/project/mod.rs:66` | `pub paths: Vec<PathBuf>` (+ `description`, `tags`, `emoji`, `memory_visible`) |
| API DTO | `crates/oxios-kernel/src/kernel_handle/project_api.rs:24-31` | `pub paths: Vec<String>` **and** `pub mount_ids: Vec<String>` side by side |
| Web type | `web/src/types/index.ts:59,65` | `paths?: string[]` **and** `mount_ids?: string[]` |
| Create flow | `web/src/components/project/create-project-dialog.tsx:68,179-191` | collects `paths` directly; **no mount picker at all** |
| Edit flow | `web/src/components/project/edit-project-dialog.tsx:48-49,173-189` | mount chip toggles exist (the correct pattern) — but `paths` is still on the type |

Because the create flow writes `paths` and the edit flow writes `mount_ids`, a single project can end up with both, and the two sources are never reconciled.

### The runtime has a "migration window" that never closed

The orchestrator still grants path access from the legacy field when the new field is empty — the explicit escape hatch RFC-025 promised to remove:

```rust
// orchestrator.rs:328-334 — "Legacy fallback (RFC-025 migration window)"
// that case grant path access directly so pre-RFC-025 Projects still
// resolve a CWD and populate `allowed_paths` (see agent_runtime.rs).
if let Some(project) = &project_for_instructions
    && project.mount_ids.is_empty()
    && !project.paths.is_empty()
```

`agent_runtime.rs:728-733` mirrors it: `mount_paths[0]` first, else `config.project_paths[0]`. So `Project.paths` is still load-bearing — which is exactly why it can't simply be hidden in the UI. The migration must actually run.

A second downstream consumer reads the field directly: `token_maxing/planner.rs:98` does `mount_paths: proj.paths.clone()` for project tasks. Removing `Project.paths` means the planner must resolve paths from the project's `mount_ids` instead.

### `@` in chat does not know Mounts exist

`ContextAttachment` and `MentionResult` are hard-coded to two kinds (`chat-input.tsx:13-29`):

```ts
export interface ContextAttachment {
  type: 'knowledge' | 'memory'
  ...
}
```

The `@`-search merges only `knowledgeSearch` and `memorySearch` (`chat-input.tsx:127-156`). Mounts — which are already fully indexed and searchable via `GET /api/mounts?search=` (`mount_routes.rs:71-115`) — are invisible to the picker.

---

## Design Principles

1. **Single source of truth for paths.** The path-bearing concept is `Mount` and *only* `Mount`. `Project` references mounts; it never owns a path.
2. **Mount is THE addressable concept.** No new parallel abstraction. The existing `@` picker widens to include Mounts; a single `ContextSource` discriminant keys all three sources.
3. **Unified picker, split binding.** The `@` UI is one popover; but downstream, Mounts bind the **session** (path access + CWD + workspace context) while Knowledge/Memory inject **message-scoped text**. Same trigger, different weight — because a Mount is structurally heavier than a note.
4. **Clean cutover.** Migrate every caller; no shims, no deprecated parallel fields kept "just in case." The legacy fallback code is deleted, not feature-flagged.

---

## Pillar A — Complete the Mount↔Project Migration

**Goal:** make `Project.paths` unreachable, then remove it.

### A1. Backfill on load

`ProjectManager` already loads all projects into memory at startup (`project/manager.rs:41-43`). On load, run a one-time backfill:

```
for each project p where p.mount_ids.is_empty() && !p.paths.is_empty():
    create one Mount named p.name (or "<p.name>" if unique, else "<p.name>-fs")
        with paths = p.paths
    p.mount_ids = [new_mount.id]
    p.paths = []            # cleared in-memory and persisted
```

- One mount per project (not per path) — a project's paths are conceptually one workspace; splitting them across mounts fragments CWD.
- Idempotent: only touches projects where `mount_ids` is empty. Re-running is a no-op.
- Persisted immediately so a crash mid-backfill doesn't re-trigger for already-migrated projects.
- The backfill is the *only* consumer of the old `paths` field after this release; the field can then be deleted.

### A2. Remove `Project.paths` and the RFC-011 leftovers

Delete from `Project` (backend struct, `ProjectInfo` DTO, web type, create/edit DTOs):

| Field | Fate | Rationale (RFC-025) |
|---|---|---|
| `paths` | **removed** | sole source of paths is now `mount_ids → Mount.paths` |
| `description` | **removed** | code projects show the primary Mount's `auto_description`; non-code projects show `instructions` |
| `tags` | **removed** | replaced by `Mount.auto_meta` (languages/stack/markers) |
| `emoji` | **derived** (UI-only) | derived from name + primary mount's `auto_meta`; never user input |
| `memory_visible` | **removed** | per-project gating replaced by explicit project binding (RFC-025) |
| `instructions` | **kept** | custom system-prompt text — the real reason a Project exists |

### A3. Remove the legacy runtime fallback

- **Delete** `orchestrator.rs:326-334` (the `mount_ids.is_empty() && !paths.is_empty()` branch). After backfill, no project has `paths`, so the branch is dead.
- **Delete** the `config.project_paths[0]` fallback in `agent_runtime.rs:730-733`. The config field `AgentConfig.project_paths` (`agent_runtime.rs:106-107`) is removed; `mount_paths` is the only CWD source.
- **Rewrite** `token_maxing/planner.rs:98`: resolve `mount_paths` from the project's `mount_ids` (load each mount, flatten `mount.paths`) instead of `proj.paths.clone()`.

### A4. Redo the create flow

`create-project-dialog.tsx` becomes minimal — it collects only what a Project uniquely contributes:

```
Name
Emoji (auto-suggested from name; editable)
Instructions (textarea — custom system-prompt text)
[ Mount drop-zone — attach mounts here, see Pillar C ]
```

No `paths`, `description`, `tags`, or `memory_visible` inputs. The mount drop-zone replaces the old `paths` textarea entirely.

---

## Pillar B — Mount as the Addressable Concept (Unified `@`)

**Goal:** one `@` picker; Mounts, Knowledge notes, and Memories all discoverable; binding routed correctly per source.

### B1. A single `ContextSource` discriminant

Replace the hard-coded `'knowledge' | 'memory'` union with one type spanning all three sources:

```ts
type ContextSourceKind = 'mount' | 'knowledge' | 'memory'

interface ContextSource {
  kind: ContextSourceKind
  id: string          // mount id | note path | memory id
  label: string
  snippet?: string
  score?: number
}
```

`MentionResult` and `ContextAttachment` both become `ContextSource`-shaped. This is a widening of the existing types, not a third sibling — the advisory's "single discriminant, not a 3rd ad-hoc type."

### B2. Unified search

The `@`-search debouncer (`chat-input.tsx:116-172`) fires **three** queries in parallel and merges:

| Source | Endpoint | Already exists? |
|---|---|---|
| Mount | `GET /api/mounts?search=q` | ✅ `mount_routes.rs:71` |
| Knowledge | `POST /api/knowledge/search` | ✅ `knowledge_routes.rs:704` |
| Memory | `POST /api/memory/semantic` | ✅ `workspace.rs:1147` |

Merge ordering: Mounts first (they're the heaviest/most intentional context), then Knowledge, then Memory, with `score` breaking ties within a kind. Cap at 8, one row each, distinct icon + kind tag per source (`HardDrive` / `FileText` / `Brain`).

### B3. Unified picker, split binding (the critical mechanism)

This is where the design diverges from "just add a type." Selecting a source routes down **two different paths** depending on `kind`:

```mermaid
flowchart LR
  P["@ picker<br/>(unified UI)"] --> S{kind?}
  S -->|mount| B["setActiveMountIds<br/>session-sticky bind"]
  S -->|knowledge| K["onSend content<br/>+ [context:knowledge:id]"
  S -->|memory| M["onSend content<br/>+ [context:memory:id]"]
  B --> O["orchestrator<br/>resolve_mount_workspace:<br/>CWD + allowed_paths<br/>+ ## Workspace Context"]
  K --> O
  M --> O
```

**Why the split is mandatory.** Knowledge/memory `@` today ride the **text-token path**: `onSend(content, contextItems)` serializes them as inline refs like `[context:knowledge:${id}]` (per-message, content-only). A Mount does not work that way — it must ride the **session-binding path**: `setActiveMountIds` → sent as `mount_ids` → the orchestrator's `resolve_mount_workspace` grants CWD + `allowed_paths` + the `## Workspace Context` prompt block (`agent_runtime.rs:728-791`). If `@mount` were bolted onto the text path, the agent would *see the mount's name as text but gain no file access* — the feature would look correct and silently fail.

So the picker is one UI; the binding splits by source:

- **`@mount`** → `useChatStore.setActiveMountIds([...])`. Session-sticky: the chip persists across turns (it's a binding, not an injection). Same store field the existing mount sidebar uses, just a faster entry point.
- **`@knowledge` / `@memory`** → appended to the per-message `contextItems` → `onSend`. Cleared after send (unchanged from today).

**Lifecycle decision: `@mount` is session-sticky, not one-shot.** This matches the user's framing — "마운트는 등록되는 순간부터 참조 가능한 하나의 개념" (once registered, a mount is a single concept referenceable anywhere). A one-shot grant would cause permission/CWD churn between turns (bind → unbind → rebind) and contradict the persistent-binding model RFC-025 already established. Sticky it is.

### B4. Precedence: explicit `@mount` beats auto-detection

RFC-025's sticky-primary auto-detection (`detect_mounts`) still runs on each message. Adding `@mount` creates a conflict risk: the detection badge and the `@`-chip would fight over the primary slot. Resolution:

| Situation | Behavior |
|---|---|
| User `@mount`-attached ≥1 mount | Those mounts ARE the authoritative binding for the session. **Detection is suppressed** for this message — no badge, no secondary auto-add. The user spoke explicitly. |
| No `@mount` attached, session has binding | Existing sticky-primary behavior (unchanged). |
| No `@mount` attached, no binding | Detection seeds primary (unchanged). |

The rule: **explicit attachment is authoritative; detection only seeds the empty case.** This is a one-line gate at the top of the detection path: if the incoming message carries explicitly-attached mount ids (or the session already has a user-set binding this turn), skip `detect_mounts` entirely. Detection never overrides an explicit `@mount`.

### B5. The "two memories" framing

The unifying mental model this design surfaces to the user:

| Concept | What it is | Addressable via |
|---|---|---|
| **Mount** | the *filesystem* memory — a path + its agent-explored description | `@` in chat, drag onto Project, sidebar binding |
| **Knowledge Base** | the *document* memory — user-written/editable `.md` notes | `@` in chat, knowledge editor, `[[wikilinks]]` |
| Agent Memory | the *episodic/semantic* memory — auto-consolidated facts | `@` in chat |

All three are editable, viewable, and referenceable. The user edits Mounts (name/path) and Knowledge notes directly; Agent Memory is managed by the system. This framing is what makes "register a mount once, reference it everywhere" feel true — because the same `@` gesture reaches all three.

---

## Pillar C — Drag-and-Drop Mount Attachment

**Goal:** attach Mounts to Projects (and to the active chat) by dragging. **Frontend-only** — the relationship (`project.mount_ids`) and mutation (`update_project_bundle`) already exist; the edit-project chips already mutate the same field. No backend change.

### C1. Drag source

Each Mount card (`web/src/components/mount/`) becomes `draggable`. On `onDragStart`, stash `{ kind: 'mount', id, name }` in the `DataTransfer`. No library — native HTML5 DnD matches the shadcn/Tailwind stack.

### C2. Drop targets

| Target | Action on drop |
|---|---|
| Project card (project management) | `updateProjectBundle({ mount_ids: [...current, dropped] })` — reuses `useUpdateProject` |
| Active chat composer / session header | `setActiveMountIds([...current, dropped])` — reuses the same store path as `@mount` |
| Mount drop-zone inside project create/edit dialog | same `mount_ids` mutation, inline |

A small `useMountDropZone({ onDrop })` hook centralizes the `onDragOver`/`onDrop`/highlight logic. Visual: drop targets get a dashed primary ring + "Drop to attach" label on `dragover`.

### C3. Remove the dual edit UI

The edit-project dialog already has mount chip-toggles (`edit-project-dialog.tsx:173-189`). Pillar C keeps those AND adds the drop-zone to the same dialog, so both click-toggle and drag work on one `mount_ids` field. The old `paths` inputs disappear entirely (Pillar A4).

---

## Migration & Rollout

Single release, ordered:

1. **Backfill (A1)** — `ProjectManager::load` migrates `paths → mount_ids`. Ship this first; it's backward-compatible (old field still tolerated until step 4).
2. **Rewire consumers (A3)** — planner resolves from `mount_ids`; orchestrator/runtime legacy branches deleted.
3. **Frontend cutover (A4, B, C)** — new create dialog, unified `@`, DnD. The web type drops `paths`/`description`/`tags`/`emoji`/`memory_visible`.
4. **Field removal** — delete `Project.paths` and the RFC-011 leftovers from backend struct + DTO. This is the irreversibility point; step 1 must have run for all users first.

**Risk:** a user with a pre-release daemon that never ran the backfill, upgrading the binary directly to step 4. Mitigation: the backfill is idempotent and runs on every load *before* the field is removed in step 4 — so any project still carrying `paths` at step-4 load time is migrated then. The `paths` column in SQLite is retained (unused) for one release as a rollback cushion, then dropped.

---

## Out of Scope

- **`@mount` inside the Knowledge markdown editor.** The editor already has `[[wikilink]]` autocomplete; cross-linking into mounts is a separate, smaller feature.
- **Changing the Mount data model.** `Mount` is already correct per RFC-025. The bug is entirely on the `Project` side.
- **Knowledge↔Memory convergence.** Both stay as separate editable stores; this design only unifies their *addressability*, not their storage.
- **Permissions UX for `@mount`.** Granting a mount via `@` uses the existing per-session permission derivation (`agent_runtime.rs:788-791`); no new permission surface.

---

## Acceptance

- A Project created via the new dialog has `mount_ids` set and `paths` absent at every layer.
- Dragging a Mount onto a Project attaches it (one `mount_ids` mutation, no `paths` written).
- Typing `@` in chat shows Mounts alongside Knowledge and Memory; selecting a Mount binds it to the session (chip persists, agent gains file access — verified by the workspace-context block appearing and a path-gated tool succeeding).
- With a Mount explicitly `@`-attached, no detection badge appears for that message.
- `cargo test --workspace` green; the `test_resolve_mount_workspace_*` tests still pass after the legacy-fallback deletion.
