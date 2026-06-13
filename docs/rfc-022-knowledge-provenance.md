# RFC-022: Knowledge Provenance, Quality & Dream Curation

> **Status:** Draft
> **Created:** 2026-06-13
> **Depends on:** RFC-016 (autonomous persistence), RFC-008 (memory consolidation / Dream)

## Problem

Agent-saved knowledge notes are indistinguishable from user-written notes. This causes:

1. **Noise** — agent responses contain conversational artifacts ("이렇게 정리해봤어요. 어떤가요?") that get saved verbatim.
2. **No cleanup path** — Dream only processes memory (SQLite). Knowledge files (markdown) are invisible to Dream.
3. **No curation feedback loop** — user can't see "this was auto-saved, you may want to review."
4. **No dedup signal** — hook + tool + UI can all save the same content with no provenance.

## Design Overview

```
Agent execution
  │
  ├─ PersistenceHook heuristic ──→ note_write_with_meta(author: agent, quality: raw, needs_review: true)
  ├─ PersistenceHook reflection ──→ note_write_with_meta(author: agent, quality: raw, needs_review: true)
  │                                    (LLM strips conversational wrapping during reflection)
  ├─ Tool-calling (knowledge write) ──→ note_write_with_meta(author: agent, source: tool)
  └─ UI button ──→ note_write_with_meta(author: agent, source: ui)
        │
        ▼
  ┌─────────────────────────────┐
  │  ~/.oxios/knowledge/        │
  │  ├── notes/                 │
  │  │   ├── rust-design.md     │ ← frontmatter: { author: agent, quality: raw, needs_review: true }
  │  │   └── journal.md         │ ← no frontmatter (user-written)
  │  └── research/              │
  └─────────┬───────────────────┘
            │
            ▼  (periodic)
  ┌─────────────────────────────┐
  │  KnowledgeDream             │
  │  Phase 1: Scan raw notes    │
  │  Phase 2: LLM curate        │
  │  Phase 3: Write back        │
  │  Phase 4: Report            │
  └─────────────────────────────┘
            │
            ▼  quality: raw → curated
```

## Part A: Provenance Frontmatter

### A1. Schema

Every knowledge note written by the agent gets a YAML frontmatter block.
User-written notes (no frontmatter, or `author: user`) are never modified by the system.

```yaml
---
oxios:
  author: agent
  source: hook           # hook | tool | ui | dream
  session_id: abc123
  message_index: 3
  saved_at: "2026-06-13T14:30:00Z"
  quality: raw           # raw | curated | refined
  needs_review: true
---
```

**Field reference:**

| Field | Type | Values | Meaning |
|-------|------|--------|---------|
| `author` | string | `agent`, `user` | Who created this note. Absent = user. |
| `source` | string | `hook`, `tool`, `ui`, `dream` | How the save was triggered. |
| `session_id` | string? | UUID | Originating session. Null for Dream-curated. |
| `message_index` | int? | 0-based | Message in the session. Null for Dream. |
| `saved_at` | ISO 8601 | timestamp | When the note was first saved. |
| `quality` | string | `raw`, `curated`, `refined` | Content quality stage. |
| `needs_review` | bool | true/false | Whether Dream should process this note. |

**Transition rules:**
- `quality` only moves forward: `raw → curated → refined`. Never backward.
- `needs_review` is set to `true` on all agent saves. Dream sets `false` after curation.
- User can manually set `needs_review: false` to opt out of Dream processing.

### A2. Three Sources → Three Frontmatter Values

| Source | `source` | `quality` | `needs_review` | Content sanitization |
|--------|----------|-----------|----------------|---------------------|
| PersistenceHook heuristic | `hook` | `raw` | `true` | None — saved as-is |
| PersistenceHook reflection | `hook` | `raw` | `true` | LLM strips conversational wrapping |
| Agent tool-calling (`knowledge write`) | `tool` | `raw` | `true` | None — agent explicitly wrote it |
| UI "지식에 저장" button | `ui` | `raw` | `true` | None — user chose to save |
| User manual edit | absent | absent | absent | N/A |
| Dream curation | `dream` | `curated` | `false` | LLM-refined |

