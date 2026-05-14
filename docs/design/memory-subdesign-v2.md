# Oxios Memory System — 하위 세부설계 (Phase 1-4)

> **문서 계층**: 세부설계 (Sub-Design)
> **상위 문서**: `memory-design.md`
> **상태**: 2차 초안 (Ruflo v3 분석 후 개정)
> **날짜**: 2026-05-14

---

## 1. Module Structure (Revised)

### 1.1 Final Module Layout

```
crates/oxios-kernel/src/
├── embedding.rs          # 수정: DenseVector, MultiProvider
├── memory/
│   ├── mod.rs           # 수정: MemoryManager + Graph integration
│   ├── store.rs         # 수정: SqliteIndex + Chunk support
│   ├── hnsw.rs          # 신규: HNSW 인덱스 (usearch)
│   ├── graph.rs         # 신규: MemoryGraph (PageRank + Communities)
│   ├── engine.rs        # 수정: OnnxEngine + OpenAI + Hybrid
│   ├── chunking.rs      # 신규: Document chunking
│   ├── normalizer.rs    # 신규: L2/FP16/INT8
│   ├── hyperbolic.rs    # 신규: Poincaré ball embeddings
│   ├── flash_attention.rs # 신규: Block-wise attention
│   ├── reasoning_bank.rs # 신규: Pattern store/search/route
│   ├── rvf_store.rs     # 신규: RVF Learning Store
│   ├── sona.rs          # 신규: SONA engine (Phase 2)
│   ├── migrate.rs       # 신규: TF-IDF → HNSW migration
│   └── error.rs         # 신규: 전용 에러 타입
├── workers/
│   ├── mod.rs           # 신규: WorkerManager
│   ├── types.rs         # 신규: Worker types
│   └── handlers.rs      # 신규: Worker implementations
└── events/
    └── rvf_event_log.rs # 신규: Binary event log (event_bus 대체)
```

### 1.2 Dependencies

```toml
# oxios-kernel/Cargo.toml

[dependencies]
# HNSW Vector Index
usearch = { version = "0.16", features = ["simd", "wasm"] }

# ONNX Runtime (pure Rust)
tract-onnx = { version = "0.26", features = ["onnx"] }

# Database
rusqlite = { version = "0.34", features = ["bundled"] }

# ML utilities
ndarray = "0.16"
num-traits = "0.2"

# Async HTTP (for OpenAI fallback)
reqwest = { version = "0.12", features = ["json"] }

# Serialization
bincode = "1.3"  # Binary encoding for RVF

[dev-dependencies]
criterion = "0.5"  # Benchmarking
tempfile = "3"

[features]
default = []
memory-hnsw = ["usearch", "tract-onnx", "rusqlite", "reqwest"]
memory-learning = ["memory-hnsw", "ndarray", "num-traits"]
memory-workers = ["memory-learning"]
rvf-events = ["bincode"]
```

---

## 2. MemoryGraph Implementation

### 2.1 Core Structure

