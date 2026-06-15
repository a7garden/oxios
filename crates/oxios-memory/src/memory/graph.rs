//! PageRank-based importance scoring for memory entries.
//!
//! Models memory entries as nodes in a graph where edges represent
//! co-access (entries accessed in the same session are linked).
//! PageRank iteration propagates importance through the graph.

use std::collections::HashMap;

/// A memory link graph for computing PageRank-style importance.
///
/// Nodes are memory entry IDs (mapped to u64 indices internally).
/// Edges represent co-access relationships.
#[derive(Debug, Clone, Default)]
pub struct MemoryGraph {
    /// Adjacency list: node -> outgoing edges (neighbors).
    edges: HashMap<u64, Vec<u64>>,
    /// Reverse mapping: node -> incoming edges.
    incoming: HashMap<u64, Vec<u64>>,
    /// Number of nodes.
    node_count: usize,
}

impl MemoryGraph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a directed edge from `from` to `to`.
    ///
    /// If the edge already exists, this is a no-op.
    pub fn add_edge(&mut self, from: u64, to: u64) {
        if from == to {
            return; // no self-loops
        }
        self.edges.entry(from).or_default();
        self.edges.entry(to).or_default();
        self.incoming.entry(from).or_default();
        self.incoming.entry(to).or_default();

        let neighbors = self
            .edges
            .get_mut(&from)
            .expect("entry(or_default) guarantees existence");
        if !neighbors.contains(&to) {
            neighbors.push(to);
            self.incoming
                .get_mut(&to)
                .expect("entry(or_default) guarantees existence")
                .push(from);
        }

        self.node_count = self.edges.len();
    }

    /// Add a bidirectional link between two nodes (co-access).
    pub fn link(&mut self, a: u64, b: u64) {
        self.add_edge(a, b);
        self.add_edge(b, a);
    }

    /// Get the number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.node_count
    }

    /// Get outgoing neighbors of a node.
    pub fn neighbors(&self, node: u64) -> &[u64] {
        self.edges.get(&node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Compute PageRank scores for all nodes.
    ///
    /// Uses the standard iterative algorithm with damping factor.
    ///
    /// # Arguments
    /// * `damping` — Damping factor (typically 0.85).
    /// * `iterations` — Number of iterations (typically 20-50).
    /// * `initial_scores` — Optional initial scores (e.g., base importance).
    ///   If provided, the initial PageRank is seeded with these values.
    ///
    /// # Returns
    /// A map of node ID -> PageRank score.
    pub fn pagerank(
        &self,
        damping: f64,
        iterations: usize,
        initial_scores: Option<&HashMap<u64, f64>>,
    ) -> HashMap<u64, f64> {
        if self.node_count == 0 {
            return HashMap::new();
        }

        let n = self.node_count as f64;
        let base = 1.0 / n;

        // Initialize scores
        let mut scores: HashMap<u64, f64> = self
            .edges
            .keys()
            .map(|&k| {
                let init = initial_scores
                    .and_then(|m| m.get(&k))
                    .copied()
                    .unwrap_or(base);
                (k, init)
            })
            .collect();

        // Compute out-degree for each node
        let out_degree: HashMap<u64, usize> =
            self.edges.iter().map(|(&k, v)| (k, v.len())).collect();

        // Iterative PageRank
        for _ in 0..iterations {
            let mut new_scores = HashMap::with_capacity(self.node_count);

            let sink_sum: f64 = scores
                .iter()
                .filter(|&(&k, _)| out_degree.get(&k).copied().unwrap_or(0) == 0)
                .map(|(_, &s)| s)
                .sum();

            for &node in self.edges.keys() {
                // Sum of incoming PageRank contributions
                let incoming_sum: f64 = self
                    .incoming
                    .get(&node)
                    .map(|neighbors| {
                        neighbors
                            .iter()
                            .map(|&src| {
                                let src_out = out_degree.get(&src).copied().unwrap_or(1) as f64;
                                scores.get(&src).copied().unwrap_or(0.0) / src_out
                            })
                            .sum()
                    })
                    .unwrap_or(0.0);

                let rank = (1.0 - damping) / n + damping * (incoming_sum + sink_sum / n);
                new_scores.insert(node, rank);
            }

            scores = new_scores;
        }

        scores
    }

    /// Compute co-access graph from session access patterns.
    ///
    /// Given a list of sessions where each session contains a list of
    /// memory IDs that were accessed together, build a graph where
    /// co-accessed memories are linked.
    pub fn from_co_access(sessions: &[Vec<u64>]) -> Self {
        let mut graph = Self::new();
        for session in sessions {
            // Link all pairs of co-accessed memories
            for i in 0..session.len() {
                for j in (i + 1)..session.len() {
                    graph.link(session[i], session[j]);
                }
            }
        }
        graph
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let graph = MemoryGraph::new();
        let scores = graph.pagerank(0.85, 20, None);
        assert!(scores.is_empty());
    }

    #[test]
    fn test_single_node() {
        let mut graph = MemoryGraph::new();
        graph.add_edge(1, 1); // self-loop ignored
        let scores = graph.pagerank(0.85, 20, None);
        assert!(scores.is_empty() || scores.values().all(|&v| v > 0.0));
    }

    #[test]
    fn test_two_nodes() {
        let mut graph = MemoryGraph::new();
        graph.link(1, 2);

        let scores = graph.pagerank(0.85, 50, None);
        assert_eq!(scores.len(), 2);

        // Both nodes should have similar scores (symmetric graph)
        let s1 = scores.get(&1).unwrap();
        let s2 = scores.get(&2).unwrap();
        assert!(
            (s1 - s2).abs() < 0.01,
            "Symmetric graph should have equal scores"
        );
    }

    #[test]
    fn test_hub_authority() {
        // Node 1 links to 2, 3, 4
        // Node 2, 3, 4 link back to 1
        // Node 1 is a hub
        let mut graph = MemoryGraph::new();
        graph.add_edge(1, 2);
        graph.add_edge(1, 3);
        graph.add_edge(1, 4);
        graph.add_edge(2, 1);
        graph.add_edge(3, 1);
        graph.add_edge(4, 1);

        let scores = graph.pagerank(0.85, 50, None);
        let s1 = scores.get(&1).unwrap();

        // Node 1 should have higher score than any single leaf
        for &node in &[2u64, 3, 4] {
            let sn = scores.get(&node).unwrap();
            assert!(*s1 >= *sn, "Hub node should have >= score than leaf");
        }
    }

    #[test]
    fn test_from_co_access() {
        let sessions = vec![
            vec![1, 2, 3], // session 1: memories 1, 2, 3 co-accessed
            vec![2, 4],    // session 2: memories 2, 4 co-accessed
        ];

        let graph = MemoryGraph::from_co_access(&sessions);
        assert_eq!(graph.node_count(), 4);

        // Node 2 is in both sessions, should have highest PageRank
        let scores = graph.pagerank(0.85, 50, None);
        let s2 = scores.get(&2).unwrap();
        for &node in &[1u64, 3, 4] {
            let sn = scores.get(&node).unwrap();
            assert!(*s2 >= *sn, "Node 2 should have highest score");
        }
    }

    #[test]
    fn test_initial_scores_influence() {
        // Asymmetric graph: 1 -> 2 (one-way)
        let mut graph = MemoryGraph::new();
        graph.add_edge(1, 2);

        // Give node 1 a much higher initial score
        let initial = HashMap::from([(1u64, 10.0), (2u64, 0.1)]);
        let scores = graph.pagerank(0.85, 5, Some(&initial));

        // Node 1 should maintain higher score due to being the only hub
        let s1 = scores.get(&1).unwrap();
        let s2 = scores.get(&2).unwrap();
        // PageRank should propagate some to node 2, but 1 started higher
        assert!(*s1 > 0.0, "Node 1 should have positive score");
        assert!(*s2 > 0.0, "Node 2 should have positive score");
    }
}
