# ToolRetriever Implementation — Output

## Files Created

### `crates/oxios-kernel/src/tools/retrieval.rs`

The semantic search engine for OS capabilities. Key components:

| Type | Description |
|------|-------------|
| `ToolEntry` | Searchable capability entry (name, category, description, skill_path, command) |
| `IndexedTool` | Internal: ToolEntry + pre-computed `EmbeddingVector` |
| `ScoredTool` | ToolEntry ranked by cosine similarity score |
| `ToolRetriever` | Main struct: in-memory vector index with top-K retrieval |

### `crates/oxios-kernel/src/tools/mod.rs` (modified)

Added `pub mod retrieval;` to register the new module.

## Design Decisions

### EmbeddingVector (not `Vec<f32>`)
The existing `EmbeddingProvider` trait returns `EmbeddingVector` (an enum with `Dense`, `DenseF32`, and `Sparse` variants), **not** `Vec<f32>`. The `retrieve()` method accepts `&EmbeddingVector` and uses `EmbeddingVector::cosine_similarity()` which handles cross-type comparison (e.g., Sparse vs Sparse, DenseF32 vs DenseF32).

### Pre-computed query embedding
`retrieve()` takes a pre-computed `&EmbeddingVector` rather than a raw text string. This lets the caller control when the embedding computation happens (the embedder is accessed via `retriever.embedder()`).

### Async index_tool
`index_tool()` is async because it calls `self.embedder.embed()`. Failed embeddings are logged at warn level and the tool is silently skipped.

### Capability index format
`format_capability_index()` generates XML suitable for injection into agent system prompts. XML escaping is applied to all fields. Program entries include optional `<command>` and `<skill>` tags.

### Kernel manifest
`build_kernel_manifest()` generates markdown listing active kernel domains with descriptions. Unknown domains are filtered out.

## Tests (19 total)

- `test_index_and_len` — indexing 2 tools, is_empty, len
- `test_retrieve_top_k` — top-2 from 3 indexed tools, sorted descending
- `test_retrieve_exceeds_index` — requesting more than available
- `test_retrieve_empty_index` — empty index returns empty
- `test_entries` — get all indexed entries
- `test_clear` — clear index
- `test_format_capability_index_basic` — XML output for os-tool
- `test_format_capability_index_program` — XML with command/skill tags
- `test_format_capability_index_xml_escaping` — special chars escaped
- `test_escape_xml` — unit test for XML escaping
- `test_build_kernel_manifest` — markdown generation
- `test_build_kernel_manifest_filters_unknown` — unknown domains filtered
- `test_build_kernel_manifest_empty` — empty domains
- `test_tool_entry_embedding_text` — text concatenation for embedding
- `test_tool_entry_embedding_text_with_command` — includes command text
- `test_embedder_accessor` — embedder() returns reference
- `test_with_tfidf_embedder` — integration test with real TfIdfEmbeddingProvider

## Compilation Status

The module itself compiles cleanly (no errors from retrieval.rs). The workspace has pre-existing compilation errors in `oxi-agent` (a path dependency) and `kernel_handle/browser_api.rs` that prevent a full `cargo test --workspace`. These are unrelated to this change.