```rust
// memory/graph.rs

use std::collections::{HashMap, HashSet, VecDeque};
use parking_lot::RwLock;

/// Edge type between memory entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    Reference,  // Explicit reference in content
    Similar,    // Auto-created from similarity > threshold
    Temporal,   // Sequential access in same session
    CoAccessed, // Accessed together frequently
    Causal,     // Output used as input for another
}

/// Graph node representing a memory entry.
#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: String,
    pub category: String,
    pub confidence: f64,
    pub access_count: u32,
    pub created_at: i64,
}

/// Graph edge between nodes.
#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub target_id: String,
    pub edge_type: EdgeType,
    pub weight: f64,
}

/// Configuration for MemoryGraph.
#[derive(Debug, Clone)]
pub struct MemoryGraphConfig {
    pub similarity_threshold: f64,      // 0.8 default
    pub pagerank_damping: f64,           // 0.85 default
    pub pagerank_iterations: usize,      // 50 default
    pub pagerank_convergence: f64,      // 1e-6 default
    pub max_nodes: usize,                // 5000 default
    pub community_detection: bool,       // true default
}

impl Default for MemoryGraphConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.8,
            pagerank_damping: 0.85,
            pagerank_iterations: 50,
            pagerank_convergence: 1e-6,
            max_nodes: 5000,
            community_detection: true,
        }
    }
}

/// Knowledge graph for memory entries.
/// Computes PageRank for structural importance and community detection
/// for topic clustering.
pub struct MemoryGraph {
    config: MemoryGraphConfig,
    nodes: RwLock<HashMap<String, GraphNode>>,
    edges: RwLock<HashMap<String, Vec<GraphEdge>>>,
    reverse_edges: RwLock<HashMap<String, HashSet<String>>>,
    page_ranks: RwLock<HashMap<String, f64>>,
    communities: RwLock<HashMap<String, String>>,
    dirty: RwLock<bool>,
}

impl MemoryGraph {
    pub fn new(config: MemoryGraphConfig) -> Self {
        Self {
            config,
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(HashMap::new()),
            reverse_edges: RwLock::new(HashMap::new()),
            page_ranks: RwLock::new(HashMap::new()),
            communities: RwLock::new(HashMap::new()),
            dirty: RwLock::new(true),
        }
    }

    /// Add a node from a memory entry.
    pub fn add_node(&self, entry: &MemoryEntry) {
        if self.nodes.read().len() >= self.config.max_nodes {
            return;
        }

        let node = GraphNode {
            id: entry.id.clone(),
            category: entry.memory_type.category().to_string(),
            confidence: entry.importance as f64,
            access_count: entry.access_count,
            created_at: entry.created_at.timestamp(),
        };

        self.nodes.write().insert(entry.id.clone(), node);
        self.edges.write().entry(entry.id.clone()).or_default();
        *self.dirty.write() = true;
    }

    /// Add an edge based on reference.
    pub fn add_reference_edge(&self, from: &str, to: &str) {
        if !self.nodes.read().contains_key(from) || !self.nodes.read().contains_key(to) {
            return;
        }

        self.edges.write()
            .entry(from.to_string())
            .or_default()
            .push(GraphEdge {
                target_id: to.to_string(),
                edge_type: EdgeType::Reference,
                weight: 1.0,
            });

        self.reverse_edges.write()
            .entry(to.to_string())
            .or_insert_with(HashSet::new)
            .insert(from.to_string());

        *self.dirty.write() = true;
    }

    /// Add similarity-based edge.
    pub fn add_similarity_edge(&self, from: &str, to: &str, similarity: f64) {
        if similarity < self.config.similarity_threshold {
            return;
        }

        self.edges.write()
            .entry(from.to_string())
            .or_default()
            .push(GraphEdge {
                target_id: to.to_string(),
                edge_type: EdgeType::Similar,
                weight: similarity as f64,
            });

        *self.dirty.write() = true;
    }

    /// Add temporal edge (sequential access).
    pub fn add_temporal_edge(&self, from: &str, to: &str) {
        if !self.nodes.read().contains_key(from) || !self.nodes.read().contains_key(to) {
            return;
        }

        self.edges.write()
            .entry(from.to_string())
            .or_default()
            .push(GraphEdge {
                target_id: to.to_string(),
                edge_type: EdgeType::Temporal,
                weight: 0.5,
            });

        *self.dirty.write() = true;
    }

    /// Compute PageRank algorithm.
    pub fn compute_pagerank(&self) {
        let nodes = self.nodes.read();
        let edges = self.edges.read();

        if nodes.is_empty() {
            return;
        }

        let mut ranks: HashMap<String, f64> = nodes.keys()
            .map(|k| (k.clone(), 1.0 / nodes.len() as f64))
            .collect();

        let damping = self.config.pagerank_damping;
        let n = nodes.len() as f64;

        for _iter in 0..self.config.pagerank_iterations {
            let mut new_ranks: HashMap<String, f64> = HashMap::new();
            let mut max_diff = 0.0_f64;

            for (node_id, _) in &nodes {
                // Sum contributions from all incoming edges
                let incoming: f64 = self.reverse_edges.read()
                    .get(node_id)
                    .map(|incoming_nodes| {
                        incoming_nodes.iter()
                            .filter_map(|source| {
                                edges.get(source).map(|edge_list| {
                                    let out_degree = edge_list.len();
                                    if out_degree == 0 {
                                        return 0.0;
                                    }
                                    let contribution: f64 = ranks.get(source).unwrap_or(&0.0);
                                    let edge_weight: f64 = edge_list.iter()
                                        .find(|e| e.target_id == *node_id)
                                        .map(|e| e.weight)
                                        .unwrap_or(1.0);
                                    contribution * edge_weight / out_degree as f64
                                })
                            })
                            .sum()
                    })
                    .unwrap_or(0.0);

                let new_rank = (1.0 - damping) / n + damping * incoming;
                max_diff = max_diff.max((new_rank - ranks.get(node_id).unwrap_or(&0.0)).abs());
                new_ranks.insert(node_id.clone(), new_rank);
            }

            ranks = new_ranks;

            // Convergence check
            if max_diff < self.config.pagerank_convergence {
                break;
            }
        }

        *self.page_ranks.write() = ranks;
        *self.dirty.write() = false;
    }

    /// Detect communities using label propagation.
    pub fn detect_communities(&self) {
        let nodes = self.nodes.read();
        let edges = self.edges.read();
        let n = nodes.len();

        if n == 0 {
            return;
        }

        // Initialize: each node gets its own label
        let mut labels: HashMap<String, String> = nodes.keys()
            .map(|k| (k.clone(), k.clone()))
            .collect();

        let mut queue: VecDeque<String> = nodes.keys().cloned().collect();
        let mut rng = rand_simple();

        for _ in 0..100 {  // Max iterations
            if queue.is_empty() {
                break;
            }

            // Shuffle for randomness
            for i in 0..queue.len() {
                let j = (rng() % (queue.len() - i)) as usize + i;
                queue.swap(i, j);
            }

            let node_id = queue.pop_front().unwrap();

            // Count label frequencies among neighbors
            let neighbor_labels: HashMap<String, usize> = edges.get(&node_id)
                .map(|edge_list| {
                    edge_list.iter()
                        .map(|e| e.target_id.clone())
                        .chain(
                            self.reverse_edges.read()
                                .get(&node_id)
                                .map(|s| s.iter().cloned())
                                .unwrap_or(std::iter::empty())
                        )
                        .filter_map(|nid| labels.get(&nid).cloned())
                        .fold(HashMap::new(), |mut m, l| {
                            *m.entry(l).or_insert(0) += 1;
                            m
                        })
                })
                .unwrap_or_default();

            if let Some((label, _)) = neighbor_labels.into_iter().max_by_key(|(_, c)| *c) {
                labels.insert(node_id.clone(), label);
            }

            // Re-queue neighbors
            if let Some(neighbor_edges) = edges.get(&node_id) {
                for edge in neighbor_edges {
                    if !queue.contains(&edge.target_id) {
                        queue.push_back(edge.target_id.clone());
                    }
                }
            }
        }

        *self.communities.write() = labels;
    }

    /// Rank HNSW search results using combined scores.
    pub fn rank_results(
        &self,
        hnsw_results: Vec<(String, f64)>,
        alpha: f64,
        beta: f64,
    ) -> Vec<RankedResult> {
        // Ensure PageRank is up to date
        if *self.dirty.read() {
            self.compute_pagerank();
            if self.config.community_detection {
                self.detect_communities();
            }
        }

        let page_ranks = self.page_ranks.read();
        let communities = self.communities.read();
        let nodes = self.nodes.read();

        let results: Vec<RankedResult> = hnsw_results
            .into_iter()
            .map(|(id, vector_score)| {
                let page_rank = page_ranks.get(&id).copied().unwrap_or(1.0 / nodes.len() as f64);
                
                // Get community
                let community = communities.get(&id).cloned();

                // Community boost: same community gets slight boost
                // (simplified: just include community info)
                let community_boost = 1.0; // Could be 1.1 if sharing community

                let combined = alpha * vector_score + beta * page_rank 
                    + (1.0 - alpha - beta) * community_boost;

                RankedResult {
                    id,
                    vector_score,
                    page_rank,
                    combined_score: combined,
                    community,
                    rank: 0, // Will be set after sorting
                }
            })
            .collect();

        // Sort by combined score descending
        let mut sorted = results;
        sorted.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
        
        // Assign ranks
        for (i, r) in sorted.iter_mut().enumerate() {
            r.rank = i + 1;
        }

        sorted
    }

    /// Get graph statistics.
    pub fn stats(&self) -> GraphStats {
        let nodes = self.nodes.read();
        let edges = self.edges.read();
        let communities = self.communities.read();
        let page_ranks = self.page_ranks.read();

        let total_edges: usize = edges.values().map(|v| v.len()).sum();
        let avg_degree = if nodes.is_empty() {
            0.0
        } else {
            total_edges as f64 / nodes.len() as f64
        };

        let unique_communities = communities.values().collect::<HashSet<_>>().len();

        let (max_pr, min_pr) = if page_ranks.is_empty() {
            (0.0, 0.0)
        } else {
            let ranks: Vec<_> = page_ranks.values().collect();
            (
                *ranks.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                *ranks.iter().cloned().fold(f64::INFINITY, f64::min),
            )
        };

        GraphStats {
            node_count: nodes.len(),
            edge_count: total_edges,
            avg_degree,
            community_count: unique_communities,
            pagerank_computed: !page_ranks.is_empty(),
            max_pagerank: max_pr,
            min_pagerank: min_pr,
        }
    }
}

/// Ranked result combining vector and graph scores.
#[derive(Debug, Clone)]
pub struct RankedResult {
    pub id: String,
    pub vector_score: f64,
    pub page_rank: f64,
    pub combined_score: f64,
    pub community: Option<String>,
    pub rank: usize,
}

/// Graph statistics.
#[derive(Debug, Clone)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub avg_degree: f64,
    pub community_count: usize,
    pub pagerank_computed: bool,
    pub max_pagerank: f64,
    pub min_pagerank: f64,
}

// Simple PRNG for label propagation (Mulberry32)
fn rand_simple() -> impl FnMut(usize) -> usize {
    let mut state = 42u64;
    move |max| {
        state = state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf36476ab7095b89);
        z = (z ^ (z >> 27)).wrapping_mul(0x9e3779b97f4a7c15);
        z = z ^ (z >> 31);
        (z as usize) % max
    }
}
```

