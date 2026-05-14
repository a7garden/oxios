# Oxios Advanced Features — Detailed Analysis Report

**Date:** 2026-05-14  
**Total Lines Analyzed:** 8,583 across 11 files  
**Analyst:** Automated code review agent

---

## Table of Contents

1. [Memory System (mod.rs)](#1-memory-system-modrs)
2. [Memory Store (store.rs)](#2-memory-store-storers)
3. [HNSW Vector Search (hnsw.rs)](#3-hnsw-vector-search-hnswrs)
4. [Hyperbolic Embeddings (hyperbolic.rs)](#4-hyperbolic-embeddings-hyperbolicrs)
5. [Flash Attention (flash_attention.rs)](#5-flash-attention-flash_attentionrs)
6. [Ouroboros Orchestrator (orchestrator.rs)](#6-ouroboros-orchestrator-orchestratorrs)
7. [Access Manager / RBAC (access_manager/mod.rs)](#7-access-manager--rbac-access_managermodrs)
8. [Audit Trail (audit_trail.rs)](#8-audit-trail-audit_trailrs)
9. [Budget Manager (budget.rs)](#9-budget-manager-budgetrs)
10. [Task Scheduler (scheduler.rs)](#10-task-scheduler-schedulerrs)
11. [Cron Scheduler (cron.rs)](#11-cron-scheduler-cronrs)
12. [Cross-Cutting Observations](#12-cross-cutting-observations)

---

## 1. Memory System (`memory/mod.rs`)

| Metric | Value |
|--------|-------|
| **Lines** | 620 |
| **Production Code** | ~330 |
| **Test Code** | ~290 |
| **Test Ratio** | 47% |

### Key Structs & Types

| Type | Purpose |
|------|---------|
| `MemoryManager` | Central memory manager; stores/recalls entries via `StateStore` |
| `MemoryEntry` | Individual memory record with metadata (importance, tags, timestamps) |
| `MemoryType` | Enum: `Conversation`, `Session`, `Fact`, `Episode`, `Knowledge` |
| `TextVector` | TF-IDF vector with cosine similarity; language-agnostic tokenizer |

### Key Functions

- `remember()` / `forget()` — CRUD operations for memories
- `recall()` — Retrieves relevant memories combining recent sessions + semantic search
- `search()` — Vector search with keyword fallback
- `curate()` — Budget-aware memory pruning based on effective importance
- `effective_importance()` — `base_importance * (1 + ln(1 + access_count))`
- `blend_into_prompt()` — Injects recalled memories into system prompts
- `is_duplicate()` / `remember_unique()` — Content-hash + semantic deduplication

### Algorithm Complexity

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| `TextVector::cosine_similarity()` | O(V) | V = unique terms in both vectors |
| `search()` (vector) | O(N × V) | Brute-force over all indexed entries |
| `search()` (keyword) | O(K × N) | K keywords × N entries |
| `curate()` | O(T × N × log N) | T types × sort per type |
| `is_duplicate()` | O(N × V + N × H) | Vector similarity + hash comparison |

### Code Quality Observations

- **Well-structured**: Clear separation between data types, manager logic, and helpers
- **Good test coverage**: Tests for Korean tokenization, vector similarity, dedup, and index rebuild
- **Korean support**: Tokenizer preserves Hangul syllables (U+AC00–U+D7A3) — practical for the project's Korean operator audience
- **Content hashing**: Uses `DefaultHasher` — fast but not cryptographically stable across Rust versions

### Issues & Concerns

1. **⚠️ Brute-force vector search**: `search()` iterates all entries. Should use HNSW when available for large-scale deployments
2. **⚠️ `is_duplicate()` loads all entries**: The hash-based dedup path calls `list(*mt, 1000)` for all 5 types — O(N) I/O per check
3. **⚠️ Stop word list is English-only**: Korean stop words not included, limiting keyword search quality
4. **ℹ️ `DefaultHasher` instability**: Hash values may change between Rust versions; deduplication across restarts relies on semantic similarity (>0.95) as primary guard

### Integration Patterns

- Consumes `StateStore` (file-based persistence)
- Consumes `EmbeddingProvider` trait (default: `TfIdfEmbeddingProvider`)
- Optional `GitLayer` for version-controlled saves
- Optional `HnswMemoryIndex` for fast ANN search
- Re-exports budget types for curation

---

## 2. Memory Store (`memory/store.rs`)

| Metric | Value |
|--------|-------|
| **Lines** | 850 |
| **Production Code** | ~540 |
| **Test Code** | 0 (tests in mod.rs) |

### Key Structs & Types

| Type | Purpose |
|------|---------|
| `HnswMemoryIndex` | Manages HNSW index with u64↔String ID mapping, persistence |
| `SemanticHit` | Search result with entry + distance + similarity |
| `VectorIndexSnapshot` | Serializable index snapshot for disk persistence |

### Key Functions

- `remember()` — Stores entry + updates both vector index and HNSW index
- `forget()` — Deletes entry + removes from HNSW index
- `search()` — Vector search with keyword fallback (threshold: 0.1)
- `semantic_search()` — HNSW-accelerated search with graceful fallback chain
- `rebuild_index()` / `rebuild_hnsw_index()` — Full index reconstruction from disk
- `save_index_snapshot()` / `load_index_snapshot()` — Persist/load vector index
- `summarize_session()` — Auto-generate session summary (no LLM)
- `is_duplicate()` — Dual check: vector similarity (>0.95) + content hash

### Algorithm Complexity

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| `semantic_search()` (HNSW) | O(log N × D) | Approximate nearest neighbor |
| `search()` (brute-force) | O(N × D) | D = embedding dimension |
| `rebuild_hnsw_index()` | O(N × log N × D) | Full rebuild from disk |
| `HnswMemoryIndex::add_entry()` | O(log N) | HNSW insertion |

### Code Quality Observations

- **Layered fallback**: HNSW → brute-force vector → keyword — resilient design
- **Lock discipline**: Read locks scoped before `.await` points — correct async safety
- **Double-checked locking**: `get_or_create_key()` uses read→write pattern to avoid write contention

### Issues & Concerns

1. **⚠️ `total_entries()` is expensive**: Calls `list()` with `usize::MAX` for all 5 types — full disk scan each call
2. **⚠️ `is_duplicate()` semantic threshold hardcoded**: 0.95 similarity is aggressive; near-duplicates with slight rewording may pass
3. **⚠️ `summarize_session()` truncation at 500 chars**: Magic number; could lose important context
4. **⚠️ HNSW index not persisted on every `remember()`**: Only `persist()` saves; crash between `remember()` calls loses index state

### Integration Patterns

- Extends `MemoryManager` with store/search methods via `impl` block
- HNSW index is optional — system degrades gracefully without it
- Git integration for version-controlled persistence

---

## 3. HNSW Vector Search (`memory/hnsw.rs`)

| Metric | Value |
|--------|-------|
| **Lines** | 314 |
| **Production Code** | ~180 |
| **Test Code** | ~134 |

### Key Structs & Types

| Type | Purpose |
|------|---------|
| `HnswIndex` | Thin wrapper around `usearch::Index` with dimension validation |

### Configuration Constants

| Parameter | Default | Purpose |
|-----------|---------|---------|
| `DEFAULT_DIMENSIONS` | 1536 | OpenAI text-embedding-3-small |
| `DEFAULT_CONNECTIVITY` | 16 | HNSW graph edges per node |
| `DEFAULT_EXPANSION_SEARCH` | 128 | Beam width for search |
| `DEFAULT_EXPANSION_ADD` | 128 | Beam width for insertion |

### Key Functions

- `new()` — Create with custom dimensions/capacity; cosine metric, F32 scalar
- `add()` / `search()` / `remove()` — Core CRUD with dimension validation
- `save()` / `load()` — Persistence to binary file
- `contains()` / `get()` — Key lookup and vector retrieval
- `rename()` — Key reassignment

### Algorithm Complexity

| Operation | Complexity |
|-----------|-----------|
| `add()` | O(log N × D × connectivity) |
| `search()` | O(log N × D × expansion_search) |
| `remove()` | O(log N) |

### Code Quality Observations

- **Clean abstraction**: Minimal wrapper over `usearch` — easy to swap implementations
- **Good error messages**: Dimension mismatches reported with expected vs actual
- **Key 0 filtering**: Filters `usearch` sentinel value (key=0) from search results
- **Comprehensive tests**: Add/search/remove/save-load/edge cases

### Issues & Concerns

1. **⚠️ Not thread-safe internally**: Documentation correctly notes callers must synchronize (done via `RwLock` in `HnswMemoryIndex`)
2. **⚠️ `load()` doesn't validate dimensions**: Returns whatever the saved index had; could mismatch with current config
3. **ℹ️ Default dimensions hardcoded to OpenAI**: May need adjustment for other embedding providers

### Integration Patterns

- Used by `HnswMemoryIndex` (in store.rs) which handles thread safety and ID mapping
- Part of the `memory` module's layered search: HNSW → brute-force → keyword

---

## 4. Hyperbolic Embeddings (`memory/hyperbolic.rs`)

| Metric | Value |
|--------|-------|
| **Lines** | 580 |
| **Production Code** | ~330 |
| **Test Code** | ~250 |

### Key Structs & Types

| Type | Purpose |
|------|---------|
| `HyperbolicConfig` | Curvature, dimensions, epsilon |
| `HyperbolicEmbedding` | Manager for named Poincaré ball embeddings |

### Key Functions

- `euclidean_to_poincare()` — `tanh`-based bounded projection onto Poincaré ball
- `hyperbolic_distance()` — Arcosh-based distance in hyperbolic space
- `mobius_add()` — Hyperbolic vector addition (gyrogroup operation)
- `mobius_scalar_mul()` — Scaling in hyperbolic space via `atanh/tanh`
- `add_child()` — Parent-child relationship via Möbius addition
- `nearest_neighbors()` / `search()` — k-NN in hyperbolic space
- `depth()` / `rank_by_depth()` — Hierarchy depth from origin

### Algorithm Complexity

| Operation | Complexity |
|-----------|-----------|
| `hyperbolic_distance()` | O(D) |
| `mobius_add()` | O(D) |
| `nearest_neighbors()` | O(N × D) — brute-force |
| `search()` | O(N × D) — brute-force |
| `rank_by_depth()` | O(N × D + N log N) |

### Code Quality Observations

- **Mathematically rigorous**: Correct Poincaré ball formulas with proper curvature handling
- **Excellent test coverage**: Tests for identity laws, symmetry, triangle inequality, boundedness
- **Numerical stability**: Epsilon clamping in scalar multiplication, boundary checks in distance
- **Well-documented**: References Nickel & Kiela (2017)

### Issues & Concerns

1. **⚠️ Brute-force k-NN**: `nearest_neighbors()` and `search()` are O(N) — no HNSW-like acceleration for hyperbolic space. Will not scale beyond ~10K embeddings
2. **⚠️ No persistence**: `HyperbolicEmbedding` is in-memory only — embeddings lost on restart
3. **⚠️ `embeddings` is a Vec of tuples**: Linear scan for `get()`, `add()`, `add_child()` — O(N) per operation. Should use a HashMap
4. **ℹ️ `c()` method is `#[allow(dead_code)]`**: Utility not yet used
5. **ℹ️ No actual training**: Embeddings are projected from Euclidean, not learned via Riemannian SGD

### Integration Patterns

- Standalone module — not yet integrated into the core `MemoryManager` pipeline
- Intended for persona hierarchies, skill graphs, and memory taxonomies (per docs)

---

## 5. Flash Attention (`memory/flash_attention.rs`)

| Metric | Value |
|--------|-------|
| **Lines** | 582 |
| **Production Code** | ~340 |
| **Test Code** | ~242 |

### Key Structs & Types

| Type | Purpose |
|------|---------|
| `FlashAttention` | Block-wise attention with online softmax |
| `FlashAttentionConfig` | Block size, dimensions, temperature |
| `BenchmarkResult` | Naive vs flash comparison metrics |
| `MemoryEstimate` | Peak memory calculation |

### Key Functions

- `attention()` — Block-wise scaled dot-product attention with O(N) memory
- `naive_attention()` — Reference O(N²) implementation for benchmarking
- `self_attention()` / `cross_attention()` — Convenience wrappers
- `benchmark()` — Timing + correctness comparison
- `memory_estimate()` — Peak memory prediction

### Algorithm Complexity

| Aspect | Flash | Naive |
|--------|-------|-------|
| **Memory** | O(N × D) | O(N² + N × D) |
| **Compute** | O(N² × D) | O(N² × D) |
| **Cache efficiency** | Block-wise (L1/L2 friendly) | Row-at-a-time |

### Code Quality Observations

- **Correct online softmax**: Proper max-tracking and rescaling for numerical stability
- **Block-size invariant**: Tests verify identical results regardless of block size
- **Deterministic test vectors**: LCG-based PRNG for reproducibility
- **Benchmark with validation**: Checks <5% relative error between flash and naive

### Issues & Concerns

1. **⚠️ CPU-only, single-threaded**: No SIMD, no parallelism. Flash Attention's benefit on CPU is primarily memory, not speed — the 2-5× speedup claim may not hold without optimization
2. **⚠️ No mask support**: Cannot handle causal or padding masks — limits use in transformer-style architectures
3. **⚠️ No multi-head support**: Single-head attention only
4. **⚠️ Not integrated**: This is a standalone utility — not used by any other kernel module currently
5. **ℹ️ LCG PRNG is weak**: Deterministic but poor statistical properties; fine for tests/benchmarks only

### Integration Patterns

- Standalone module — no current integration with memory search or agent runtime
- Potential future use: re-ranking memories, attention-based context selection

---

## 6. Ouroboros Orchestrator (`orchestrator.rs`)

| Metric | Value |
|--------|-------|
| **Lines** | 891 |
| **Production Code** | ~650 |
| **Test Code** | 0 |

### Key Structs & Types

| Type | Purpose |
|------|---------|
| `Orchestrator` | Coordinates full Ouroboros lifecycle |
| `OrchestrationResult` | Response with phase, seed, evaluation metadata |
| `InterviewSession` | Multi-turn interview state |
| `SubTask` | Subtask for multi-agent delegation |
| `AgentRole` | `Worker` or `Manager` role within a group |

### Key Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_EVOLUTION_ITERATIONS` | 3 | Max evolve→execute→evaluate loops |

### Key Functions

- `handle_message()` — Full pipeline: interview → seed → execute → evaluate → evolve
- `delegate_subtasks()` — Multi-agent delegation via A2A or direct lifecycle
- `delegate_via_a2a()` — A2A-based parallel task dispatch with `JoinSet`
- `delegate_via_lifecycle()` — Fallback parallel execution via `AgentGroup`
- `should_split_seed()` / `split_into_subtasks()` — Heuristic multi-task decomposition
- `save_seed()` / `save_evaluation()` — State persistence with git commits

### Code Quality Observations

- **Comprehensive lifecycle**: All 5 Ouroboros phases implemented with event publishing
- **Multi-agent support**: A2A delegation with fallback to direct lifecycle
- **Proper lock scoping**: Sessions HashMap locks are scoped before async operations
- **Event-driven**: Phase transitions published to EventBus for observability
- **Metrics integration**: Orchestrator duration, agent completion/failure tracked

### Issues & Concerns

1. **🔴 `next_task()` recursive on budget exhaustion**: In `scheduler.rs`, when an agent's budget is exhausted, `next_task()` calls itself recursively to try the next task. With many exhausted agents, this could stack overflow
2. **⚠️ `delegate_via_a2a()` closure complexity**: The spawned async closure captures `lifecycle` by clone — 60+ line closure is hard to test and debug
3. **⚠️ `split_into_subtasks()` heuristic is fragile**: Keyword-based capability inference ("review" → "code-review") is simplistic and will miss many cases
4. **⚠️ `should_split_seed()` threshold is arbitrary**: 3+ acceptance criteria → split; no consideration of criterion complexity or dependencies
5. **⚠️ No timeout on Ouroboros loop**: The evolve→execute→evaluate loop has max iterations but no wall-clock timeout. A stuck agent could block indefinitely
6. **⚠️ Session cleanup on error paths**: If `spawn_and_run()` fails after multi-agent delegation, session cleanup may be skipped

### Integration Patterns

- Consumes `OuroborosProtocol` (trait from `oxios-ouroboros`)
- Consumes `AgentLifecycleManager` for agent spawning
- Consumes `EventBus` for phase event publishing
- Consumes `StateStore` + `GitLayer` for persistence
- Optional `A2AProtocol` for inter-agent delegation
- Produces `OrchestrationResult` consumed by Gateway/Channels

---

## 7. Access Manager / RBAC (`access_manager/mod.rs`)

| Metric | Value |
|--------|-------|
| **Lines** | 1,278 |
| **Production Code** | ~550 |
| **Test Code** | ~728 |
| **Test Ratio** | 57% |

### Key Structs & Types

| Type | Purpose |
|------|---------|
| `AccessManager` | Central permission manager with RBAC + audit + workspace sandbox |
| `AgentPermissions` | Per-agent tool/path/network/fork/memory/timeout permissions |
| `RbacManager` | Role-based access control with approval workflow |
| `AuditEntry` | Access decision record |

### Key Functions

- `can_use_tool()` / `can_access_path()` / `can_access_network()` — Permission checks with audit logging
- `can_execute_for()` / `can_use_memory()` — Resource limit checks
- `can_access_path_in_workspace()` — Full sandbox check: RBAC → path → workspace boundary
- `assign_workspace()` / `register_workspace_path()` — Workspace management
- `is_path_in_workspace()` — Canonical path comparison for sandbox enforcement
- `validate_permissions()` — Security review helper
- `log_access()` / `persist_audit_entry()` — Audit trail with file persistence

### Code Quality Observations

- **OWASP-aligned**: Follows least-privilege, agent identity, sandbox boundaries
- **Comprehensive tests**: 50+ test cases covering all permission types, workspace sandbox, edge cases
- **Deny-first**: Unknown agents denied all access by default
- **Workspace sandbox**: Canonical path comparison prevents path traversal
- **Clone derived**: `#[derive(Clone)]` for ExecTool compatibility (cheap — HashMaps of primitives)

### Issues & Concerns

1. **⚠️ `persist_audit_entry()` spawns std::thread**: Each audit log write spawns a new OS thread. Under high throughput, this could create hundreds of threads. Should use a bounded channel + dedicated writer task
2. **⚠️ `audit_log` is unbounded during burst**: Between prunes, the Vec grows without limit. Max audit entries pruning happens on every log, but the Vec is still unbounded during a single log call
3. **⚠️ `can_access_path_in_workspace()` takes `&mut self`**: Because it calls `log_access()`, it requires exclusive access even for read-only checks. This limits concurrent access checking
4. **⚠️ Path glob matching**: Uses `glob::Pattern` (imported from permissions module) for path matching — may have edge cases with symlinks
5. **ℹ️ `AuditEntry` is duplicated**: Both `access_manager::AuditEntry` and `audit_trail::AuditEntry` exist with different schemas

### Integration Patterns

- Consumed by `ExecTool` for pre-execution permission checks
- Integrates with `RbacManager` for HitL approval workflows
- Workspace sandbox used by Orchestrator for agent isolation
- Audit log persisted to file (JSONL format)

---

## 8. Audit Trail (`audit_trail.rs`)

| Metric | Value |
|--------|-------|
| **Lines** | 1,128 |
| **Production Code** | ~460 |
| **Test Code** | ~668 |
| **Test Ratio** | 59% |

### Key Structs & Types

| Type | Purpose |
|------|---------|
| `AuditTrail` | Tamper-evident hash chain (Merkle-chain style) |
| `AuditEntry` | Single chain entry with seq, actor, action, hash links |
| `AuditAction` | 12 typed action variants (AgentSpawn, ToolCall, MemoryWrite, etc.) |
| `AuditError` | ChainBroken, InvalidTimestamp, ExportFailed |

### Key Functions

- `append()` / `append_with_meta()` — Add entry with automatic hash chain computation
- `verify()` — Full chain integrity check (prev_hash linkage + hash recomputation)
- `by_agent()` / `by_action()` / `by_action_type()` — Query filters
- `entries()` / `all_entries()` — Range and full access
- `export_json()` / `export_all_json()` — JSON export
- `flush()` / `restore_from()` — Persistence via StateStore
- `compute_entry_hash()` — blake3 hash of all entry fields

### Algorithm Complexity

| Operation | Complexity |
|-----------|-----------|
| `append()` | O(1) amortized, O(N) on prune |
| `verify()` | O(N) — full chain traversal |
| `by_agent()` / `by_action()` | O(N) — linear scan |
| `auto-prune + rehash` | O(N) — rebuild entire chain |

### Code Quality Observations

- **Cryptographic tamper detection**: blake3 hash chain makes any modification detectable
- **Prune-aware verification**: Handles "pruned" as valid chain root after auto-pruning
- **Deterministic hashing**: Includes `oxios-audit-v1` domain separator
- **Comprehensive test coverage**: Tampering detection, chain verification, prune-and-verify, all action types
- **12 action types**: Covers all kernel operations (spawn, tool, memory, config, cron, git, access)

### Issues & Concerns

1. **🔴 Auto-prune rehashes entire chain**: When pruning triggers, every remaining entry is rehashed in O(N). For a 100K-entry trail with 50K pruned, this rehashes 50K entries
2. **⚠️ `verify()` checks timestamp against `Utc::now()`**: Entries created on a machine with a fast clock will fail verification on a machine with a slow clock
3. **⚠️ `flush()` is synchronous**: Writes all entries as a single JSON file — blocks the caller and produces a large file for long-running systems
4. **⚠️ `by_action()` compares full enum equality**: Only matches exact action variants including data fields, not just the discriminant
5. **⚠️ `restore_from()` holds write lock during trim+rehash**: Could block readers for extended periods on large restored sets

### Integration Patterns

- StateStore extension: `save_audit_entries()` / `load_audit_entries()`
- Used by kernel for security-critical event logging
- Independent from `AccessManager`'s simpler audit log (duplication concern)

---

## 9. Budget Manager (`budget.rs`)

| Metric | Value |
|--------|-------|
| **Lines** | 599 |
| **Production Code** | ~270 |
| **Test Code** | ~329 |
| **Test Ratio** | 55% |

### Key Structs & Types

| Type | Purpose |
|------|---------|
| `BudgetManager` | Sliding-window budget tracker per agent |
| `BudgetLimit` | Per-agent token + call limits with window duration |
| `Usage` | Current window consumption state |
| `BudgetInfo` | Remaining budget information |
| `BudgetExceeded` | Error with agent ID, kind (Token/Call), message |

### Key Functions

- `set_budget()` / `remove_budget()` — Budget lifecycle
- `reserve()` — Reserve tokens (checks + deducts atomically)
- `release()` — Return tokens on retry/error
- `track_call()` — Increment call counter
- `remaining()` / `can_schedule()` — Budget queries
- `reset_window()` — Manual window reset

### Algorithm Complexity

| Operation | Complexity |
|-----------|-----------|
| `reserve()` | O(1) — HashMap lookup + arithmetic |
| `track_call()` | O(1) |
| `remaining()` | O(1) |
| `reset_if_expired()` | O(1) |

### Code Quality Observations

- **Simple and correct**: Sliding window with automatic reset
- **Thread-safe**: `RwLock<HashMap>` for concurrent access
- **Two budget dimensions**: Tokens and calls tracked independently
- **Good test coverage**: Exhaustion, window reset, multi-agent, release

### Issues & Concerns

1. **⚠️ No budget persistence**: Budgets are in-memory only — lost on restart. An agent that was rate-limited before restart gets a fresh window
2. **⚠️ `Instant` is not serializable**: `Usage.window_start` uses `Instant` which is opaque — cannot be persisted or inspected
3. **⚠️ `reserve()` is not atomic with actual use**: Tokens are reserved but the actual LLM call may fail or use fewer tokens. No auto-refund mechanism
4. **⚠️ No budget alerts**: Only hard limits; no warning thresholds (e.g., "80% consumed")
5. **ℹ️ `BudgetExceeded` doesn't implement `std::error::Error` properly**: Missing `source()` implementation

### Integration Patterns

- Consumed by `AgentScheduler` for admission control
- `can_schedule()` used as soft gate before task execution
- `track_call()` called when scheduler starts a task

---

## 10. Task Scheduler (`scheduler.rs`)

| Metric | Value |
|--------|-------|
| **Lines** | 994 |
| **Production Code** | ~470 |
| **Test Code** | ~524 |
| **Test Ratio** | 53% |

### Key Structs & Types

| Type | Purpose |
|------|---------|
| `AgentScheduler` | Priority-based task queue with rate limiting and zombie detection |
| `ScheduledTask` | Task with ID, priority, status, agent association |
| `Priority` | `Low(0)` / `Normal(1)` / `High(2)` / `Critical(3)` |
| `TaskStatus` | `Queued` / `Running` / `Completed` / `Failed` / `Cancelled` |
| `SchedulerStats` | Queue statistics |
| `RateLimiter` | Sliding-window request rate tracker |

### Key Functions

- `submit()` — Priority-sorted insertion into queue
- `next_task()` — Pop highest-priority with rate limit + concurrency + budget checks
- `complete_task()` / `fail_task()` — Task lifecycle completion
- `reap_zombies()` — Detect and kill tasks running longer than timeout
- `cancel_task()` — Cancel queued (not running) tasks
- `start_task()` — Atomically claim a specific queued task

### Algorithm Complexity

| Operation | Complexity |
|-----------|-----------|
| `submit()` | O(N) — linear scan for insertion position |
| `next_task()` | O(1) amortized (pop from end) + O(K) recursive on budget skip |
| `reap_zombies()` | O(R) — R = running tasks |
| `cancel_task()` | O(N) — linear scan |

### Code Quality Observations

- **Priority queue via sorted Vec**: Simple, works for moderate queue sizes
- **Rate limiter**: Sliding window with per-minute tracking
- **Zombie detection**: Time-based reaping prevents resource leaks
- **Budget integration**: Soft gate — exhausted agents skipped, next task tried
- **Comprehensive tests**: Priority ordering, concurrency limits, rate limiting, zombie reaping, budget integration

### Issues & Concerns

1. **🔴 Recursive `next_task()` on budget exhaustion**: Calls itself recursively when skipping exhausted agents. With many exhausted agents in the queue, this is O(K) recursion where K = exhausted count. Should use an iterative loop instead
2. **⚠️ Priority queue is a sorted Vec**: `submit()` is O(N) for insertion. A `BinaryHeap` would give O(log N) insertion and O(log N) extraction
3. **⚠️ `stats()` reports completed/failed as 0**: The counters are computed from queue/running only — historical stats lost. Comment says "could optimize with separate counters"
4. **⚠️ FIFO within same priority is LIFO**: Same-priority tasks are inserted at the first position, making the queue LIFO within a priority level. This may cause starvation of earlier-submitted tasks
5. **⚠️ Rate limiter counts against `next_task()` calls**: Even failed `next_task()` calls (queue empty, max concurrent) consume rate limit tokens
6. **⚠️ Lock granularity**: Multiple separate Mutex/RwLock acquisitions in `next_task()` — potential for inconsistent state views

### Integration Patterns

- Consumes `BudgetManager` for budget-aware scheduling
- Used by `Orchestrator` for task submission (`Priority::Normal`/`High`)
- `reap_zombies()` called periodically by Orchestrator
- `Arc<Mutex<AgentScheduler>>` pattern for shared access

---

## 11. Cron Scheduler (`cron.rs`)

| Metric | Value |
|--------|-------|
| **Lines** | 747 |
| **Production Code** | ~420 |
| **Test Code** | ~327 |
| **Test Ratio** | 44% |

### Key Structs & Types

| Type | Purpose |
|------|---------|
| `CronScheduler` | Time-based autonomous agent execution |
| `CronJob` | Job definition with schedule, goal, constraints, state |
| `CronJobResult` | Execution result record |
| `CronJobUpdate` | Partial update for existing jobs |
| `JobSource` | `Config` vs `Api` origin tracking |

### Key Functions

- `add_job()` / `remove_job()` / `update_job()` — Job lifecycle
- `trigger_job()` / `mark_job_completed()` — Manual execution
- `start()` — Main tick loop with configurable interval
- `tick_inner()` — Find due jobs and spawn async execution
- `restore_jobs()` — Load persisted jobs on startup
- `load_from_config()` — Load config-defined jobs (API wins on conflict)
- `normalize_expr()` — 5-field cron → 6-field (prepend seconds)

### Algorithm Complexity

| Operation | Complexity |
|-----------|-----------|
| `tick_inner()` | O(N) — scan all jobs for due time |
| `add_job()` | O(1) + I/O |
| `normalize_expr()` | O(1) |

### Code Quality Observations

- **Flexible cron expressions**: Supports 5/6/7 field formats via normalization
- **Config/API precedence**: Config jobs don't overwrite API-created jobs
- **Graceful cancellation**: `AtomicBool` flag checked each tick
- **Job isolation**: Running jobs tracked to prevent double-execution
- **Good lock discipline**: `RwLockWriteGuard` dropped before `.await` in `update_job()`

### Issues & Concerns

1. **⚠️ No job execution timeout**: Spawned cron tasks run indefinitely. No zombie detection for cron jobs (unlike `AgentScheduler`)
2. **⚠️ No retry on failure**: Failed cron jobs are recorded but never retried
3. **⚠️ No max concurrent job limit**: All due jobs are spawned simultaneously — could overwhelm the system
4. **⚠️ `persist_jobs()` on every mutation**: Every `add_job()` / `remove_job()` / `update_job()` writes all jobs to disk. High-frequency updates cause I/O pressure
5. **⚠️ `tick_inner()` spawns `tokio::spawn` without `JoinHandle` tracking**: Fire-and-forget execution — no way to cancel or track running cron tasks
6. **ℹ️ `dirty` flag is set but never checked**: Unused optimization hint

### Integration Patterns

- Consumes `StateStore` for job persistence
- Consumes `GitLayer` for version-controlled saves
- Consumes `Priority` from `scheduler` module
- `start()` takes generic executor closure — decoupled from specific agent runtime
- `load_from_config()` integrates with `CronConfig` from kernel config

---

## 12. Cross-Cutting Observations

### Architecture Strengths

1. **Layered fallback patterns**: Memory search (HNSW → brute-force → keyword), multi-agent delegation (A2A → lifecycle → single), budget checks
2. **Event-driven design**: EventBus for phase transitions, agent lifecycle events
3. **Security-first**: OWASP-inspired AccessManager with RBAC, workspace sandboxing, tamper-evident audit trail
4. **Observability**: Comprehensive tracing throughout all modules
5. **Test coverage**: All modules have thorough unit tests (40-59% test ratio)

### Systemic Concerns

| Concern | Severity | Affected Modules |
|---------|----------|-----------------|
| Recursive `next_task()` in scheduler | 🔴 High | scheduler |
| Auto-prune rehashes entire audit chain | 🔴 High | audit_trail |
| No persistence for budgets | ⚠️ Medium | budget |
| Thread-per-audit-entry persistence | ⚠️ Medium | access_manager |
| Brute-force k-NN in hyperbolic space | ⚠️ Medium | hyperbolic |
| Flash Attention not integrated | ⚠️ Medium | flash_attention |
| Duplicate AuditEntry types | ⚠️ Medium | access_manager, audit_trail |
| Priority queue O(N) insertion | ⚠️ Low | scheduler |
| No job retry/timeout in cron | ⚠️ Low | cron |

### Dependency Graph

```
Orchestrator
├── OuroborosProtocol (trait)
├── AgentLifecycleManager
├── AgentScheduler ← BudgetManager
├── EventBus
├── StateStore
├── GitLayer
└── A2AProtocol (optional)

MemoryManager
├── StateStore
├── EmbeddingProvider (trait)
├── HnswMemoryIndex → HnswIndex (usearch)
├── GitLayer (optional)
└── MemoryBudget

AccessManager
├── AgentPermissions
├── RbacManager
└── Workspace sandbox

AuditTrail
├── blake3 (hashing)
└── StateStore (persistence)

CronScheduler
├── StateStore
├── GitLayer (optional)
└── cron crate (parsing)
```

### Recommendations

1. **Replace recursive `next_task()`** with an iterative loop to prevent stack overflow
2. **Use incremental audit chain**: Only hash new entries, don't rehash entire chain on prune
3. **Add budget persistence**: Serialize `BudgetLimit` to StateStore on `set_budget()`
4. **Use async channel for audit persistence**: Replace `std::thread::spawn` with `tokio::sync::mpsc`
5. **Replace sorted Vec** in scheduler with `BinaryHeap` for O(log N) priority operations
6. **Add cron job timeout and retry**: Track `JoinHandle`s, enforce max concurrent cron jobs
7. **Unify audit types**: Merge `access_manager::AuditEntry` and `audit_trail::AuditEntry` into a single type
8. **Consider hyperbolic HNSW**: For production use of hyperbolic embeddings, implement hyperbolic distance in a specialized ANN index
9. **Integrate Flash Attention**: Connect to memory re-ranking or context selection pipeline

---

*End of report.*