### A3. KnowledgeBase API

```rust
// Existing — unchanged. Used for user writes and internal operations.
pub fn note_write(&self, path: &str, content: &str) -> Result<()>

// New — for agent-originated writes.
pub fn note_write_with_meta(&self, path: &str, content: &str, meta: &NoteMeta) -> Result<()>

// New — parse frontmatter, returning (metadata, body).
pub fn parse_note_meta(content: &str) -> (Option<NoteMeta>, String)

// New — list notes needing Dream review.
pub fn notes_needing_review(&self) -> Result<Vec<(String, NoteMeta)>>
```

```rust
/// Provenance metadata for agent-originated knowledge writes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteMeta {
    pub source: NoteSource,
    pub quality: NoteQuality,
    pub needs_review: bool,
    pub session_id: Option<String>,
    pub message_index: Option<usize>,
    pub saved_at: Option<String>,  // ISO 8601
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NoteSource {
    Hook,
    Tool,
    Ui,
    Dream,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum NoteQuality {
    Raw,
    Curated,
    Refined,
}
```

`note_write_with_meta` prepends the YAML frontmatter block, then delegates to `note_write`.

`parse_note_meta` extracts and strips the `---` block, returning `(Some(NoteMeta), body)`.
For files without frontmatter, returns `(None, original_content)`.

The backlink indexer and search indexer already skip `---` delimited blocks.

### A4. Content Sanitization — Reflection Path Only

The PersistenceHook reflection LLM already has the full context. The reflection prompt gains:

> "When saving to knowledge, strip conversational wrapping: greetings, sign-offs, questions to the user, hedging phrases. Extract only substantive content."

This is language-agnostic because the LLM understands context. No regex patterns needed.

The heuristic path does **not** sanitize. Raw output saved as-is. Dream cleans later.

### A5. Changes to Existing Code

**`persistence_hook.rs`:**
- `KnowledgeWrite` gains `meta: NoteMeta` field.
- Heuristic path: `NoteMeta { source: Hook, quality: Raw, needs_review: true, session_id, message_index, saved_at }`.
- Reflection path: same `NoteMeta`, but LLM returns already-sanitized content.
- `execute_plan()` calls `knowledge_base.note_write_with_meta()` instead of `note_write()`.

**`knowledge_tool.rs`:**
- `"write"` action wraps with `NoteMeta { source: Tool, quality: Raw, needs_review: true }`.

**`chat.rs` (web API):**
- `handle_save_to_knowledge()` wraps with `NoteMeta { source: Ui, quality: Raw, needs_review: true }`.

---

## Part B: Knowledge Dream

### B1. Architecture

Knowledge Dream is a separate process from Memory Dream. It runs on the same schedule
(triggered by `DreamProcess` or independently) but operates on the markdown knowledge vault
instead of the SQLite memory store.

```
DreamProcess (memory)          KnowledgeDream (knowledge)
     │                              │
     ├─ Orient                      ├─ Scan
     ├─ Gather Signal               ├─ Curate
     ├─ Consolidate                 ├─ Write Back
     └─ Prune & Index               └─ Report
```

### B2. Why Not Extend DreamProcess?

`DreamProcess` lives in `oxios-memory` crate. `KnowledgeBase` lives in `oxios-markdown` crate.
They have no dependency on each other, and shouldn't — they're orthogonal storage systems.

Instead, `KnowledgeDream` is a standalone struct in `oxios-kernel` (which depends on both crates)
and is invoked alongside `DreamProcess` from the kernel's dream scheduling logic.

### B3. KnowledgeDream