---

## 3. Multi-Provider Embedding

### 3.1 Embedding Engine Trait (Revised)

```rust
// memory/engine.rs

/// Embedding generation result.
#[derive(Debug, Clone)]
pub struct EmbeddingResult {
    pub vector: DenseVector,
    pub engine: String,
    pub latency_ms: u64,
    pub cached: bool,
}

/// Batch embedding result.
#[derive(Debug, Clone)]
pub struct BatchEmbeddingResult {
    pub vectors: Vec<DenseVector>,
    pub engine: String,
    pub total_latency_ms: u64,
}

/// Configuration for embedding provider.
#[derive(Debug, Clone)]
pub enum EmbeddingConfig {
    /// Local ONNX model.
    Onnx {
        model_path: PathBuf,
        dimensions: usize,
    },
    /// OpenAI API.
    OpenAi {
        api_key: String,
        model: String,        // "text-embedding-3-small" or "text-embedding-3-large"
        dimensions: usize,
    },
    /// Local + API fallback.
    Hybrid {
        primary: Box<EmbeddingConfig>,
        fallback: Box<EmbeddingConfig>,
    },
    /// Local with OpenAI fallback.
    OnnxWithFallback {
        model_path: PathBuf,
        openai_api_key: String,
    },
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        EmbeddingConfig::Onnx {
            model_path: PathBuf::from("models/all-MiniLM-L6-v2.onnx"),
            dimensions: 384,
        }
    }
}

/// Embedding engine trait.
#[async_trait::async_trait]
pub trait EmbeddingEngine: Send + Sync {
    /// Generate embedding for single text.
    async fn embed(&self, text: &str) -> Result<EmbeddingResult, MemoryError>;

    /// Generate embeddings for batch of texts.
    async fn embed_batch(&self, texts: &[&str]) -> Result<BatchEmbeddingResult, MemoryError>;

    /// Engine name.
    fn name(&self) -> &str;

    /// Dimensions.
    fn dimensions(&self) -> usize;

    /// Check if engine is ready.
    async fn health_check(&self) -> Result<(), MemoryError>;

    /// Estimate token count (rough).
    fn estimate_tokens(&self, text: &str) -> usize;
}

/// Factory for creating embedding engines.
pub fn create_engine(config: &EmbeddingConfig) -> Result<Arc<dyn EmbeddingEngine>, MemoryError> {
    match config {
        EmbeddingConfig::Onnx { model_path, dimensions } => {
            let engine = OnnxEngine::load(model_path, *dimensions)?;
            Ok(Arc::new(engine))
        }
        EmbeddingConfig::OpenAi { api_key, model, dimensions } => {
            let engine = OpenAiEngine::new(api_key, model, *dimensions);
            Ok(Arc::new(engine))
        }
        EmbeddingConfig::Hybrid { primary, fallback } => {
            Ok(Arc::new(HybridEngine {
                primary: create_engine(primary)?,
                fallback: create_engine(fallback)?,
            }))
        }
        EmbeddingConfig::OnnxWithFallback { model_path, openai_api_key } => {
            let primary = create_engine(&EmbeddingConfig::Onnx {
                model_path: model_path.clone(),
                dimensions: 384,
            })?;
            let fallback = create_engine(&EmbeddingConfig::OpenAi {
                api_key: openai_api_key.clone(),
                model: "text-embedding-3-small".to_string(),
                dimensions: 1536,
            })?;
            Ok(Arc::new(HybridEngine { primary, fallback }))
        }
    }
}
```

