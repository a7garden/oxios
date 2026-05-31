# cargo doc Assessment

**Date:** 2026-05-31  
**Command:** `cargo doc --workspace --no-deps`  
**Result:** 684 HTML pages generated for `oxios-kernel` (other crates also generated)

---

## Build Status

| Crate | Status | Warnings |
|-------|--------|----------|
| `oxios-kernel` | ✅ Generated | 25 (6 duplicates) |
| `oxios-markdown` | ✅ Generated | 0 |
| `oxios-mcp` | ✅ Generated | 0 |
| `oxios-ouroboros` | ✅ Generated | 0 |
| `oxios-gateway` | ❌ Build error | unrelated compile error |
| `oxios-bench` | ✅ Generated | 0 |

**Note:** `oxios-gateway` has a pre-existing compile error unrelated to documentation. This prevents its docs from being generated.

---

## Warning Breakdown

### Broken Links (16 warnings, 4 distinct types)

| Count | Broken Link | Location(s) |
|-------|-------------|-------------|
| 10 | `[`KernelHandle`]` | `tools/mcp_tool.rs`, `tools/memory_tools.rs`, `tools/browser_tool.rs`, `tools/exec_tool.rs`, `tools/a2a_tools.rs`, `agent_runtime.rs`, `supervisor.rs` |
| 2 | `[`KnowledgeBase`]` | `kernel_handle/knowledge_lens.rs`, `kernel_handle/mod.rs` |
| 1 | `[`AgentTool`]` | `tools/kernel/mod.rs` |
| 1 | `[`ProviderPool`]` | `kernel_handle/engine_api.rs` (doc comment) |
| 1 | `[`build`]` / `[`build_for`]` / `[`engine`]` | `capability/template.rs` |
| 1 | `[`0`]` | `project/detection.rs` |

**Root cause:** Most broken links reference types that are either in a different crate (not `doc`-linked), or are private/internal types. `KernelHandle` is referenced heavily in tool `from_kernel()` doc comments but isn't importable from the tool modules' scope.

**Fix pattern:** Replace `[`TypeName`]` with `` `TypeName` `` (backticks, no link) for types that shouldn't be cross-referenced, or add correct paths.

### Missing Documentation (6 warnings)

| Count | Location |
|-------|----------|
| 5 | `kernel_handle/engine_api.rs:68-71` — struct fields on `EngineRoutingConfig` |
| 1 | `kernel_handle/engine_api.rs:560` — `record_usage_to_stats()` function |

### HTML Issues (1 warning)

| Location | Issue |
|----------|-------|
| `credential.rs:27` | Unclosed `<PROVIDER>` in doc comment: `OXIOS_<PROVIDER>_API_KEY` |

**Fix:** Change to `` `OXIOS_<PROVIDER>`_API_KEY `` or escape the angle brackets.

---

## Top 5 Most Important Public Types Lacking Docs

These are high-visibility types with no doc comments at all:

| # | Type | Location | Importance |
|---|------|----------|------------|
| 1 | `AgentPool` | `supervisor.rs:52` | Core agent lifecycle — manages running agent pool |
| 2 | `KernelError` | `error.rs:10` | Central error type — every kernel consumer hits this |
| 3 | `AgentLifecycleManager` | `agent_lifecycle.rs:23` | Full agent lifecycle orchestration (fork→schedule→run→cleanup) |
| 4 | `BudgetInfo` / `BudgetLimit` | `budget.rs:18-42` | Budget enforcement types — used by every agent |
| 5 | `Persona` | `persona.rs:12` | Persona system — no docs on the core struct |

**Estimated total:** ~309 public items without doc comments (out of ~1203 total public items = ~26% undocumented).

---

## Navigation & Usability

The generated docs at `target/doc/oxios_kernel/index.html` are:

- ✅ **Navigable** — Standard rustdoc layout with sidebar, search, module tree
- ✅ **Searchable** — search-index.js generated (684 pages indexed)
- ✅ **Module-organized** — All 30+ pub modules visible from root page
- ⚠️ **Broken links** — Clicking `[KernelHandle]` in tool docs goes nowhere
- ⚠️ **Sparse descriptions** — Many struct pages show fields with no descriptions
- ⚠️ **No doc-tests shown** — Most examples are `ignore` so they don't render as tested

---

## Recommendations (Priority Order)

1. **Fix broken links** (16 warnings) — Replace `[`KernelHandle`]` with backtick-quoted `` `KernelHandle` `` in tool modules. This is the single biggest quality improvement.

2. **Fix HTML issue** (1 warning) — Escape `<PROVIDER>` in credential.rs.

3. **Doc the top 5 types** — AgentPool, KernelError, AgentLifecycleManager, BudgetInfo, Persona. Even a one-liner each helps.

4. **Doc EngineRoutingConfig fields** — 5 missing field docs in engine_api.rs.

5. **Fix oxios-gateway compile error** — So its docs generate too.