```rust
pub struct KnowledgeDream {
    knowledge_base: Arc<KnowledgeBase>,
    git_layer: Arc<GitLayer>,
    engine_handle: Arc<EngineHandle>,
    model_id: String,
    config: KnowledgeDreamConfig,
}

pub struct KnowledgeDreamConfig {
    /// Enable/disable knowledge dream.
    pub enabled: bool,
    /// Minimum number of raw notes before triggering.
    pub min_raw_notes: usize,
    /// Maximum notes to curate per dream run.
    pub batch_size: usize,
    /// Whether to auto-curate or just flag.
    pub auto_curate: bool,
}

pub struct KnowledgeDreamReport {
    pub dream_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub notes_scanned: usize,
    pub notes_curated: usize,
    pub notes_skipped: usize,
    pub errors: Vec<String>,
    pub duration_ms: u64,
}
```

### B4. Phase 1: Scan

```rust
async fn scan(&self) -> Result<Vec<RawNote>>
```

1. Call `knowledge_base.notes_needing_review()`.
2. Filter to `quality == Raw` and `needs_review == true`.
3. Sort by `saved_at` (oldest first — they've been raw the longest).
4. Take up to `batch_size`.
5. Return `Vec<RawNote>` with `(path, meta, body)`.

### B5. Phase 2: Curate

```rust
async fn curate(&self, notes: &[RawNote]) -> Result<Vec<CuratedNote>>
```

For each note, invoke the LLM with a curation prompt:

```
You are a knowledge editor. Your job is to refine a raw agent-generated note into
a clean, well-structured knowledge document.

Rules:
- Remove conversational artifacts: greetings, sign-offs, hedging, questions to the user.
- Keep all substantive content: facts, analysis, code, data, explanations.
- Improve structure if needed: add headers, organize sections.
- Preserve the original meaning. Do not add new information.
- Output only the cleaned markdown body. No frontmatter.

Original note:
---
{body}
```

Each note is an independent LLM call. Failed curations are logged and skipped.

### B6. Phase 3: Write Back

```rust
async fn write_back(&self, curated: &[CuratedNote]) -> Result<usize>
```

For each curated note:
1. Build new `NoteMeta` with `quality: Curated`, `needs_review: false`, `source: Dream`.
2. Keep `session_id`, `message_index`, `saved_at` from original for traceability.
3. Call `note_write_with_meta(path, curated_body, &meta)`.

The original content is overwritten in-place. The file path stays the same.
Backlinks and search index are updated automatically by `note_write`.

### B7. Phase 4: Report

Generate `KnowledgeDreamReport` and save to `~/.oxios/knowledge/.dream_reports/{dream_id}.json`.

### B8. Scheduling

`KnowledgeDream` is invoked from the same place `DreamProcess` is spawned — `src/kernel.rs`:

```rust
// Existing memory dream
if consolidation.dream_enabled {
    let dream = Arc::new(DreamProcess::new(...));
    dream.spawn_dream_task();
}

// New: knowledge dream
if config.knowledge_dream.enabled {
    let kd = Arc::new(KnowledgeDream::new(
        knowledge_base.clone(),
        engine_handle.clone(),
        config.knowledge_dream.clone(),
    ));
    kd.spawn();
}
```

It runs on the same interval as memory dream (default: every 24 hours, after N sessions).

### B9. Quality Lifecycle

```
Agent saves note ──→ quality: raw, needs_review: true
         │
         ▼  (Dream pass 1)
  quality: curated, needs_review: false
  (conversational artifacts removed, structure improved)
         │
         ▼  (Dream pass 2 — future)
  quality: refined, needs_review: false
  (cross-referenced, merged with related notes, enriched)
```

Dream only processes `needs_review: true` notes. Once curated, a note is stable unless
the user explicitly sets `needs_review: true` again.

### B10. Safety

- **User notes are never touched.** `notes_needing_review()` only returns notes with `author: agent`.
- **Failed curations are skipped.** The original raw note stays intact.
- **Dream is rate-limited.** `batch_size` prevents runaway LLM costs.
- **Dream can be disabled.** `knowledge_dream.enabled = false` in config.
- **Manual opt-out.** User can set `needs_review: false` on any note.

---

## Part C: Web UI

### C1. Knowledge Tree Provenance Badges

The file tree in the Knowledge panel shows a small icon per file, using lucide-react icons:

| Quality | Icon | Label | Style |
|---------|------|-------|-------|
| absent (user) | none | — | — |
| `raw` | `Bot` | 자동 저장 | `text-muted-foreground` |
| `curated` | `Sparkles` | 정리됨 | `text-green-600` |
| `refined` | `Gem` | 정제됨 | `text-blue-600` |

### C2. Editor Header

When viewing an agent-generated note, the editor shows a header bar:

```
[Bot icon] 에이전트가 저장 · 2026-06-13 · 품질: raw
[다시 정리] [삭제]
```

Icons: `Bot` (source), `RefreshCw` (다시 정리), `Trash2` (삭제).
"다시 정리" sets `needs_review: true` and triggers curation on next Dream pass.

### C3. Save Indicator Update

The existing `knowledge-save-indicator.tsx` already shows source info.
The `KnowledgeSaveRecord` gains a `quality` field from the frontmatter.

---

## Design Review

### Defect 1: Backlink indexer does NOT skip `---` blocks

**Claim in A3:** "The backlink indexer and search indexer already skip `---` delimited blocks."

**Reality:** `backlinks.rs:index_file()` iterates every line of content with no frontmatter awareness.
If frontmatter contains `[[SomeNote]]` or `[link](target.md)`, it would create spurious backlinks.

**Fix:** `index_file()` must strip the frontmatter block before processing. Add a `strip_frontmatter(content) -> (Option<&str>, &str)` helper that detects the opening `---\n` and finds the closing `---\n`, returning the body only. Apply in `index_file()` before link extraction.

### Defect 2: `notes_needing_review()` vault scan cost

**Problem:** `notes_needing_review()` needs to walk the directory tree, read every `.md` file, and parse frontmatter. For large vaults this could be slow.

**Assessment:** Dream runs every 24 hours. Scanning 500 files and parsing frontmatter (read first ~10 lines only) takes < 1 second. A separate index file (`meta_index.json`) would introduce race conditions on concurrent writes, staleness when files are modified externally, and sync bugs on `note_move`/`note_delete`.

**Fix:** No index file. `notes_needing_review()` scans files directly, but optimizes by reading only the frontmatter block (stop reading after the closing `---`). This is a simple, correct, and fast enough approach. If vault size ever exceeds 10,000 notes, revisit.

### Defect 3: Overwrite in-place loses original content

**Problem:** Phase 3 (Write Back) overwrites the raw note with curated content. If the LLM hallucinates or removes important content during curation, the original is lost.

**Fix:** KnowledgeDream holds an `Arc<GitLayer>` reference. Before each write-back, call `git_layer.commit_file(path, "dream: pre-curation snapshot")`. This creates a git commit with the raw content. Users can restore via `git checkout` or the Web UI git history view.

The knowledge vault root (`~/.oxios/knowledge/`) is already git-initialized when `GitLayer` is enabled (default). No separate backup directory needed.

### Defect 4: `oxios-markdown` lacks `serde_yaml` dependency

**Problem:** `NoteMeta` types and `parse_note_meta()` live in `oxios-markdown`, but the crate has no `serde_yaml` dependency. Currently only `oxios-kernel` has it.

**Fix:** Add `serde_yaml = "0.9"` to `oxios-markdown/Cargo.toml`. This is a lightweight, well-maintained dependency. The alternative — moving frontmatter parsing to `oxios-kernel` — would create a leaky abstraction where `KnowledgeBase` methods return raw content but callers must handle frontmatter separately.

### Defect 5: No checkpoint for KnowledgeDream

**Problem:** Memory Dream has a checkpoint mechanism (`DreamCheckpoint`) for crash recovery. If KnowledgeDream crashes mid-curation (e.g., after curating 3 of 10 notes), it has no way to resume. On next run, it would re-curate the already-curated notes.

**Fix:** `notes_needing_review()` filters by `quality: raw`, so already-curated notes (now `quality: curated`) are naturally skipped on re-scan. This is sufficient — no checkpoint needed. The idempotency comes from the quality field itself, not from checkpoint state.

~~Defect 5 resolved by design.~~ No fix needed.

### Defect 6: `note_write_with_meta()` on existing frontmatter

**Problem:** What happens if a note already has frontmatter (e.g., Dream curated it once, then the agent saves to the same path again)? `note_write_with_meta` should merge or replace, not double-prepend.

**Fix:** `note_write_with_meta()` must call `parse_note_meta()` on existing content first. If frontmatter exists, merge: preserve `saved_at` from original, update `quality` to the new value, set `source` to the new source. If no frontmatter exists (user note), do NOT add frontmatter — this would be user content being overwritten by an agent save, which should be a no-op.

### Defect 7: User-written frontmatter (Obsidian-style)

**Problem:** Users may have existing notes with YAML frontmatter (tags, aliases, etc.) that has nothing to do with Oxios. `parse_note_meta()` must not misinterpret user frontmatter as Oxios metadata.

**Fix:** `parse_note_meta()` looks specifically for the `oxios:` key inside the frontmatter. If the `---` block exists but contains no `oxios:` key, treat it as user frontmatter — return `(None, original_content)` (return full content including the user's frontmatter). Only extract `NoteMeta` when the `oxios:` key is present.

### Defect 8: KnowledgeTool doesn't know session context

**Problem:** `KnowledgeTool::execute()` has no access to `session_id` or `message_index`. The tool runs inside `oxi_sdk`'s tool-calling loop, which doesn't pass session context.

**Fix:** Set `session_id` and `message_index` to `None` in tool-originated writes. The tool path is explicit user intent, so traceability to a specific message is less critical than for hook saves. `saved_at` timestamp provides sufficient provenance.

---

## Scope

**In scope (this RFC):**

| Component | Location | Change |
|-----------|----------|--------|
| `NoteMeta`, `NoteSource`, `NoteQuality` | `oxios-markdown/src/types.rs` | New types |
| `serde_yaml` dependency | `oxios-markdown/Cargo.toml` | Add |
| `note_write_with_meta()` | `oxios-markdown/src/knowledge.rs` | New method |
| `parse_note_meta()` | `oxios-markdown/src/knowledge.rs` | New method |
| `notes_needing_review()` | `oxios-markdown/src/knowledge.rs` | New method (direct scan, no index) |
| `strip_frontmatter()` helper | `oxios-markdown/src/backlinks.rs` | Frontmatter skip in index_file |
| `KnowledgeWrite.meta` | `oxios-kernel/src/persistence_hook.rs` | Add field |
| Hook heuristic: meta injection | `oxios-kernel/src/persistence_hook.rs` | Modified |
| Hook reflection: prompt + meta | `oxios-kernel/src/persistence_hook.rs` | Modified |
| Tool write: meta injection | `oxios-kernel/src/tools/builtin/knowledge_tool.rs` | Modified |
| UI save: meta injection | `surface/oxios-web/src/routes/chat.rs` | Modified |
| `KnowledgeDream` | `oxios-kernel/src/knowledge_dream.rs` | New file |
| `KnowledgeDreamConfig` | `oxios-kernel/src/config.rs` | New section |
| Kernel scheduling | `src/kernel.rs` | Spawn knowledge dream |
| Web UI badges | `web/src/components/knowledge/` | Tree + editor indicators |
| i18n | `locales/ko.json`, `locales/en.json` | New keys |

**Out of scope:**
- `quality: refined` Dream pass (future — requires cross-note merging)
- Frontmatter editing UI
- Knowledge dedup during Dream