### 3.2 ONNX Engine Implementation

```rust
// memory/engine.rs - OnnxEngine

use tract_onnx::prelude::*;

/// ONNX-based embedding engine using all-MiniLM-L6-v2.
pub struct OnnxEngine {
    model: Session,
    dimensions: usize,
    input_ids_name: String,
    attention_mask_name: String,
}

impl OnnxEngine {
    /// Load ONNX model from file.
    pub fn load(model_path: &Path, dimensions: usize) -> Result<Self, MemoryError> {
        if !model_path.exists() {
            return Err(MemoryError::ModelNotFound(format!(
                "Model not found: {}. Download from HuggingFace.",
                model_path.display()
            )));
        }

        // Load model using tract-onnx
        let model = tract_onnx::onnx()
            .model_for_path(model_path)
            .map_err(|e| MemoryError::EmbeddingFailed(e.to_string()))?;

        // Get input/output names
        let input_ids_name = model.input_names()[0].to_string();
        let attention_mask_name = model.input_names()[1].to_string();

        Ok(Self {
            model,
            dimensions,
            input_ids_name,
            attention_mask_name,
        })
    }

    /// Simple tokenization (for demo - use tokenizers crate in production).
    fn tokenize(&self, text: &str, max_length: usize) -> (Vec<i64>, Vec<i64>) {
        let tokens: Vec<i64> = text
            .chars()
            .filter(|c| !c.is_whitespace())
            .take(max_length - 2)
            .map(|c| (c as u32 % 50257) as i64)
            .collect();

        let mut input_ids = vec![101]; // [CLS]
        input_ids.extend(tokens);
        input_ids.push(102); // [SEP]

        let mut attention_mask = vec![1i64; input_ids.len()];
        while input_ids.len() < max_length {
            input_ids.push(0);
            attention_mask.push(0);
        }

        (input_ids, attention_mask)
    }

    fn run_inference(&self, input_ids: &[i64], attention_mask: &[i64]) 
        -> Result<Vec<f32>, MemoryError> 
    {
        // Create input tensors
        let input_shape = (1, input_ids.len());
        
        // Run inference (simplified - real implementation needs proper tensor creation)
        todo!("Implement ONNX inference with tract")
    }

    fn mean_pool(hidden_states: &[f32], attention_mask: &[i64], seq_len: usize) -> Vec<f32> {
        let hidden_dim = hidden_states.len() / seq_len;
        let mask = attention_mask.iter().take(seq_len)
            .map(|&x| x as f32)
            .collect::<Vec<_>>();
        let mask_sum: f32 = mask.iter().sum();
        
        if mask_sum == 0.0 {
            return vec![0.0; hidden_dim];
        }

        let mut result = vec![0.0; hidden_dim];
        for (i, h) in hidden_states.chunks(hidden_dim).enumerate() {
            let weight = mask[i];
            for (j, &v) in h.iter().enumerate() {
                result[j] += v * weight;
            }
        }
        
        for v in &mut result {
            *v /= mask_sum;
        }
        
        result
    }
}

#[async_trait::async_trait]
impl EmbeddingEngine for OnnxEngine {
    async fn embed(&self, text: &str) -> Result<EmbeddingResult, MemoryError> {
        let start = std::time::Instant::now();
        
        let (input_ids, attention_mask) = self.tokenize(text, 128);
        let hidden = self.run_inference(&input_ids, &attention_mask)?;
        let pooled = Self::mean_pool(&hidden, &input_ids, input_ids.len());
        
        let vector = DenseVector::new(pooled)?;
        let latency = start.elapsed().as_millis() as u64;
        
        Ok(EmbeddingResult {
            vector,
            engine: self.name().to_string(),
            latency_ms: latency,
            cached: false,
        })
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<BatchEmbeddingResult, MemoryError> {
        let start = std::time::Instant::now();
        
        let mut vectors = Vec::with_capacity(texts.len());
        for text in texts {
            let result = self.embed(text).await?;
            vectors.push(result.vector);
        }
        
        let latency = start.elapsed().as_millis() as u64;
        
        Ok(BatchEmbeddingResult {
            vectors,
            engine: self.name().to_string(),
            total_latency_ms: latency,
        })
    }

    fn name(&self) -> &str {
        "onnx-mini-lm-l6-v2"
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    async fn health_check(&self) -> Result<(), MemoryError> {
        Ok(())
    }

    fn estimate_tokens(&self, text: &str) -> usize {
        // Rough estimate: ~4 chars per token for English
        (text.len() / 4).max(1)
    }
}

/// OpenAI embedding engine.
pub struct OpenAiEngine {
    client: reqwest::Client,
    api_key: String,
    model: String,
    dimensions: usize,
    base_url: String,
}

impl OpenAiEngine {
    pub fn new(api_key: &str, model: &str, dimensions: usize) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            dimensions,
            base_url: "https://api.openai.com/v1".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl EmbeddingEngine for OpenAiEngine {
    async fn embed(&self, text: &str) -> Result<EmbeddingResult, MemoryError> {
        let start = std::time::Instant::now();
        
        #[derive(Deserialize)]
        struct EmbeddingResponse {
            data: Vec<EmbeddingData>,
        }
        #[derive(Deserialize)]
        struct EmbeddingData {
            embedding: Vec<f32>,
        }
        
        let response = self.client
            .post(format!("{}/embeddings", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "model": self.model,
                "input": text,
                "dimensions": self.dimensions,
            }))
            .send()
            .await
            .map_err(|e| MemoryError::EmbeddingFailed(e.to_string()))?;
        
        let result: EmbeddingResponse = response
            .json()
            .await
            .map_err(|e| MemoryError::EmbeddingFailed(e.to_string()))?;
        
        let vector = DenseVector::new(result.data[0].embedding.clone())?;
        let latency = start.elapsed().as_millis() as u64;
        
        Ok(EmbeddingResult {
            vector,
            engine: self.name().to_string(),
            latency_ms: latency,
            cached: false,
        })
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<BatchEmbeddingResult, MemoryError> {
        let start = std::time::Instant::now();
        
        let response = self.client
            .post(format!("{}/embeddings", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "model": self.model,
                "input": texts,
                "dimensions": self.dimensions,
            }))
            .send()
            .await
            .map_err(|e| MemoryError::EmbeddingFailed(e.to_string()))?;
        
        #[derive(Deserialize)]
        struct EmbeddingResponse {
            data: Vec<EmbeddingData>,
        }
        #[derive(Deserialize)]
        struct EmbeddingData {
            embedding: Vec<f32>,
        }
        
        let result: EmbeddingResponse = response
            .json()
            .await
            .map_err(|e| MemoryError::EmbeddingFailed(e.to_string()))?;
        
        let vectors = result.data.into_iter()
            .map(|d| DenseVector::new(d.embedding).unwrap())
            .collect();
        
        let latency = start.elapsed().as_millis() as u64;
        
        Ok(BatchEmbeddingResult {
            vectors,
            engine: self.name().to_string(),
            total_latency_ms: latency,
        })
    }

    fn name(&self) -> &str {
        &self.model
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    async fn health_check(&self) -> Result<(), MemoryError> {
        Ok(())
    }

    fn estimate_tokens(&self, text: &str) -> usize {
        (text.len() / 4).max(1)
    }
}

/// Hybrid engine with fallback.
pub struct HybridEngine {
    primary: Arc<dyn EmbeddingEngine>,
    fallback: Arc<dyn EmbeddingEngine>,
}

#[async_trait::async_trait]
impl EmbeddingEngine for HybridEngine {
    async fn embed(&self, text: &str) -> Result<EmbeddingResult, MemoryError> {
        match self.primary.embed(text).await {
            Ok(result) => Ok(result),
            Err(_) => {
                tracing::warn!("Primary embedding failed, using fallback");
                self.fallback.embed(text).await
            }
        }
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<BatchEmbeddingResult, MemoryError> {
        match self.primary.embed_batch(texts).await {
            Ok(result) => Ok(result),
            Err(_) => {
                tracing::warn!("Primary batch embedding failed, using fallback");
                self.fallback.embed_batch(texts).await
            }
        }
    }

    fn name(&self) -> &str {
        "hybrid"
    }

    fn dimensions(&self) -> usize {
        self.primary.dimensions()
    }

    async fn health_check(&self) -> Result<(), MemoryError> {
        self.primary.health_check().await
            .or_else(|_| self.fallback.health_check().await)
    }

    fn estimate_tokens(&self, text: &str) -> usize {
        self.primary.estimate_tokens(text)
    }
}
```

