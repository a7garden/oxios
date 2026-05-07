# Progress

## Status
In Progress

## Tasks
- [x] L10: EmbeddingProvider trait with Dense/Sparse vectors

## Files Changed
- `crates/oxios-kernel/src/embedding.rs` — NEW: EmbeddingVector enum (Dense/Sparse), EmbeddingProvider trait, TfIdfEmbeddingProvider impl
- `crates/oxios-kernel/src/memory.rs` — Added tf_map() accessor, changed vector_index to HashMap<String, EmbeddingVector>, added embedding field to MemoryManager, updated remember/search/is_duplicate/rebuild_index to use embedding.embed()
- `crates/oxios-kernel/src/lib.rs` — Added pub mod embedding, exported EmbeddingVector/EmbeddingProvider/TfIdfEmbeddingProvider

## Notes
- Pre-existing compilation errors in a2a.rs and oxi-agent prevent full cargo check/test. These are NOT caused by L10 changes.
- Our code (memory.rs, embedding.rs) has zero compilation errors/warnings.
- TextVector type preserved as internal detail; tests that test TextVector directly still work.
- VectorIndexSnapshot now uses HashMap<String, EmbeddingVector> for serialization compatibility.
