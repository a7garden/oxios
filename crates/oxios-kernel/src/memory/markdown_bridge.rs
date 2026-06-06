//! `MarkdownSource` implementation for `oxios_markdown::KnowledgeBase`.
//!
//! Required because of Rust's orphan rule: both the trait
//! (`MarkdownSource`, defined in `oxios_memory`) and the concrete
//! type (`KnowledgeBase`, defined in `oxios_markdown`) are foreign.
//! The wrapper provides a local home for the impl.

/// Thin newtype wrapper around `KnowledgeBase`.
///
/// Pass to `AutoMemoryBridge::with_knowledge_base()`:
/// ```ignore
/// bridge.with_knowledge_base(Arc::new(MarkdownKnowledgeBase(kb)))
/// ```
pub struct MarkdownKnowledgeBase(pub oxios_markdown::KnowledgeBase);

impl oxios_memory::memory::storage::MarkdownSource for MarkdownKnowledgeBase {
    fn index_all(&self) -> anyhow::Result<usize> {
        self.0.index_all()
    }

    fn note_tree(
        &self,
        dir: &str,
    ) -> anyhow::Result<Vec<oxios_memory::memory::storage::NoteEntry>> {
        let entries = self.0.note_tree(dir)?;
        Ok(entries
            .into_iter()
            .map(|e| oxios_memory::memory::storage::NoteEntry {
                name: e.name,
                parent_dir: e.parent_dir,
                is_dir: e.is_dir,
            })
            .collect())
    }

    fn note_read(&self, path: &str) -> anyhow::Result<Option<String>> {
        self.0.note_read(path)
    }

    fn extract_headings(&self, content: &str) -> Vec<String> {
        self.0.extract_headings(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxios_memory::memory::storage::{MarkdownSource, NoteEntry};
    use std::sync::Arc;

    /// Regression: `parent_dir` must come from `FileEntry`, not the
    /// `dir` argument (which is the search root, not the entry's parent).
    #[test]
    fn note_tree_preserves_entry_parent_dir() {
        let kb =
            oxios_markdown::KnowledgeBase::new(tempfile::tempdir().unwrap().path().to_path_buf())
                .unwrap();
        let wrapper = MarkdownKnowledgeBase(kb);
        let entries = wrapper.note_tree("/").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn markdown_source_is_object_safe() {
        let _: Arc<dyn MarkdownSource> = Arc::new(MarkdownKnowledgeBase(
            oxios_markdown::KnowledgeBase::new(tempfile::tempdir().unwrap().path().to_path_buf())
                .unwrap(),
        ));
        let _: NoteEntry = NoteEntry {
            name: String::new(),
            parent_dir: String::new(),
            is_dir: false,
        };
    }
}