---

## 4. Document Chunking

### 4.1 Chunking Service

```rust
// memory/chunking.rs

/// Configuration for document chunking.
#[derive(Debug, Clone)]
pub struct ChunkingConfig {
    /// Target chunk size in characters.
    pub chunk_size: usize,
    /// Overlap between chunks (characters).
    pub overlap: usize,
    /// Maximum chunks per document.
    pub max_chunks: usize,
    /// Split on sentence boundaries.
    pub split_sentences: bool,
    /// Minimum chunk size.
    pub min_chunk_size: usize,
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        Self {
            chunk_size: 512,
            overlap: 50,
            max_chunks: 100,
            split_sentences: true,
            min_chunk_size: 50,
        }
    }
}

/// A text chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub text: String,
    pub offset: usize,
    pub length: usize,
    pub chunk_index: usize,
    pub total_chunks: usize,
}

/// Chunked document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkedDocument {
    pub original_id: String,
    pub chunks: Vec<Chunk>,
    pub total_chunks: usize,
    pub estimated_tokens: usize,
}

/// Split text into chunks.
pub fn chunk_text(text: &str, config: &ChunkingConfig) -> ChunkedDocument {
    let chunks: Vec<Chunk> = if config.split_sentences {
        split_into_sentences(text)
            .chunks(config.chunk_size)
            .enumerate()
            .map(|(i, sentences)| {
                let chunk_text = sentences.join(" ");
                let offset = sentences.iter().take(i).map(|s| s.len() + 1).sum();
                Chunk {
                    id: format!("{}-chunk-{}", text.hash(), i),
                    text: chunk_text,
                    offset,
                    length: chunk_text.len(),
                    chunk_index: i,
                    total_chunks: 0, // Will be set below
                }
            })
            .collect()
    } else {
        text.chars()
            .collect::<Vec<_>>()
            .chunks(config.chunk_size)
            .enumerate()
            .map(|(i, chars)| {
                let chunk_text: String = chars.iter().collect();
                let offset = i * config.chunk_size;
                Chunk {
                    id: format!("{}-chunk-{}", text.hash(), i),
                    text: chunk_text,
                    offset,
                    length: chunk_text.len(),
                    chunk_index: i,
                    total_chunks: 0,
                }
            })
            .collect()
    };

    let total = chunks.len();
    let chunks: Vec<Chunk> = chunks
        .into_iter()
        .filter(|c| c.text.len() >= config.min_chunk_size)
        .take(config.max_chunks)
        .map(|mut c| {
            c.total_chunks = total;
            c
        })
        .collect();

    let total_chunks = chunks.len();
    let estimated_tokens = chunks.iter()
        .map(|c| c.text.len() / 4)
        .sum();

    ChunkedDocument {
        original_id: text.hash().to_string(),
        chunks,
        total_chunks,
        estimated_tokens,
    }
}

fn split_into_sentences(text: &str) -> Vec<String> {
    // Simple sentence splitting
    let mut sentences = Vec::new();
    let mut current = String::new();

    for line in text.lines() {
        for ch in line.chars() {
            current.push(ch);
            if ['.', '!', '?'].contains(&ch) {
                sentences.push(current.trim().to_string());
                current.clear();
            }
        }
        if !current.is_empty() {
            current.push(' ');
        }
    }

    if !current.trim().is_empty() {
        sentences.push(current.trim().to_string());
    }

    sentences
}

fn split_into_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;

    for ch in text.chars() {
        current.push(ch);
        
        match ch {
            '"' | '\'' => in_quote = !in_quote,
            '.' | '!' | '?' if !in_quote => {
                sentences.push(current.trim().to_string());
                current.clear();
            }
            _ => {}
        }
    }

    if !current.trim().is_empty() {
        sentences.push(current.trim().to_string());
    }

    sentences
}
```

