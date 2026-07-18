//! Bidirectional link tracking between markdown notes.
//!
//! Tracks `[text](path)` links in the knowledge base, enabling
//! forward-link and backlink queries in O(1) time.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::parser::extract_markdown_links;

/// A single backlink: a link from one note to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backlink {
    /// File that contains the link.
    pub source_path: String,
    /// File that the link points to.
    pub target_path: String,
    /// Link display text.
    pub link_text: String,
    /// Line number where the link appears (1-indexed).
    pub line_number: usize,
}

/// Link graph data for visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkGraph {
    /// Node entries.
    pub nodes: Vec<LinkNode>,
    /// Edge entries.
    pub edges: Vec<LinkEdge>,
}

/// A node in the link graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkNode {
    /// File path (unique ID).
    pub id: String,
    /// Display label.
    pub label: String,
    /// Group (directory name).
    pub group: String,
}

/// An edge in the link graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkEdge {
    /// Source file path.
    pub source: String,
    /// Target file path.
    pub target: String,
    /// Link text.
    pub label: String,
}

/// Bidirectional link index.
///
/// Maintains forward links (source → targets) and backward links
/// (target → sources) for O(1) lookup.
#[derive(Debug, Clone, Default)]
pub struct BacklinkIndex {
    /// Forward: source_path → set of target_paths.
    forward: HashMap<String, HashSet<String>>,
    /// Backward: target_path → set of source_paths.
    backward: HashMap<String, HashSet<String>>,
    /// Detailed backlink records.
    details: HashMap<String, Vec<Backlink>>,
}

impl BacklinkIndex {
    /// Create a new empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Index all links in a file's content.
    ///
    /// Replaces any previously indexed links for this file (incremental update).
    /// Markdown links only — wikilinks are not resolved.
    pub fn index_file(&mut self, path: &str, content: &str) {
        self.index_file_inner(path, content, None);
    }

    /// Index both markdown links AND wikilinks. Wikilink targets are
    /// resolved against `stem_index` (basename → paths) so a bare
    /// `[[Rust]]` lands under the canonical path it actually points at.
    /// Ambiguous/unresolved wikilinks are silently skipped — they're not
    /// indexed, so a later rename of any single candidate won't rewrite
    /// them (design doc §6).
    pub fn index_file_with(
        &mut self,
        path: &str,
        content: &str,
        stem_index: &crate::parser::StemIndex,
    ) {
        self.index_file_inner(path, content, Some(stem_index));
    }

    fn index_file_inner(
        &mut self,
        path: &str,
        content: &str,
        stem_index: Option<&crate::parser::StemIndex>,
    ) {
        let body = strip_frontmatter(content);
        let md_links = extract_markdown_links(body);
        let wiki_links = match stem_index {
            Some(_) => crate::parser::extract_wikilinks(body),
            None => Vec::new(),
        };

        // Tear down the previous forward set for this source so re-indexing
        // never accumulates stale entries.
        if let Some(old_targets) = self.forward.remove(path) {
            for target in &old_targets {
                if let Some(sources) = self.backward.get_mut(target) {
                    sources.remove(path);
                }
            }
        }
        self.details
            .retain(|k, _| !k.starts_with(&format!("{path}→")));

        // Canonical targets this file points at. Markdown links are already
        // path-shaped (their captured target IS the key); wikilinks are
        // resolved to a canonical path before keying, so both link kinds
        // unify under the same backward[target] bucket.
        let mut new_targets: HashSet<String> = HashSet::new();
        for (text, target) in &md_links {
            new_targets.insert(target.clone());
            self.backward
                .entry(target.clone())
                .or_default()
                .insert(path.to_string());
            self.details.insert(
                format!("{path}→{target}"),
                vec![Backlink {
                    source_path: path.to_string(),
                    target_path: target.clone(),
                    link_text: text.clone(),
                    line_number: 0,
                }],
            );
        }
        for (target, alias) in &wiki_links {
            let Some(canonical) =
                crate::parser::resolve_wikilink(target, Some(path), stem_index.unwrap())
            else {
                continue;
            };
            new_targets.insert(canonical.clone());
            self.backward
                .entry(canonical.clone())
                .or_default()
                .insert(path.to_string());
            self.details.insert(
                format!("{path}→{canonical}"),
                vec![Backlink {
                    source_path: path.to_string(),
                    target_path: canonical.clone(),
                    link_text: alias.clone().unwrap_or_else(|| target.clone()),
                    line_number: 0,
                }],
            );
        }
        self.forward.insert(path.to_string(), new_targets);
    }

    /// Remove a file from the index.
    pub fn remove_file(&mut self, path: &str) {
        if let Some(targets) = self.forward.remove(path) {
            for target in &targets {
                if let Some(sources) = self.backward.get_mut(target) {
                    sources.remove(path);
                }
            }
        }
        for sources in self.backward.values_mut() {
            sources.remove(path);
        }
        self.details.retain(|k, _| !k.contains(path));
    }

