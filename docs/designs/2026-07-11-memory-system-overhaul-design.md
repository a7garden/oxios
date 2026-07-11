# Memory System Overhaul — Design

> **Date**: 2026-07-11
> **Status**: Proposed
> **Scope**: `oxios-kernel`, `oxios-memory`
> **Reference**: [mnemopi](https://github.com/can1357/oh-my-pi/tree/master/packages/mnemopi) (oh-my-pi's memory engine)

## 1. Problem

The memory system has the right concepts (PersistenceHook auto-extraction, SQLite+FTS5+sqlite-vec, HNSW, tiered decay, Dream consolidation) but the end-to-end pipeline is broken in four places. A user chatting via Web UI gets **zero cross-session memory**.

### Evidence

| Metric | Value | Expected |
|--------|-------|----------|
| Total memories in `memory.db` | 8 | Growing per conversation |
| `fact` / `episode` entries | 0 | Extracted from conversations |
| `conversation` (compaction) entries | 0 | One per compacted session |
| Vector index entries | **0** | One per text entry |
| Sources | All `knowledge:lens` | Mix of `persistence-hook`, `agent`, `compaction` |

### Root Causes

#### RC-1: Dead vector index — TF-IDF produces sparse vectors that can't enter sqlite-vec

```
TfIdfEmbeddingProvider → EmbeddingVector::Sparse(HashMap<String, f64>)
EmbeddingVector::to_f32_dense() → None for Sparse  (embedding.rs:99-106)
SqliteMemoryStore::remember() line 156:
    if let Some(f32_vec) = embedding_vec.to_f32_dense()  // always None
        → vector insert silently skipped
```

Every `remember()` call stores text + FTS5 index but **never inserts a vector**. `search()` falls back to BM25-only keyword matching. Semantic recall is dead.

#### RC-2: Split backends — oxi-agent tools write to a void

| Surface | Backend | Connected? |
|---------|---------|-----------|
| Auto-recall (`agent_runtime.rs:425`) | `MemoryManager` → SQLite | ✅ |
| PersistenceHook (`persistence_hook.rs:211`) | `MemoryManager` → SQLite | ✅ |
| Compaction handler (`agent_runtime.rs:1606`) | `MemoryManager` → SQLite | ✅ |
| OLD tools `memory_read/write/search` (CSpace-gated) | `MemoryManager` → SQLite | ✅ |
| **NEW tools** `MemoryRecall/Retain/Edit/Reflect` (unconditional) | `AgentConfig.memory` = `NoopMemoryStore` | ❌ |

The NEW tools (`builtin/mod.rs:63-66`) are registered unconditionally but `AgentConfig` at `agent_runtime.rs:935` leaves `..Default::default()`, so `ctx.memory` resolves to `NoopMemoryStore`. Writes silently succeed and vanish. The RFC-034 "OxiosMemoryBackend bridge" was planned (`builtin/mod.rs:61-62` comment) but **never implemented** — `OxiosMemoryBackend` does not exist in the codebase.

#### RC-3: PersistenceHook reflection produces nothing

The hook IS attached (`src/kernel.rs:1345`), fires post-execution (`agent_runtime.rs:654`, gated on `success && Some(hook)`), and receives `Some(directive)` (always passed at `agent_runtime.rs:309`). But `reflect()` (`persistence_hook.rs:295-388`) builds a lightweight LLM agent to extract facts from the conversation output, and either:
- The LLM call fails (model resolution, API key, network) → `Err` → logged as `warn`, plan returned with empty `memory: []`
- The LLM returns empty JSON `{"memory":[],"knowledge":[]}` → nothing persisted

DB confirms: 0 entries with `source = persistence-hook`.

#### RC-4: System prompt advertises disconnected tools

`agent_runtime.rs:1660`:
> Memory tools: memory_read, memory_write, memory_search

But the agent also sees `memory_retain`, `memory_recall`, `memory_edit`, `memory_reflect` (NEW, from oxi-agent). The NEW tools write to `NoopMemoryStore`. The agent has 7 memory tools from two incompatible backends.

#### RC-5: Default web-chat agent has NO memory capability

The default persona "Dev" has `role: "developer"`. The cspace resolver (`capability/resolve.rs:80-103`) maps known role names (`worker`/`standard`/`operator`/`supervisor`) to templates; `"developer"` is **unknown** → falls back to `worker()`.

```
resolve_cspace(None, Some("developer"), Some("worker"), agent_id)
  → "developer" not in [worker, standard, operator, supervisor]
  → fallback: worker()
  → worker(): Exec(shell) + Browser only
  → NO Memory capability (not even READ)
```

The worker template grants zero memory rights. The CSpace-gated registration (`registration.rs:167`) checks `KernelDomain { "memory" }` — which the worker lacks — so `memory_read/write/search` are **never registered** for the default web-chat agent.

Today this is masked by the 4 dead oxi-agent tools (registered unconditionally). Removing them (Phase 1) would leave the agent with **zero memory tools** — not even read access.

## 2. Design Decisions

### Decision 1: Consolidate on MemoryManager — unregister dead tools, register working tools unconditionally

**Choice**:
1. Remove `oxi_agent::{MemoryRecallTool, MemoryRetainTool, MemoryEditTool, MemoryReflectTool}` from `register_all_kernel_tools`.
2. Register `MemoryReadTool`, `MemorySearchTool`, `MemoryWriteTool` **unconditionally** in `register_all_kernel_tools` — same location where the dead tools were.
3. Remove the CSpace-gated memory registration from `registration.rs` (now redundant).

**Rationale**:
- Oxios is a personal agent OS, not a multi-tenant sandbox. Every agent needs memory access — read and write. The CSpace gate was designed for a threat model (untrusted sub-agents) that doesn't apply to the default chat path.
- The dead oxi-agent tools were already unconditional. Replacing them with unconditional MemoryManager-backed tools restores the same availability with a working backend.
- `MemoryManager` is the richer backend: SQLite + FTS5 + sqlite-vec + HNSW + tiers + decay + Dream.
- `oxi-sdk`'s `MemoryStore` trait (`put`/`search`/`list`) is a strict subset. Bridging would downgrade capability.

**Rejected alternatives**:
- *Implement `OxiosMemoryBackend: oxi_sdk::MemoryStore`*: More code, two tool namespaces, no user benefit.
- *Grant Memory(WRITE) in `standard()` template*: Doesn't fix the root cause — the default persona role `"developer"` falls through to `worker()`, bypassing `standard()` entirely. Would need either mapping `"developer"` → `standard` or changing the default role, both fragile.
- *Register memory tools in `worker()`*: Pollutes the capability template with domain-specific tools. Tools belong in the tool registry, not the capability template.

### Decision 2: New API-based EmbeddingProvider — not GGUF, not ONNX

**Choice**: Implement `ApiEmbeddingProvider` — an `EmbeddingProvider` that calls an OpenAI-compatible `/v1/embeddings` endpoint. Wire it as the default when an API key is available; fall back to `TfIdfEmbeddingProvider` when offline.

**Rationale**:
- GGUF (`embedding-gguf` feature) is `cfg(target_arch = "aarch64")` only + 329MB download + off by default. Not cross-platform.
- ONNX/fastembed (mnemopi's local path) adds a heavy native dependency. Premature.
- API-based is zero-dependency (reqwest is already in the tree), works on all platforms, and the infrastructure already anticipates it — HNSW default dimension is commented "OpenAI text-embedding-3-small (1536)" (`hnsw.rs`), and the model catalog can resolve embedding models.
- The user already has API keys configured for LLM providers. Reusing the same key/endpoint for embeddings is the path of least resistance.

**Design**:
```rust
/// API-based embedding provider (OpenAI-compatible /v1/embeddings).
///
/// Calls a remote embedding endpoint, caches results in the existing
/// embedding_cache table, and produces DenseF32 vectors for sqlite-vec + HNSW.
pub struct ApiEmbeddingProvider {
    client: reqwest::Client,
    endpoint: String,       // e.g. "https://api.openai.com/v1/embeddings"
    api_key: String,
    model: String,          // e.g. "text-embedding-3-small"
    dimensions: usize,      // e.g. 1536
}
```

- Config: `[embedding]` section in `config.toml` (endpoint, api_key, model, dimensions).
- Resolution at boot: if config present → `ApiEmbeddingProvider`; else if `embedding-gguf` feature + aarch64 → `GgufEmbeddingProvider`; else → `TfIdfEmbeddingProvider`.
- The provider plugs into `MemoryManager` via the existing `Arc<dyn EmbeddingProvider>` field — **zero changes to MemoryManager or SqliteMemoryStore**.

### Decision 3: Fix PersistenceHook reflection — not redesign

**Choice**: Debug and fix the existing `reflect()` path. The design is sound (mnemopi-equivalent auto-extraction); the implementation is broken at runtime.

**Steps**:
1. Add structured tracing to `reflect()`: log model_id resolved, agent build result, LLM response length, JSON parse result.
2. Verify `engine.default_model_id()` resolves to a working model.
3. If LLM returns non-JSON or empty, add a heuristic fallback (like mnemopi's `heuristicExtractFacts`): extract sentences containing preference/fact indicators ("I prefer", "always use", "the user wants") as fact entries.
4. Lower the bar: currently `reflect()` outputs JSON with `memory` array. If empty, try again with a more aggressive prompt that explicitly asks "What did you learn about the user?"

**Rejected alternative**: Rewrite the extraction as a streaming pipeline that runs after every message (like mnemopi's `remember(extract: true)` per turn). Too invasive — the PersistenceHook's post-execution design is fine; it just needs to produce output.

### Decision 4: System prompt alignment

**Choice**: Update the system prompt at `agent_runtime.rs:1660` to advertise `memory_read/write/search` and add guidance on when to use them.

## 3. Implementation Plan

### Phase 1: Backend consolidation + tool registration (low risk, immediate)

| File | Change |
|------|--------|
| `tools/builtin/mod.rs:61-66` | Replace 4 `oxi_agent::Memory*Tool` registrations with `MemoryReadTool::from_kernel`, `MemorySearchTool::from_kernel`, `MemoryWriteTool::from_kernel` (all unconditional) |
| `tools/registration.rs:166-175` | Remove the CSpace-gated `"memory"` branch (now redundant — tools registered unconditionally) |
| `tools/registration.rs:249-260` | Same removal in the second registration path |
| `agent_runtime.rs:1660-1661` | Update system prompt to match actual tool names |

**Before** (current):
```rust
// builtin/mod.rs:61-66 — dead tools, unconditional
registry.register(oxi_agent::MemoryRecallTool);
registry.register(oxi_agent::MemoryRetainTool);
registry.register(oxi_agent::MemoryEditTool);
registry.register(oxi_agent::MemoryReflectTool);
```

**After**:
```rust
// builtin/mod.rs — working tools, unconditional
registry.register(MemoryWriteTool::from_kernel(kernel));
registry.register(MemoryReadTool::from_kernel(kernel));
registry.register(MemorySearchTool::from_kernel(kernel));
```

**Why unconditional, not CSpace-gated**: The default web-chat agent runs as `worker` (persona role `"developer"` falls through to worker template, which grants zero memory rights). CSpace gating would leave the default agent without memory tools. Memory is a core capability in a personal agent OS — not a privilege.

**Verification**:
1. `cargo build -p oxios-kernel`
2. `oxios run --json "remember that I prefer dark mode"` → agent calls `memory_write` → `sqlite3 ~/.oxios/workspace/memory.db "SELECT * FROM memories WHERE source='agent'"` shows the entry
3. `oxios run --json "what do I prefer?"` → agent calls `memory_search` → retrieves the entry

### Phase 2: API embedding provider + SqliteMemoryStore fixes (medium risk, high impact)

This phase has three sub-tasks: new provider, embedding-error resilience, and vector backfill.

#### Phase 2a: New `ApiEmbeddingProvider`

| File | Change |
|------|--------|
| `crates/oxios-kernel/src/embedding/api.rs` | New file — `ApiEmbeddingProvider` impl |
| `crates/oxios-kernel/src/embedding/mod.rs` | Re-export + provider resolution function |
| `crates/oxios-kernel/src/config.rs` | Add `EmbeddingConfig` section (endpoint, api_key, model, dimensions) |
| `src/kernel.rs` (assembler) | Wire `ApiEmbeddingProvider` into `MemoryManager` at boot when config present |
| `share/default-config.toml` | Document `[embedding]` section (commented out by default) |

**Provider resolution order**:
1. `[embedding]` config present → `ApiEmbeddingProvider`
2. `embedding-gguf` feature + aarch64 → `GgufEmbeddingProvider`
3. Fallback → `TfIdfEmbeddingProvider` (sparse, no vectors, BM25-only search)

#### Phase 2b: Make `embed()` non-fatal in `remember()`

**Problem** (verified): `SqliteMemoryStore::remember()` at `store.rs:155` does `self.embedding.embed(&entry.content).await?` — the `?` propagates API errors. The memory row IS already inserted (lines 114-144), but the caller sees `Err` and treats the write as failed. A transient API blip causes the agent to think the memory write failed, retry, and create duplicates.

**Fix**: Replace `?` with `match` — matching the existing non-fatal pattern already used for the vector insert at line 157:

```rust
// Before (store.rs:155):
let embedding_vec = self.embedding.embed(&entry.content).await?;

// After:
let embedding_vec = match self.embedding.embed(&entry.content).await {
    Ok(v) => v,
    Err(e) => {
        tracing::warn!(id = %id, error = %e, "Embedding failed, storing text only");
        return Ok(id);  // text + FTS5 already saved; vector skipped
    }
};
```

Same fix applies to `get_query_vector()` (`store.rs:719`) — though search already degrades gracefully via `recall().unwrap_or_default()`.

#### Phase 2c: sqlite-vec backfill + dimension migration

**Problem** (verified): `MemoryManager::rebuild_index()` (`manager/store.rs:55-88`) iterates file-based JSON storage (`self.storage.list_category()`), not SQLite. In SQLite mode it's a no-op. Even in file mode, it never touches the `memory_vectors` vec0 table. There is no existing method to populate sqlite-vec from existing `memories` rows.

**New method** — `SqliteMemoryStore::backfill_vectors()`:

```rust
/// Compute and insert embeddings for all memories missing a vector.
/// Called on first boot with a dense embedding provider.
pub async fn backfill_vectors(&self) -> Result<usize> {
    let conn = self.db.conn();
    let missing: Vec<(i64, String)> = conn
        .prepare(
            "SELECT m.rowid, m.content
             FROM memories m
             WHERE m.rowid NOT IN (SELECT rowid FROM memory_vectors_rowids)"
        )?
        .query_map([], |row| (row.get(0)?, row.get(1)?))?
        .filter_map(Result::ok)
        .collect();

    let mut count = 0;
    for (rowid, content) in missing {
        if let Ok(vec) = self.embedding.embed(&content).await
            && let Some(f32_vec) = vec.to_f32_dense()
            && memory_insert_vector(&self.db, rowid, &f32_vec).is_ok()
        {
            count += 1;
        }
    }
    tracing::info!(backfilled = count, "Vector backfill complete");
    Ok(count)
}
```

**Dimension migration** — detect embedding model change on boot:

```rust
/// Check if the configured embedding dimension matches what's stored.
/// On mismatch, wipe the vec0 table and backfill from scratch.
fn reconcile_vector_dimension(db: &MemoryDatabase, new_dim: usize) -> Result<()> {
    let stored_dim: Option<usize> = db.conn()
        .query_row("SELECT length(data) / 4 FROM memory_vectors LIMIT 1", [], |r| r.get(0))
        .ok();
    if let Some(old) = stored_dim {
        if old != new_dim {
            tracing::warn!(old_dim = old, new_dim = new_dim, "Embedding dimension changed, wiping vectors");
            db.conn().execute("DELETE FROM memory_vectors", [])?;
        }
    }
    Ok(())
}
```

This mirrors mnemopi's `reconcileEmbeddingModel()` (`memory.ts:435`).

**Boot sequence** (in `src/kernel.rs` after MemoryManager construction):
1. Resolve embedding provider (API → GGUF → TF-IDF)
2. If dense provider: `reconcile_vector_dimension()` → `backfill_vectors()` (background task)
3. If TF-IDF: skip (no vectors, BM25-only — current behavior)

| File | Change |
|------|--------|
| `crates/oxios-memory/src/memory/sqlite/store.rs` | Add `backfill_vectors()`, fix `remember()` embed-error handling |
| `crates/oxios-memory/src/memory/sqlite/mod.rs` | Add `reconcile_vector_dimension()` |
| `src/kernel.rs` | Wire boot sequence: resolve provider → reconcile → backfill |

**Verification**: After config + restart:
- `sqlite3 ~/.oxios/workspace/memory.db "SELECT count(*) FROM memory_vectors_rowids"` → matches `memories` count
- Kill network mid-conversation → memory write still succeeds (text only), vector skipped, `warn` logged

### Phase 3: PersistenceHook fix (medium risk)

| File | Change |
|------|--------|
| `crates/oxios-kernel/src/persistence_hook.rs:295-388` | Add tracing to `reflect()`, add heuristic fallback, improve prompt |

**Tracing additions**:
```rust
tracing::info!(
    model = %engine.default_model_id(),
    prompt_len = prompt.len(),
    "PersistenceHook reflection starting"
);
// ... after agent.run() ...
tracing::info!(
    response_len = response.content.len(),
    "PersistenceHook reflection response received"
);
// ... after JSON parse ...
tracing::info!(
    memory_count = plan.memory.len(),
    knowledge_count = plan.knowledge.len(),
    "PersistenceHook reflection plan"
);
```

**Heuristic fallback** (when LLM fails or returns empty):
- Scan **user input** (`directive.original_request`) for preference patterns: "I prefer", "I like", "always", "never", "remember that", "기억해", "항상", "절대"
- Scan **user input** for factual statements: "my name is", "I use", "the project uses", "내가 쓰는", "우리 프로젝트는"
- Also scan agent **output** for stated facts about the user/project context
- If found, create `MemoryType::Fact` entries with `importance: 0.6`, `source: "persistence-hook-heuristic"`
- Note: the user's preferences live in their *input*, not the agent's *output*. The LLM reflection prompt already sees both (`reflect()` passes `directive.original_request`), but the heuristic must match this.

**Verification**: `RUST_LOG=oxios_kernel::persistence_hook=info oxios run "I always use vim for editing"` → log shows reflection plan with 1+ memory entries → `memory.db` gains a `fact` entry.

### Phase 4: Recall integration test

End-to-end test that mirrors mnemopi's workflow:
1. Session A: "I prefer Korean responses and use zsh"
2. Session B (new session, same space): "What shell do I use?"
3. Assert: auto-recall injects the preference memory into session B's system prompt
4. Assert: agent responds with "zsh" without being told

## 4. Config Schema

```toml
[embedding]
# API-based embedding provider for semantic memory search.
# When unset, falls back to TF-IDF (keyword-only search, no vectors).
# endpoint = "https://api.openai.com/v1/embeddings"
# api_key = ""                    # falls back to provider API key if empty
# model = "text-embedding-3-small"
# dimensions = 1536
```

Resolution: `[embedding].endpoint` + `[embedding].api_key` (or inherited from the active LLM provider) → `ApiEmbeddingProvider`.

## 5. What Changes vs What Doesn't

### Changes (this design)

- **SqliteMemoryStore** — `remember()` embed-error handling (`?` → `match`), new `backfill_vectors()`, new `reconcile_vector_dimension()`.
- **tools/builtin/mod.rs** — replace 4 dead oxi-agent tools with 3 working MemoryManager tools.
- **tools/registration.rs** — remove CSpace-gated memory branch (redundant).
- **agent_runtime.rs:1660** — system prompt update.
- **New**: `embedding/api.rs` (ApiEmbeddingProvider), config schema.

### Does NOT change

- **MemoryManager** — untouched. Delegates to SqliteMemoryStore correctly.
- **HNSW index** — initialized at boot (`kernel.rs:1209`), but **dead weight in SQLite mode**: `MemoryManager::remember()` early-returns to `SqliteMemoryStore::remember()` (`store.rs:144`), so `hnsw.add_entry()` never fires. sqlite-vec serves the same role. Not a bug, but the index occupies ~10000 × dim × 4 bytes of memory for nothing. Future cleanup: skip HNSW init when SQLite mode is active.
- **Dream/SONA/decay** — untouched. They operate on MemoryManager entries which will now actually exist.
- **KnowledgeLens** — untouched. Its 8 `knowledge:lens` entries are correct and will get vectors via backfill.
- **Compaction handler** — `handle_compaction()` (`agent_runtime.rs:1578`) creates `MemoryType::Conversation` entries, but DB has 0. Either compaction isn't triggering or the callback isn't wired. **Not addressed in this design** — separate issue. Affects recall quality (`recall()` always tries to include recent conversation summaries).

## 6. Risk Assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| API embedding latency on first backfill | Medium | Background task, don't block boot |
| API key not configured | High | Graceful fallback to TF-IDF (current behavior) |
| Embedding API transient failure | Medium | Phase 2b: `embed()` non-fatal in `remember()` — text stored, vector skipped |
| Dimension mismatch on model change | Low | Phase 2c: `reconcile_vector_dimension()` detects + wipes on boot |
| PersistenceHook LLM call adds cost/latency | Low | Already fire-and-forget; runs post-execution |
| Removing oxi-agent tools breaks something | Low | grep confirms no internal callers; agent-facing only |
| Backfill iterates all rows synchronously | Low | For ~10K entries at ~50ms/embed = ~8min. Acceptable as background task |

## 7. Comparison with mnemopi After Fix

| Aspect | mnemopi | Oxios (after) |
|--------|---------|---------------|
| Backend | Single SQLite | Single SQLite (`memory.db`) |
| Embeddings | BGE-small (local) or API | API (primary), TF-IDF (fallback) |
| Embedding failure | Graceful (FTS-only) | Graceful (Phase 2b: text stored, vector skipped) |
| Dimension migration | `reconcileEmbeddingModel()` | `reconcile_vector_dimension()` (Phase 2c) |
| Auto-extraction | `remember(extract: true)` per turn | PersistenceHook post-execution + heuristic fallback |
| Recall | BM25 + vector + temporal (RRF) | BM25 + vector (RRF) — same `search/rrf.rs` |
| Consolidation | `sleep` (Weibull veracity) | `Dream` process (decay + compaction) |
| Agent tools | `remember`, `recall`, `stats`, `sleep` | `memory_write`, `memory_read`, `memory_search` |
| Tool↔store consistency | ✅ single backend | ✅ single backend (after Phase 1) |