---

## 5. Normalization Utilities

### 5.1 Vector Normalization

```rust
// memory/normalizer.rs

/// Normalization type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizationType {
    L2,      // Unit vector
    L1,      // Sum to 1
    MinMax,   // Scale to [0, 1]
    ZScore,   // Mean 0, std 1
}

/// Normalize vector using specified method.
pub fn normalize(vector: &mut [f32], norm_type: NormalizationType) {
    match norm_type {
        NormalizationType::L2 => l2_normalize(vector),
        NormalizationType::L1 => l1_normalize(vector),
        NormalizationType::MinMax => minmax_normalize(vector),
        NormalizationType::ZScore => zscore_normalize(vector),
    }
}

/// L2 (Euclidean) normalization — makes unit vector.
pub fn l2_normalize(vector: &mut [f32]) {
    let norm = l2_norm(vector);
    if norm > 0.0 {
        for v in vector.iter_mut() {
            *v /= norm;
        }
    }
}

/// L2 norm of vector.
pub fn l2_norm(vector: &[f32]) -> f32 {
    vector.iter().map(|v| v * v).sum::<f32>().sqrt()
}

/// L1 normalization — sum of absolute values = 1.
pub fn l1_normalize(vector: &mut [f32]) {
    let sum: f32 = vector.iter().map(|v| v.abs()).sum();
    if sum > 0.0 {
        for v in vector.iter_mut() {
            *v /= sum;
        }
    }
}

/// Min-max normalization — scale to [0, 1].
pub fn minmax_normalize(vector: &mut [f32]) {
    if vector.is_empty() {
        return;
    }
    
    let min = vector.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = vector.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let range = max - min;
    
    if range > 0.0 {
        for v in vector.iter_mut() {
            *v = (*v - min) / range;
        }
    }
}

/// Z-score normalization — mean 0, std 1.
pub fn zscore_normalize(vector: &mut [f32]) {
    if vector.is_empty() {
        return;
    }
    
    let n = vector.len() as f32;
    let mean: f32 = vector.iter().sum::<f32>() / n;
    let variance: f32 = vector.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;
    let std = variance.sqrt();
    
    if std > 0.0 {
        for v in vector.iter_mut() {
            *v = (*v - mean) / std;
        }
    }
}

/// Convert f32 to FP16 for storage.
pub fn f32_to_fp16(f32: &[f32]) -> Vec<u8> {
    f32.iter().flat_map(|v| {
        let bits = v.to_bits();
        let sign = (bits >> 31) & 0x8000;
        let exp = ((bits >> 23) & 0xFF) as u16;
        let frac = ((bits & 0x7FFFFF) >> 13) as u16;
        
        let biased_exp = if exp == 0 {
            0
        } else if exp >= 142 {
            31
        } else {
            ((exp - 127) + 15) as u16
        };
        
        let fp16 = sign | (biased_exp << 10) | frac;
        fp16.to_le_bytes()
    }).collect()
}

/// Convert FP16 back to f32.
pub fn fp16_to_f32(fp16: &[u8]) -> Vec<f32> {
    fp16.chunks_exact(2)
        .map(|chunk| {
            let val = u16::from_le_bytes(chunk.try_into().unwrap());
            let sign = (val & 0x8000) != 0;
            let biased_exp = ((val & 0x7C00) >> 10) as u32;
            let frac = (val & 0x3FF) as u32;
            
            let bits = if biased_exp == 0 {
                if frac == 0 {
                    0u32
                } else {
                    // Denormalized
                    (frac << 13) >> (1 - (31 - 15 - 10) as i32)
                }
            } else if biased_exp == 31 {
                // Infinity/NaN
                0x7F800000 | (frac << 13)
            } else {
                // Normalized
                ((biased_exp as u32 - 15 + 127) << 23) | (frac << 13)
            };
            
            f32::from_bits(bits)
        })
        .collect()
}
```

---

## 6. RVF Learning Store

### 6.1 Binary Format

```
File format (.rvls):
  4-byte magic "RVLS" + newline (0x0A)
  One JSON per line: {"type":"pattern"|"lora"|"ewc"|"trajectory","data":{...}}
  4-byte magic "REND" at end (optional)
```

### 6.2 Implementation