    /// Get all backlinks pointing to a file (files that reference this one).
    pub fn backlinks_for(&self, path: &str) -> Vec<Backlink> {
        let sources = self.backward.get(path).cloned().unwrap_or_default();
        let mut result = Vec::new();
        for source in &sources {
            let key = format!("{source}→{path}");
            if let Some(details) = self.details.get(&key) {
                result.extend(details.clone());
            }
        }
        result
    }

    /// Get the set of source files that link to `target` (the backward index
    /// entry). Used by `note_move` to find every file whose links must be
    /// rewritten when the target is renamed.
    pub fn sources_for(&self, target: &str) -> HashSet<String> {
        self.backward.get(target).cloned().unwrap_or_default()
    }

    /// Get all forward links from a file (files this one references).
    pub fn forward_links_for(&self, path: &str) -> Vec<String> {
        self.forward
            .get(path)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect()
    }

    /// Get the number of backlinks for a file.
    pub fn backlink_count(&self, path: &str) -> usize {
        self.backward.get(path).map(|s| s.len()).unwrap_or(0)
    }

    /// Get the full link graph for visualization.
    pub fn link_graph(&self) -> LinkGraph {
        let mut node_set = HashSet::new();
        let mut edges = Vec::new();

        for (source, targets) in &self.forward {
            node_set.insert(source.clone());
            for target in targets {
                node_set.insert(target.clone());
                edges.push(LinkEdge {
                    source: source.clone(),
                    target: target.clone(),
                    label: String::new(),
                });
            }
        }

        let nodes: Vec<LinkNode> = node_set
            .into_iter()
            .map(|id| {
                let label = id
                    .trim_end_matches(".md")
                    .rsplit('/')
                    .next()
                    .unwrap_or(&id)
                    .to_string();
                let group = id.split('/').next().unwrap_or("").to_string();
                LinkNode { id, label, group }
            })
            .collect();

        LinkGraph { nodes, edges }
    }

    /// Compute connection strength between two files (shared backlink sources).
    pub fn connection_strength(&self, path_a: &str, path_b: &str) -> usize {
        let sources_a = self.backward.get(path_a).cloned().unwrap_or_default();
        let sources_b = self.backward.get(path_b).cloned().unwrap_or_default();
        sources_a.intersection(&sources_b).count()
    }

    /// Number of files in the index.
    pub fn len(&self) -> usize {
        self.forward.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.forward.is_empty()
    }

    /// Clear all indexed data.
    pub fn clear(&mut self) {
        self.forward.clear();
        self.backward.clear();
        self.details.clear();
    }
}

/// Strip YAML frontmatter from content, returning the body.
/// If no frontmatter is found, returns the original content unchanged.
pub fn strip_frontmatter(content: &str) -> &str {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return content;
    }
    // Skip the opening ---
    let after_first = &trimmed[3..];
    let rest = after_first.trim_start_matches(['-', '\n', '\r']);
    if let Some(idx) = rest.find("\n---") {
        let body_start = idx + 4;
        rest[body_start..].trim_start()
    } else {
        content
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_and_backlinks() {
        let mut idx = BacklinkIndex::new();
        idx.index_file(
            "brain/Rust.md",
            "See [Ownership](brain/Ownership.md) and [Go](brain/Go.md)",
        );

        let bl = idx.backlinks_for("brain/Ownership.md");
        assert_eq!(bl.len(), 1);
        assert_eq!(bl[0].source_path, "brain/Rust.md");
    }

    #[test]
    fn test_forward_links() {
        let mut idx = BacklinkIndex::new();
        idx.index_file("a.md", "[b](b.md) [c](c.md)");
        let fwd = idx.forward_links_for("a.md");
        assert_eq!(fwd.len(), 2);
    }

    #[test]
    fn test_remove_file() {
        let mut idx = BacklinkIndex::new();
        idx.index_file("a.md", "[b](b.md)");
        idx.remove_file("a.md");
        assert!(idx.backlinks_for("b.md").is_empty());
    }

    #[test]
    fn test_connection_strength() {
        let mut idx = BacklinkIndex::new();
        idx.index_file("x.md", "[a](a.md) [b](b.md)");
        idx.index_file("y.md", "[a](a.md) [b](b.md)");
        assert_eq!(idx.connection_strength("a.md", "b.md"), 2);
    }

    #[test]
    fn test_link_graph() {
        let mut idx = BacklinkIndex::new();
        idx.index_file("brain/A.md", "[B](brain/B.md)");
        let graph = idx.link_graph();
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.nodes.len(), 2);
    }

    #[test]
    fn test_update_replaces_old_links() {
        let mut idx = BacklinkIndex::new();
        idx.index_file("a.md", "[old](old.md)");
        idx.index_file("a.md", "[new](new.md)");
        assert!(idx.backlinks_for("old.md").is_empty());
        assert_eq!(idx.backlinks_for("new.md").len(), 1);
    }
}