```rust
// memory/rvf_store.rs

use std::io::{BufRead, BufReader, Write, BufWriter};
use std::fs::{File, OpenOptions};
use std::path::Path;

const MAGIC_HEADER: &[u8] = b"RVLS\n";
const MAGIC_END: &[u8] = b"REND\n";

/// RVF Learning Store for pattern/LoRA/EWC persistence.
pub struct RvfLearningStore {
    store_path: PathBuf,
    patterns: RwLock<HashMap<String, PatternRecord>>,
    trajectories: RwLock<Vec<TrajectoryRecord>>,
    ewc_state: RwLock<Option<EwcState>>,
    dirty: AtomicBool,
}

impl RvfLearningStore {
    pub fn new(store_path: PathBuf) -> Self {
        Self {
            store_path,
            patterns: RwLock::new(HashMap::new()),
            trajectories: RwLock::new(Vec::new()),
            ewc_state: RwLock::new(None),
            dirty: AtomicBool::new(false),
        }
    }

    /// Initialize by loading from disk.
    pub async fn initialize(&self) -> Result<(), MemoryError> {
        if !self.store_path.exists() {
            return Ok(());
        }

        let file = File::open(&self.store_path)
            .map_err(|e| MemoryError::from(e))?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line.map_err(MemoryError::from)?;
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Ok(record) = serde_json::from_str::<StoreLine>(&line) {
                match record.r#type.as_str() {
                    "pattern" => {
                        if let Ok(pattern) = serde_json::from_value(record.data.clone()) {
                            self.patterns.write().insert(pattern.id.clone(), pattern);
                        }
                    }
                    "trajectory" => {
                        if let Ok(traj) = serde_json::from_value(record.data.clone()) {
                            self.trajectories.write().push(traj);
                        }
                    }
                    "ewc" => {
                        if let Ok(ewc) = serde_json::from_value(record.data.clone()) {
                            *self.ewc_state.write() = Some(ewc);
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// Persist to disk.
    pub async fn persist(&self) -> Result<(), MemoryError> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.store_path)
            .map_err(MemoryError::from)?;
        
        let mut writer = BufWriter::new(file);

        // Write header
        writer.write_all(MAGIC_HEADER)
            .map_err(MemoryError::from)?;

        // Write patterns
        for pattern in self.patterns.read().values() {
            let line = serde_json::to_string(&StoreLine {
                r#type: "pattern".to_string(),
                data: serde_json::to_value(pattern).map_err(MemoryError::from)?,
            }).map_err(MemoryError::from)?;
            writeln!(writer, "{}", line).map_err(MemoryError::from)?;
        }

        // Write trajectories
        for traj in self.trajectories.read().iter() {
            let line = serde_json::to_string(&StoreLine {
                r#type: "trajectory".to_string(),
                data: serde_json::to_value(traj).map_err(MemoryError::from)?,
            }).map_err(MemoryError::from)?;
            writeln!(writer, "{}", line).map_err(MemoryError::from)?;
        }

        // Write EWC state
        if let Some(ewc) = self.ewc_state.read().as_ref() {
            let line = serde_json::to_string(&StoreLine {
                r#type: "ewc".to_string(),
                data: serde_json::to_value(ewc).map_err(MemoryError::from)?,
            }).map_err(MemoryError::from)?;
            writeln!(writer, "{}", line).map_err(MemoryError::from)?;
        }

        // Write end marker
        writer.write_all(MAGIC_END)
            .map_err(MemoryError::from)?;

        writer.flush().map_err(MemoryError::from)?;
        self.dirty.store(false, Ordering::SeqCst);

        Ok(())
    }

    pub fn save_pattern(&self, pattern: PatternRecord) {
        self.patterns.write().insert(pattern.id.clone(), pattern);
        self.dirty.store(true, Ordering::SeqCst);
    }

    pub fn get_pattern(&self, id: &str) -> Option<PatternRecord> {
        self.patterns.read().get(id).cloned()
    }

    pub fn get_all_patterns(&self) -> Vec<PatternRecord> {
        self.patterns.read().values().cloned().collect()
    }
}

#[derive(Serialize, Deserialize)]
struct StoreLine {
    #[serde(rename = "type")]
    r#type: String,
    data: serde_json::Value,
}
```

---

## 7. MemoryManager Integration (Phase 1)

### 7.1 Updated MemoryManager

```rust
// memory/mod.rs - MemoryManager with Graph integration

pub struct MemoryManager {
    // Core subsystems
    engine: Arc<dyn EmbeddingEngine>,
    index: Arc<RwLock<HnswIndex>>,
    graph: Arc<MemoryGraph>,
    db: Arc<SqliteIndex>,
    store: Arc<StateStore>,
    
    // Learning (Phase 2)
    reasoning_bank: Option<Arc<RwLock<ReasoningBank>>>,
    learning_store: Option<Arc<RvfLearningStore>>,
    
    // Config
    config: MemoryConfig,
    max_recall: usize,
}

impl MemoryManager {
    pub async fn new_hnsw(
        store: Arc<StateStore>,
        config: &MemoryConfig,
        data_dir: &Path,
    ) -> Result<Self, MemoryError> {
        // 1. Create embedding engine
        let engine = create_engine(&config.embedding)?;
        
        // 2. Initialize index
        let dimensions = engine.dimensions();
        let index_path = data_dir.join("hnsw.usearch");
        let index = if index_path.exists() {
            let mut idx = HnswIndex::new(dimensions)?;
            idx.load(&index_path)?;
            idx
        } else {
            HnswIndex::with_params(dimensions, config.hnsw_m, config.hnsw_ef, config.hnsw_ef)?
        };
        
        // 3. Initialize graph
        let graph = Arc::new(MemoryGraph::new(MemoryGraphConfig::default()));
        
        // 4. Open SQLite
        let db_path = data_dir.join("memory.sqlite");
        let db = SqliteIndex::open(&db_path)?;
        
        Ok(Self {
            engine,
            index: Arc::new(RwLock::new(index)),
            graph,
            db: Arc::new(db),
            store,
            reasoning_bank: None,
            learning_store: None,
            config: config.clone(),
            max_recall: config.max_recall,
        })
    }

    /// Store entry with embedding and graph integration.
    pub async fn store(&self, input: MemoryEntryInput) -> Result<String, MemoryError> {
        // 1. Generate embedding
        let emb_result = self.engine.embed(&input.content).await?;
        let vector = emb_result.vector;
        
        // 2. Create entry
        let entry = MemoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            memory_type: input.memory_type,
            content: input.content,
            source: input.source,
            session_id: input.session_id,
            tags: input.tags,
            importance: input.importance,
            created_at: chrono::Utc::now(),
            accessed_at: chrono::Utc::now(),
            access_count: 0,
        };
        
        // 3. Store in SQLite
        self.db.insert(&entry, &vector)?;
        
        // 4. Add to HNSW index
        {
            let mut index = self.index.write();
            index.insert(&entry.id, vector.as_slice())?;
        }
        
        // 5. Add to graph
        self.graph.add_node(&entry);
        // Add edges for any references in content
        for ref_id in self.extract_references(&entry.content) {
            self.graph.add_reference_edge(&entry.id, &ref_id);
        }
        
        // 6. Persist to state store
        let category = entry.memory_type.category();
        self.store.save(&category, &entry.id, &entry).await?;
        
        Ok(entry.id)
    }

    /// Search with HNSW + Graph ranking.
    pub async fn semantic_search(
        &self,
        query: &str,
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<SearchResult>, MemoryError> {
        let start = std::time::Instant::now();
        
        // 1. Generate query embedding
        let emb_result = self.engine.embed(query).await?;
        let query_vector = emb_result.vector.as_slice();
        
        // 2. Search HNSW
        let hnsw_results = {
            let index = self.index.read();
            index.search(query_vector, limit * 2)?  // Fetch more for filtering
        };
        
        // 3. Filter by threshold
        let filtered: Vec<(String, f64)> = hnsw_results
            .into_iter()
            .filter(|(_, score)| *score >= threshold as f64)
            .take(limit)
            .collect();
        
        if filtered.is_empty() {
            return Ok(vec![]);
        }
        
        // 4. Graph ranking
        let ranked = self.graph.rank_results(filtered, 0.6, 0.3);
        
        // 5. Lookup metadata
        let ids: Vec<String> = ranked.iter().map(|r| r.id.clone()).collect();
        let entries = self.db.lookup(&ids)?;
        
        // 6. Build results
        let mut results = Vec::new();
        for (i, r) in ranked.iter().enumerate() {
            if let Some(entry) = entries.iter().find(|e| e.id == r.id) {
                results.push(SearchResult {
                    entry: entry.clone(),
                    score: r.combined_score as f32,
                    rank: i + 1,
                    page_rank: r.page_rank as f32,
                    latency_ms: None,
                });
            }
        }
        
        Ok(results)
    }

    /// Extract references from content.
    fn extract_references(&self, content: &str) -> Vec<String> {
        // Look for patterns like #entry-id or references to other entries
        let mut refs = Vec::new();
        
        for line in content.lines() {
            if line.starts_with('#') {
                let id = line.trim_start_matches('#').trim();
                if !id.is_empty() {
                    refs.push(id.to_string());
                }
            }
        }
        
        refs
    }
}

/// Search result with graph scores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub entry: MemoryEntry,
    pub score: f32,
    pub rank: usize,
    pub page_rank: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}
```

---

## 8. Web API Routes

### 8.1 Memory Routes

```rust
// oxios-web/src/routes/memory_routes.rs (new)

use axum::{
    extract::{Query, State},
    Json, Router, routing::{get, post, delete},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct SearchParams {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default = "default_threshold")]
    threshold: f32,
    #[serde(default = "default_graph_boost")]
    graph_boost: bool,
}

fn default_limit() -> usize { 10 }
fn default_threshold() -> f32 { 0.6 }
fn default_graph_boost() -> bool { true }

#[derive(Serialize)]
pub struct SearchResponse {
    query: String,
    results: Vec<SearchResult>,
    latency_ms: u64,
    graph_enabled: bool,
}

pub async fn handle_search(
    state: State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Json<SearchResponse> {
    let start = std::time::Instant::now();
    
    let results = state.kernel.memory()
        .semantic_search(&params.q, params.limit, params.threshold)
        .await
        .unwrap_or_default();
    
    let latency = start.elapsed().as_millis() as u64;
    
    Json(SearchResponse {
        query: params.q,
        results,
        latency_ms: latency,
        graph_enabled: params.graph_boost,
    })
}

#[derive(Serialize)]
pub struct GraphStats {
    node_count: usize,
    edge_count: usize,
    avg_degree: f64,
    community_count: usize,
}

pub async fn handle_graph_stats(
    state: State<Arc<AppState>>,
) -> Json<GraphStats> {
    let stats = state.kernel.memory().graph().stats();
    Json(GraphStats {
        node_count: stats.node_count,
        edge_count: stats.edge_count,
        avg_degree: stats.avg_degree,
        community_count: stats.community_count,
    })
}

pub fn memory_routes() -> Router {
    Router::new()
        .route("/api/memory/search", get(handle_search))
        .route("/api/memory/graph/stats", get(handle_graph_stats))
}
```

---

## 9. Implementation Phases

### Phase 1: Foundation (Week 1-2)
| Module | File | Tasks |
|--------|------|-------|
| HNSW Index | `memory/hnsw.rs` | usearch CRUD + persistence |
| SQL Store | `memory/store.rs` | SQLite hybrid storage |
| Embed Engine | `memory/engine.rs` | OnnxEngine + OpenAI |
| MemoryGraph | `memory/graph.rs` | PageRank + communities |
| Integration | `memory/mod.rs` | MemoryManager wired up |
| Web Routes | `routes/memory_routes.rs` | Search API |
| **Deliverable** | - | P50 <10ms for 1K entries |

### Phase 2: Learning (Week 3-4)
| Module | File | Tasks |
|--------|------|-------|
| ReasoningBank | `memory/reasoning_bank.rs` | Pattern store/search/route |
| RVF Store | `memory/rvf_store.rs` | RVF persistence |
| Workers | `workers/mod.rs` | WorkerManager |
| SONA | `memory/sona.rs` | Simplified SONA |
| Flash Attention | `memory/flash_attention.rs` | Block attention |
| **Deliverable** | - | Pattern routing |

### Phase 3: Polish (Week 5-6)
| Module | File | Tasks |
|--------|------|-------|
| Chunking | `memory/chunking.rs` | Document chunking |
| Normalizer | `memory/normalizer.rs` | L2/FP16/INT8 |
| Hyperbolic | `memory/hyperbolic.rs` | Poincaré ball |
| Migration | `memory/migrate.rs` | TF-IDF → HNSW |
| **Deliverable** | - | Production ready |

### Phase 4: Integration (Week 7-8)
| Module | File | Tasks |
|--------|------|-------|
| Kernel | `kernel.rs` | Memory subsystem |
| Ouroboros | `ouroboros/seed.rs` | Memory enrichment |
| Events | `event_bus.rs` | KernelEvent updates |
| **Deliverable** | - | Full integration |