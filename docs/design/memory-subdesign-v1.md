# Oxios HNSW Memory — 하위 세부설계

> **문서 계층**: 세부설계 (Sub-Design)
> **상위 문서**: `hnsw-memory-design.md`
> **상태**: 초안
> **날짜**: 2026-05-14

---

## 1. Crate Structure

### 1.1 Proposed Module Layout

```
crates/oxios-kernel/src/
├── embedding.rs          # 수정: DenseVector 추가, trait 확장
├── memory/
│   ├── mod.rs           # 수정: MemoryManager 확장
│   ├── store.rs         # 수정: SqliteIndex 통합
│   ├── hnsw.rs          # 신규: HNSW 인덱스 래퍼
│   ├── engine.rs        # 신규: EmbeddingEngine (ONNX WASM)
│   ├── quantize.rs      # 신규: INT8/FP16 양자화
│   ├── migrate.rs       # 신규: TF-IDF → HNSW 마이그레이션
│   └── error.rs         # 신규: 전용 에러 타입
```

### 1.2 Dependency Additions

```toml
# oxios-kernel/Cargo.toml

# usearch (HNSW library)
usearch = { version = "0.15", features = ["simd", "wasm"] }

# ONNX Runtime WASM (embedded models)
# Note: ort-wasm은 JavaScript integration이 필요하므로
# pure Rust alternative 사용:
#   - candle (Rust NLP, heavier)
#   - tract-onnx (lighter, better for embedding)
# Decision: tract-onnx
tract-onnx = { version = "0.26", features = ["onnx"] }

# Database
rusqlite = { version = "0.34", features = ["bundled"] }

# ML utilities
ndarray = "0.16"
num-traits = "0.2"

# WASM detection
cfg-if = "1"
```

### 1.3 Feature Flags

```toml
[features]
default = []
# ...existing flags...
hnsw = ["usearch", "tract-onnx", "rusqlite"]
onnx-cache = ["hnsw"]  # 모델 다운로드/캐싱
quantize = ["hnsw"]    # INT8/FP16 양자화
```

---

## 2. Error Types

### 2.1 Error Hierarchy

```rust
// memory/error.rs

#[derive(thiserror::Error, Debug)]
pub enum MemoryError {
    #[error("embedding generation failed: {0}")]
    EmbeddingFailed(String),
    
    #[error("embedding engine not ready: {0}")]
    EngineNotReady(String),
    
    #[error("HNSW index error: {0}")]
    IndexError(String),
    
    #[error("index not found for id: {0}")]
    IndexNotFound(String),
    
    #[error("database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),
    
    #[error("serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("quantization error: {0}")]
    QuantizationError(String),
    
    #[error("migration failed: {0}")]
    MigrationFailed(String),
    
    #[error("invalid vector dimensions: expected {expected}, got {actual}")]
    InvalidDimensions { expected: usize, actual: usize },
    
    #[error("search threshold {threshold} too low (min {min})")]
    ThresholdTooLow { threshold: f32, min: f32 },
    
    #[error("model not found: {0}")]
    ModelNotFound(String),
}

pub type Result<T> = std::result::Result<T, MemoryError>;
```

---

## 3. Core Data Structures

### 3.1 DenseVector

```rust
// embedding.rs - 수정

/// Dense embedding vector (384~1536 dimensions).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenseVector {
    /// Raw f32 values.
    values: Vec<f32>,
    /// Dimension count.
    dimensions: usize,
    /// Optional: L2 norm for cosine similarity.
    #[serde(skip)]
    norm: Option<f32>,
}

impl DenseVector {
    pub fn new(values: Vec<f32>) -> Result<Self, MemoryError> {
        let dimensions = values.len();
        if dimensions == 0 {
            return Err(MemoryError::InvalidDimensions {
                expected: 384,
                actual: 0,
            });
        }
        
        let norm = Self::compute_norm(&values);
        Ok(Self { values, dimensions, norm: Some(norm) })
    }
    
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }
    
    pub fn as_slice(&self) -> &[f32] {
        &self.values
    }
    
    /// L2 norm.
    fn compute_norm(values: &[f32]) -> f32 {
        values.iter().map(|v| v * v).sum::<f32>().sqrt()
    }
    
    /// Cosine similarity.
    pub fn cosine_similarity(&self, other: &DenseVector) -> f32 {
        if self.dimensions != other.dimensions {
            return 0.0;
        }
        
        let dot: f32 = self.values.iter()
            .zip(other.values.iter())
            .map(|(a, b)| a * b)
            .sum();
        
        let norm_a = self.norm.unwrap_or_else(|| Self::compute_norm(&self.values));
        let norm_b = other.norm.unwrap_or_else(|| Self::compute_norm(&other.values));
        
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        
        dot / (norm_a * norm_b)
    }
    
    /// Convert to quantized INT8 for storage.
    pub fn to_int8(&self) -> Vec<i8> {
        let norm = self.norm.unwrap_or_else(|| Self::compute_norm(&self.values));
        if norm == 0.0 {
            return vec![0; self.dimensions];
        }
        
        self.values.iter()
            .map(|v| {
                let normalized = v / norm;
                // Scale to INT8 range [-127, 127]
                (normalized * 127.0).round() as i8
            })
            .collect()
    }
    
    /// Convert from INT8 quantization.
    pub fn from_int8(quantized: &[i8], norm: f32) -> Self {
        let values: Vec<f32> = quantized.iter()
            .map(|v| (*v as f32) / 127.0 * norm)
            .collect();
        
        Self {
            values,
            dimensions: values.len(),
            norm: Some(norm),
        }
    }
}

/// Unified embedding vector.
#[derive(Debug, Clone)]
pub enum EmbeddingVector {
    /// Sparse TF-IDF vector (legacy, for migration).
    Sparse(HashMap<String, f64>),
    /// Dense neural embedding (384~1536 dims).
    Dense(DenseVector),
}

impl EmbeddingVector {
    pub fn dimensions(&self) -> usize {
        match self {
            Self::Sparse(m) => m.len(),
            Self::Dense(v) => v.dimensions(),
        }
    }
}
```

### 3.2 SearchResult

```rust
// memory/mod.rs - 수정

/// Result of a memory search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Matching entry.
    pub entry: MemoryEntry,
    /// Similarity score (0.0 - 1.0).
    pub score: f32,
    /// Rank in results.
    pub rank: usize,
    /// Search latency in ms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

impl SearchResult {
    pub fn new(entry: MemoryEntry, score: f32, rank: usize) -> Self {
        Self {
            entry,
            score,
            rank,
            latency_ms: None,
        }
    }
    
    pub fn with_latency(mut self, ms: u64) -> Self {
        self.latency_ms = Some(ms);
        self
    }
}

/// Input for creating a memory entry.
#[derive(Debug, Clone)]
pub struct MemoryEntryInput {
    pub memory_type: MemoryType,
    pub content: String,
    pub source: String,
    pub session_id: Option<String>,
    pub tags: Vec<String>,
    pub importance: f32,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}
```

### 3.3 HnswIndex

```rust
// memory/hnsw.rs

use usearch::{Index, MetricKind, QuantizationType, ScalarKind};
use std::path::Path;
use parking_lot::RwLock;

pub struct HnswIndex {
    index: RwLock<Index>,
    dimensions: usize,
    path: Option<PathBuf>,
}

impl HnswIndex {
    /// Create new HNSW index with default parameters.
    pub fn new(dimensions: usize) -> Result<Self, MemoryError> {
        let index = Index::new(
            MetricKind::Cosine,     // Cosine similarity
            dimensions,
            ScalarKind::F16,       // FP16 quantization (half precision)
            Default::default(),
        ).map_err(|e| MemoryError::IndexError(e.to_string()))?;
        
        Ok(Self {
            index: RwLock::new(index),
            dimensions,
            path: None,
        })
    }
    
    /// Create with custom parameters.
    pub fn with_params(
        dimensions: usize,
        m: usize,              // connections per node (default 16)
        ef_construction: usize, // construction search width (default 128)
        ef_search: usize,       // search width (default 128)
    ) -> Result<Self, MemoryError> {
        let mut config = usearch::IndexOptions::default();
        config.m = m;
        config.ef_construction = ef_construction;
        config.ef_search = ef_search;
        
        let index = Index::new(
            MetricKind::Cosine,
            dimensions,
            ScalarKind::F16,
            config,
        ).map_err(|e| MemoryError::IndexError(e.to_string()))?;
        
        Ok(Self {
            index: RwLock::new(index),
            dimensions,
            path: None,
        })
    }
    
    /// Add vector to index.
    pub fn insert(&self, id: &str, vector: &[f32]) -> Result<(), MemoryError> {
        if vector.len() != self.dimensions {
            return Err(MemoryError::InvalidDimensions {
                expected: self.dimensions,
                actual: vector.len(),
            });
        }
        
        let key = usearch::Key::from(id);
        self.index.write().add(key, vector)
            .map_err(|e| MemoryError::IndexError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Remove vector from index.
    pub fn remove(&self, id: &str) -> Result<(), MemoryError> {
        let key = usearch::Key::from(id);
        self.index.write().remove(key)
            .map_err(|e| MemoryError::IndexError(e.to_string()))?;
        Ok(())
    }
    
    /// Search ANN.
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>, MemoryError> {
        if query.len() != self.dimensions {
            return Err(MemoryError::InvalidDimensions {
                expected: self.dimensions,
                actual: query.len(),
            });
        }
        
        let results = self.index.read().search(query, k);
        
        Ok(results.map(|r| {
            let id = String::from(r.key.as_str());
            let score = 1.0 - r.distance; // usearch returns distance, convert to similarity
            (id, score)
        }).collect())
    }
    
    /// Save index to disk.
    pub fn save(&self, path: &Path) -> Result<(), MemoryError> {
        let index = self.index.read();
        index.save_to_path(path, usearch::SaveOptions::default())
            .map_err(|e| MemoryError::IndexError(e.to_string()))?;
        Ok(())
    }
    
    /// Load index from disk.
    pub fn load(&mut self, path: &Path) -> Result<(), MemoryError> {
        let loaded = Index::load_from_path(path, usearch::SaveOptions::default())
            .map_err(|e| MemoryError::IndexError(e.to_string()))?;
        self.index = RwLock::new(loaded);
        self.path = Some(path.to_path_buf());
        Ok(())
    }
    
    /// Get index size.
    pub fn size(&self) -> usize {
        self.index.read().size()
    }
    
    /// Get dimensions.
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hnsw_insert_and_search() {
        let index = HnswIndex::new(4).unwrap();
        
        // Insert 3 vectors
        index.insert("a", &[1.0, 0.0, 0.0, 0.0]).unwrap();
        index.insert("b", &[0.0, 1.0, 0.0, 0.0]).unwrap();
        index.insert("c", &[0.9, 0.1, 0.0, 0.0]).unwrap();
        
        // Search for similar to 'a'
        let results = index.search(&[1.0, 0.0, 0.0, 0.0], 3).unwrap();
        
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, "a"); // self-match should be highest
        assert!(results[0].1 > results[1].1); // ordered by score
    }
}
```

---

## 4. Embedding Engine

### 4.1 Trait Definition

```rust
// memory/engine.rs

/// Embedding generation engine.
#[async_trait::async_trait]
pub trait EmbeddingEngine: Send + Sync {
    /// Generate embedding for single text.
    async fn embed(&self, text: &str) -> Result<DenseVector, MemoryError>;
    
    /// Generate embeddings for batch of texts.
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<DenseVector>, MemoryError>;
    
    /// Engine identifier.
    fn name(&self) -> &str;
    
    /// Check engine readiness.
    async fn health_check(&self) -> Result<(), MemoryError>;
    
    /// Get embedding dimensions.
    fn dimensions(&self) -> usize;
}
```

### 4.2 ONNX Engine Implementation

```rust
// memory/engine.rs - ONNX Engine

use tract_onnx::prelude::{tract_helpers::tract_ndarray::Array2, *};

/// ONNX-based embedding engine (all-MiniLM-L6-v2).
pub struct OnnxEngine {
    model: TractRunnableModel,
    session: Session,
    dimensions: usize,
    batch_size: usize,
}

impl OnnxEngine {
    /// Load model from embedded bytes or file.
    pub fn load(model_path: &Path) -> Result<Self, MemoryError> {
        let model = tract_onnx::onnx()
            .model_for_path(model_path)
            .map_err(|e| MemoryError::ModelNotFound(e.to_string()))?;
        
        let dimensions = Self::infer_dimensions(&model)?;
        
        Ok(Self {
            model,
            session: Session::new(&model).map_err(|e| MemoryError::EmbeddingFailed(e.to_string()))?,
            dimensions,
            batch_size: 32,
        })
    }
    
    /// Load model from embedded bytes (compiled into binary).
    #[cfg(feature = "onnx-cache")]
    pub fn load_embedded(model_bytes: &[u8]) -> Result<Self, MemoryError> {
        let model = tract_onnx::onnx()
            .model_for_read(&mut Cursor::new(model_bytes))
            .map_err(|e| MemoryError::ModelNotFound(e.to_string()))?;
        
        let dimensions = Self::infer_dimensions(&model)?;
        
        Ok(Self {
            model,
            session: Session::new(&model).map_err(|e| MemoryError::EmbeddingFailed(e.to_string()))?,
            dimensions,
            batch_size: 32,
        })
    }
    
    fn infer_dimensions(model: &TractModel) -> Result<usize, MemoryError> {
        // all-MiniLM-L6-v2 outputs 384-dim vectors
        // Infer from model output shape
        todo!("Implement shape inference")
    }
}

#[async_trait::async_trait]
impl EmbeddingEngine for OnnxEngine {
    async fn embed(&self, text: &str) -> Result<DenseVector, MemoryError> {
        let results = self.embed_batch(&[text]).await?;
        results.into_iter().next()
            .ok_or_else(|| MemoryError::EmbeddingFailed("Empty batch".into()))
    }
    
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<DenseVector>, MemoryError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        
        // Tokenize (simplified - real implementation uses tokenizer)
        let input_ids = self.tokenize_batch(texts);
        
        // Run inference
        let output = self.run_inference(&input_ids)?;
        
        // Extract embeddings (mean pooling)
        let embeddings = self.mean_pool(output);
        
        Ok(embeddings)
    }
    
    fn name(&self) -> &str {
        "onnx-mini-lm-l6-v2"
    }
    
    async fn health_check(&self) -> Result<(), MemoryError> {
        // Verify model is loaded
        Ok(())
    }
    
    fn dimensions(&self) -> usize {
        self.dimensions
    }
    
    fn tokenize_batch(&self, texts: &[&str]) -> Array2<i64> {
        // Simplified tokenization
        // Real implementation: use tokenizers crate or HuggingFace tokenizer
        let max_len = 128;
        let batch_size = texts.len();
        
        let mut ids = Array2::<i64>::zeros((batch_size, max_len));
        
        for (batch_idx, text) in texts.iter().enumerate() {
            let tokens: Vec<i64> = text
                .chars()
                .take(max_len - 2) // [CLS] ... [SEP]
                .map(|c| c as i64 % 50257) // Simplified mapping
                .collect();
            
            ids[[batch_idx, 0]] = 101; // [CLS]
            for (i, token) in tokens.iter().enumerate() {
                ids[[batch_idx, i + 1]] = *token;
            }
            ids[[batch_idx, tokens.len() + 1]] = 102; // [SEP]
        }
        
        ids
    }
    
    fn run_inference(&self, input: &Array2<i64>) -> Result<Array2<f32>, MemoryError> {
        // Run ONNX inference
        // Returns (batch_size, seq_len, hidden_dim) tensor
        todo!("Implement ONNX inference")
    }
    
    fn mean_pool(&self, tensor: Array2<f32>) -> Vec<DenseVector> {
        // Mean pool across sequence dimension
        // Returns (batch_size, hidden_dim) vectors
        todo!("Implement mean pooling")
    }
}
```

### 4.3 OpenAI Fallback Engine

```rust
// memory/engine.rs - OpenAI Engine

use reqwest::Client;

/// OpenAI text-embedding-3-small fallback engine.
pub struct OpenAiEngine {
    client: Client,
    api_key: String,
    dimensions: usize,
    base_url: String,
}

impl OpenAiEngine {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            dimensions: 1536, // text-embedding-3-small default
            base_url: "https://api.openai.com/v1".into(),
        }
    }
    
    pub fn with_dimensions(mut self, dimensions: usize) -> Self {
        self.dimensions = dimensions;
        self
    }
}

#[async_trait::async_trait]
impl EmbeddingEngine for OpenAiEngine {
    async fn embed(&self, text: &str) -> Result<DenseVector, MemoryError> {
        let results = self.embed_batch(&[text]).await?;
        results.into_iter().next()
            .ok_or_else(|| MemoryError::EmbeddingFailed("Empty response".into()))
    }
    
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<DenseVector>, MemoryError> {
        let response = self.client
            .post(format!("{}/embeddings", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "model": "text-embedding-3-small",
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
        
        result.data.into_iter()
            .map(|d| DenseVector::new(d.embedding))
            .collect()
    }
    
    fn name(&self) -> &str {
        "openai-embedding-3-small"
    }
    
    async fn health_check(&self) -> Result<(), MemoryError> {
        // Simple API check
        Ok(())
    }
    
    fn dimensions(&self) -> usize {
        self.dimensions
    }
}
```

### 4.4 Engine Factory

```rust
// memory/engine.rs - Factory

/// Create embedding engine based on configuration.
pub fn create_engine(config: &EmbeddingConfig) -> Result<Arc<dyn EmbeddingEngine>, MemoryError> {
    match config {
        EmbeddingConfig::Onnx { model_path } => {
            let engine = OnnxEngine::load(model_path)?;
            Ok(Arc::new(engine))
        }
        EmbeddingConfig::OpenAi { api_key } => {
            let engine = OpenAiEngine::new(api_key.clone());
            Ok(Arc::new(engine))
        }
        EmbeddingConfig::Hybrid { primary, fallback } => {
            // Hybrid: try primary, fallback on failure
            Ok(Arc::new(HybridEngine {
                primary: create_engine(primary)?,
                fallback: create_engine(fallback)?,
            }))
        }
    }
}

/// Configuration for embedding engine.
pub enum EmbeddingConfig {
    /// Local ONNX model.
    Onnx { model_path: PathBuf },
    /// OpenAI API.
    OpenAi { api_key: String },
    /// Local + OpenAI fallback.
    Hybrid {
        primary: Box<EmbeddingConfig>,
        fallback: Box<EmbeddingConfig>,
    },
}

/// Hybrid engine with fallback.
struct HybridEngine {
    primary: Arc<dyn EmbeddingEngine>,
    fallback: Arc<dyn EmbeddingEngine>,
}

#[async_trait::async_trait]
impl EmbeddingEngine for HybridEngine {
    async fn embed(&self, text: &str) -> Result<DenseVector, MemoryError> {
        self.primary.embed(text).await
            .or_else(|_| self.fallback.embed(text).await)
    }
    
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<DenseVector>, MemoryError> {
        self.primary.embed_batch(texts).await
            .or_else(|_| self.fallback.embed_batch(texts).await)
    }
    
    fn name(&self) -> &str {
        "hybrid"
    }
    
    async fn health_check(&self) -> Result<(), MemoryError> {
        // Check primary first
        self.primary.health_check().await
            .or_else(|_| self.fallback.health_check().await)
    }
    
    fn dimensions(&self) -> usize {
        self.primary.dimensions()
    }
}
```

---

## 5. SQLite Storage

### 5.1 Schema

```rust
// memory/store.rs - SqliteIndex

use rusqlite::{Connection, params};

pub struct SqliteIndex {
    conn: Connection,
    path: PathBuf,
}

impl SqliteIndex {
    /// Open or create database.
    pub fn open(path: &Path) -> Result<Self, MemoryError> {
        let conn = Connection::open(path)
            .map_err(MemoryError::from)?;
        
        let store = Self {
            conn,
            path: path.to_path_buf(),
        };
        
        store.init_schema()?;
        Ok(store)
    }
    
    fn init_schema(&self) -> Result<(), MemoryError> {
        self.conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS entries (
                id          TEXT PRIMARY KEY,
                memory_type TEXT NOT NULL,
                content     TEXT NOT NULL,
                source      TEXT NOT NULL,
                session_id  TEXT,
                tags        TEXT,
                importance  REAL DEFAULT 0.5,
                created_at  TEXT NOT NULL,
                accessed_at TEXT NOT NULL,
                access_count INTEGER DEFAULT 0,
                vector_id   INTEGER,
                metadata    TEXT
            );
            
            CREATE INDEX IF NOT EXISTS idx_memory_type ON entries(memory_type);
            CREATE INDEX IF NOT EXISTS idx_session_id ON entries(session_id);
            CREATE INDEX IF NOT EXISTS idx_created_at ON entries(created_at);
            
            CREATE TABLE IF NOT EXISTS embeddings (
                entry_id    TEXT PRIMARY KEY REFERENCES entries(id) ON DELETE CASCADE,
                vector_fp16 BLOB NOT NULL,
                vector_f32  BLOB NOT NULL,
                dimensions  INTEGER NOT NULL,
                norm        REAL NOT NULL
            );
            
            CREATE TABLE IF NOT EXISTS search_log (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                query       TEXT NOT NULL,
                result_count INTEGER NOT NULL,
                top_score   REAL,
                latency_ms  INTEGER,
                created_at  TEXT DEFAULT CURRENT_TIMESTAMP
            );
        "#).map_err(MemoryError::from)?;
        
        Ok(())
    }
    
    /// Insert memory entry.
    pub fn insert(&self, entry: &MemoryEntry, vector: &DenseVector) -> Result<(), MemoryError> {
        let tx = self.conn.transaction()
            .map_err(MemoryError::from)?;
        
        tx.execute(
            r#"INSERT INTO entries (
                id, memory_type, content, source, session_id,
                tags, importance, created_at, accessed_at,
                access_count, metadata
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
            params![
                entry.id,
                serde_json::to_string(&entry.memory_type)?,
                entry.content,
                entry.source,
                entry.session_id,
                serde_json::to_string(&entry.tags)?,
                entry.importance,
                entry.created_at.to_rfc3339(),
                entry.accessed_at.to_rfc3339(),
                entry.access_count,
                entry.metadata.as_ref().map(|m| serde_json::to_string(m)).transpose()?,
            ],
        ).map_err(MemoryError::from)?;
        
        // Store embedding (both FP16 and F32 for accuracy + speed)
        let vector_f32 = vector.as_slice();
        let vector_fp16 = Self::f32_to_fp16(vector_f32);
        
        tx.execute(
            r#"INSERT INTO embeddings (entry_id, vector_fp16, vector_f32, dimensions, norm)
               VALUES (?1, ?2, ?3, ?4, ?5)"#,
            params![
                entry.id,
                vector_fp16,
                vector_f32,
                vector.dimensions(),
                vector.norm.unwrap_or_else(|| DenseVector::compute_norm(vector_f32)),
            ],
        ).map_err(MemoryError::from)?;
        
        tx.commit().map_err(MemoryError::from)?;
        Ok(())
    }
    
    /// Lookup entries by IDs.
    pub fn lookup(&self, ids: &[String]) -> Result<Vec<MemoryEntry>, MemoryError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        
        let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
        let query = format!(
            r#"SELECT id, memory_type, content, source, session_id,
                      tags, importance, created_at, accessed_at,
                      access_count, metadata
               FROM entries
               WHERE id IN ({})"#,
            placeholders.join(",")
        );
        
        let mut stmt = self.conn.prepare(&query)
            .map_err(MemoryError::from)?;
        
        let entries = ids.iter()
            .map(|id| {
                stmt.query_row(params![id], |row| {
                    Ok(MemoryEntry {
                        id: row.get(0)?,
                        memory_type: serde_json::from_str(&row.get::<_, String>(1)?).unwrap(),
                        content: row.get(2)?,
                        source: row.get(3)?,
                        session_id: row.get(4)?,
                        tags: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
                        importance: row.get(6)?,
                        created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_else(|_| chrono::Utc::now()),
                        accessed_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_else(|_| chrono::Utc::now()),
                        access_count: row.get(9)?,
                        metadata: row.get::<_, Option<String>>(10)?
                            .and_then(|m| serde_json::from_str(&m).ok()),
                    })
                })
            })
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(MemoryError::from)?;
        
        Ok(entries)
    }
    
    /// Update access stats.
    pub fn touch(&self, id: &str) -> Result<(), MemoryError> {
        self.conn.execute(
            r#"UPDATE entries SET
                accessed_at = ?1,
                access_count = access_count + 1
               WHERE id = ?2"#,
            params![chrono::Utc::now().to_rfc3339(), id],
        ).map_err(MemoryError::from)?;
        Ok(())
    }
    
    /// Delete entry.
    pub fn delete(&self, id: &str) -> Result<(), MemoryError> {
        self.conn.execute("DELETE FROM entries WHERE id = ?1", params![id])
            .map_err(MemoryError::from)?;
        Ok(())
    }
    
    /// Log search query for analytics.
    pub fn log_search(&self, query: &str, result_count: usize, top_score: f32, latency_ms: u64)
        -> Result<(), MemoryError>
    {
        self.conn.execute(
            r#"INSERT INTO search_log (query, result_count, top_score, latency_ms)
               VALUES (?1, ?2, ?3, ?4)"#,
            params![query, result_count, top_score, latency_ms],
        ).map_err(MemoryError::from)?;
        Ok(())
    }
    
    /// Get all entries for re-indexing.
    pub fn get_all_entries(&self) -> Result<Vec<MemoryEntry>, MemoryError> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, memory_type, content, source, session_id,
                      tags, importance, created_at, accessed_at,
                      access_count, metadata
               FROM entries ORDER BY created_at DESC"#
        ).map_err(MemoryError::from)?;
        
        let entries = stmt.query_map([], |row| {
            Ok(MemoryEntry {
                id: row.get(0)?,
                memory_type: serde_json::from_str(&row.get::<_, String>(1)?).unwrap(),
                content: row.get(2)?,
                source: row.get(3)?,
                session_id: row.get(4)?,
                tags: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
                importance: row.get(6)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                accessed_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                access_count: row.get(9)?,
                metadata: row.get::<_, Option<String>>(10)?
                    .and_then(|m| serde_json::from_str(&m).ok()),
            })
        }).map_err(MemoryError::from)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(MemoryError::from)?;
        
        Ok(entries)
    }
    
    /// Get all vectors for re-indexing.
    pub fn get_all_vectors(&self) -> Result<Vec<(String, Vec<f32>)>, MemoryError> {
        let mut stmt = self.conn.prepare(
            "SELECT entry_id, vector_f32 FROM embeddings"
        ).map_err(MemoryError::from)?;
        
        let vectors = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            let f32_vec = Self::blob_to_f32(&blob);
            Ok((id, f32_vec))
        }).map_err(MemoryError::from)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(MemoryError::from)?;
        
        Ok(vectors)
    }
    
    // === Utility ===
    
    fn f32_to_fp16(f32: &[f32]) -> Vec<u8> {
        // Simple FP16 conversion
        f32.iter().flat_map(|v| {
            let bits = v.to_bits();
            let sign = (bits >> 31) & 0x8000;
            let exp = ((bits >> 23) & 0xFF) as u16;
            let frac = ((bits & 0x7FFFFF) >> 13) as u16;
            
            // Bias adjustment: 127 (f32) -> 15 (fp16 bias)
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
    
    fn blob_to_f32(blob: &[u8]) -> Vec<f32> {
        blob.chunks_exact(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
            .collect()
    }
}
```

---

## 6. MemoryManager (Orchestrator) — Updated

### 6.1 Complete Implementation

```rust
// memory/mod.rs - MemoryManager update

pub struct MemoryManager {
    // Core subsystems
    engine: Arc<dyn EmbeddingEngine>,
    index: Arc<RwLock<HnswIndex>>,
    db: Arc<SqliteIndex>,
    state_store: Arc<StateStore>,
    
    // Optional subsystems
    git_layer: Option<Arc<GitLayer>>,
    legacy_tfidf: Option<Arc<TfIdfEmbeddingProvider>>,  // For fallback during migration
    
    // Configuration
    config: MemoryConfig,
    dimensions: usize,
    max_recall: usize,
}

impl MemoryManager {
    /// Create new HNSW-based memory manager.
    pub async fn new_hnsw(
        state_store: Arc<StateStore>,
        config: &MemoryConfig,
        data_dir: &Path,
    ) -> Result<Self, MemoryError> {
        // 1. Create embedding engine
        let engine = create_engine(&config.embedding)?;
        
        // 2. Initialize dimensions
        let dimensions = engine.dimensions();
        
        // 3. Open/create HNSW index
        let index_path = data_dir.join("hnsw.bin");
        let index = if index_path.exists() {
            let mut idx = HnswIndex::new(dimensions)?;
            idx.load(&index_path)?;
            idx
        } else {
            HnswIndex::with_params(
                dimensions,
                config.hnsw_m,
                config.hnsw_ef_construction,
                config.hnsw_ef_search,
            )?
        };
        
        // 4. Open SQLite storage
        let db_path = data_dir.join("memory.sqlite");
        let db = SqliteIndex::open(&db_path)?;
        
        Ok(Self {
            engine: Arc::new(engine),
            index: Arc::new(RwLock::new(index)),
            db: Arc::new(db),
            state_store,
            git_layer: None,
            legacy_tfidf: None,
            config: config.clone(),
            dimensions,
            max_recall: config.max_recall,
        })
    }
    
    /// Store entry with automatic embedding.
    pub async fn store(&self, input: MemoryEntryInput) -> Result<String, MemoryError> {
        let start = std::time::Instant::now();
        
        // 1. Generate embedding
        let vector = self.engine.embed(&input.content).await?;
        
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
        
        // 3. Store in SQLite (metadata + vector)
        self.db.insert(&entry, &vector)?;
        
        // 4. Add to HNSW index
        {
            let mut index = self.index.write();
            index.insert(&entry.id, vector.as_slice())?;
            if let Some(ref path) = self.config.hnsw_path {
                index.save(path)?;
            }
        }
        
        // 5. Persist to state store
        let category = entry.memory_type.category();
        self.state_store.save(&category, &entry.id, &entry).await?;
        self.git_commit(&format!("{}/{}.json", category, entry.id), "Memory stored");
        
        let latency = start.elapsed().as_millis() as u64;
        
        // 6. Publish event
        if let Some(ref infra) = self.config.event_bus {
            infra.publish(KernelEvent::MemoryStored {
                id: entry.id.clone(),
                memory_type: entry.memory_type,
                source: entry.source.clone(),
            });
            infra.publish(KernelEvent::EmbeddingGenerated {
                engine: self.engine.name().to_string(),
                latency_ms: latency,
            });
        }
        
        Ok(entry.id)
    }
    
    /// Semantic search.
    pub async fn semantic_search(
        &self,
        query: &str,
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<SearchResult>, MemoryError> {
        let start = std::time::Instant::now();
        
        // 1. Generate query embedding
        let query_vector = self.engine.embed(query).await?;
        
        // 2. Search HNSW index
        let hnsw_results = {
            let index = self.index.read();
            index.search(query_vector.as_slice(), limit)?
        };
        
        // 3. Filter by threshold and collect IDs
        let filtered: Vec<(String, f32)> = hnsw_results
            .into_iter()
            .filter(|(_, score)| score >= threshold)
            .take(limit)
            .collect();
        
        if filtered.is_empty() {
            return Ok(vec![]);
        }
        
        let ids: Vec<String> = filtered.iter().map(|(id, _)| id.clone()).collect();
        
        // 4. Lookup metadata from SQLite
        let entries = self.db.lookup(&ids)?;
        
        // 5. Build ranked results
        let mut results: Vec<SearchResult> = filtered
            .into_iter()
            .zip(entries.into_iter())
            .map(|((id, score), entry)| {
                // Update access stats (async, non-blocking)
                let db = self.db.clone();
                tokio::spawn(async move {
                    let _ = db.touch(&id);
                });
                
                SearchResult::new(entry, score, 0)
            })
            .collect();
        
        // 6. Sort by score (descending)
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        
        // 7. Add rank
        for (i, result) in results.iter_mut().enumerate() {
            result.rank = i + 1;
        }
        
        // 8. Log search
        let latency = start.elapsed().as_millis() as u64;
        let top_score = results.first().map(|r| r.score).unwrap_or(0.0);
        self.db.log_search(query, results.len(), top_score, latency)?;
        
        // 9. Publish event
        if let Some(ref infra) = self.config.event_bus {
            infra.publish(KernelEvent::MemoryRecalled {
                query: query.to_string(),
                count: results.len(),
                top_score,
            });
        }
        
        Ok(results)
    }
    
    /// Delete entry.
    pub async fn forget(&self, id: &str, memory_type: MemoryType) -> Result<(), MemoryError> {
        // 1. Remove from HNSW index
        {
            let mut index = self.index.write();
            index.remove(id)?;
            if let Some(ref path) = self.config.hnsw_path {
                index.save(path)?;
            }
        }
        
        // 2. Delete from SQLite
        self.db.delete(id)?;
        
        // 3. Delete from state store
        let category = memory_type.category();
        self.state_store.delete(&category, id).await?;
        
        // 4. Git commit
        self.git_commit(&format!("{}/{}.json", category, id), "Memory deleted");
        
        Ok(())
    }
}
```

---

## 7. Migration

### 7.1 TF-IDF → HNSW Migration

```rust
// memory/migrate.rs

pub struct MigrationProgress {
    pub total: usize,
    pub migrated: usize,
    pub failed: usize,
    pub errors: Vec<(String, String)>,
}

impl MigrationProgress {
    pub fn finished(&self) -> bool {
        self.migrated + self.failed >= self.total
    }
    
    pub fn percent(&self) -> f32 {
        if self.total == 0 { 100.0 } else {
            (self.migrated + self.failed) as f32 / self.total as f32 * 100.0
        }
    }
}

/// Migrate from legacy TF-IDF to HNSW.
pub async fn migrate_from_tfidf(
    legacy_entries: Vec<MemoryEntry>,
    engine: &dyn EmbeddingEngine,
    index: &HnswIndex,
    db: &SqliteIndex,
    progress_callback: impl Fn(MigrationProgress),
) -> Result<MigrationProgress, MemoryError> {
    let total = legacy_entries.len();
    let mut progress = MigrationProgress {
        total,
        migrated: 0,
        failed: 0,
        errors: vec![],
    };
    
    // Batch processing for efficiency
    let batch_size = 10;
    
    for chunk in legacy_entries.chunks(batch_size) {
        let contents: Vec<&str> = chunk.iter().map(|e| e.content.as_str()).collect();
        
        match engine.embed_batch(&contents).await {
            Ok(vectors) => {
                for (entry, vector) in chunk.iter().zip(vectors.into_iter()) {
                    match db.insert(entry, &vector) {
                        Ok(()) => {
                            if let Err(e) = index.insert(&entry.id, vector.as_slice()) {
                                progress.errors.push((entry.id.clone(), e.to_string()));
                                progress.failed += 1;
                            } else {
                                progress.migrated += 1;
                            }
                        }
                        Err(e) => {
                            progress.errors.push((entry.id.clone(), e.to_string()));
                            progress.failed += 1;
                        }
                    }
                }
            }
            Err(e) => {
                for entry in chunk {
                    progress.errors.push((entry.id.clone(), e.to_string()));
                    progress.failed += 1;
                }
            }
        }
        
        progress_callback(progress.clone());
        
        // Small delay to avoid overwhelming the system
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    
    Ok(progress)
}
```

---

## 8. Configuration

### 8.1 Config Struct

```rust
// config.rs - MemoryConfig addition

#[derive(Debug, Clone)]
pub struct MemoryConfig {
    /// Max memories returned by recall.
    pub max_recall: usize,
    
    /// Embedding configuration.
    pub embedding: EmbeddingConfig,
    
    /// HNSW parameters.
    pub hnsw_m: usize,
    pub hnsw_ef_construction: usize,
    pub hnsw_ef_search: usize,
    
    /// HNSW index path.
    pub hnsw_path: Option<PathBuf>,
    
    /// SQLite path.
    pub sqlite_path: Option<PathBuf>,
    
    /// Event bus (optional).
    pub event_bus: Option<Arc<EventBus>>,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_recall: 10,
            embedding: EmbeddingConfig::Onnx {
                model_path: PathBuf::from("models/all-MiniLM-L6-v2.onnx"),
            },
            hnsw_m: 16,
            hnsw_ef_construction: 128,
            hnsw_ef_search: 128,
            hnsw_path: None,
            sqlite_path: None,
            event_bus: None,
        }
    }
}

/// TOML config section.
/// 
/// ```toml
/// [memory]
/// max_recall = 10
/// hnsw_m = 16
/// hnsw_ef_construction = 128
/// hnsw_ef_search = 128
/// 
/// [memory.embedding]
/// type = "onnx"
/// model_path = "./models/mini-lm.onnx"
/// # OR
/// # type = "openai"
/// # api_key = "${OPENAI_API_KEY}"
/// # dimensions = 1536
/// ```
```

---

## 9. API Endpoints (Web)

### 9.1 New Endpoints

```rust
// oxios-web/src/routes/memory_routes.rs (new file)

use axum::{extract::Query, Json, routing::{get, post}, Router};
use serde::{Deserialize, Serialize};

/// Search query parameters.
#[derive(Deserialize)]
pub struct SearchParams {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default = "default_threshold")]
    threshold: f32,
}

fn default_limit() -> usize { 10 }
fn default_threshold() -> f32 { 0.6 }

/// Search response.
#[derive(Serialize)]
pub struct SearchResponse {
    query: String,
    results: Vec<SearchResult>,
    latency_ms: u64,
}

/// GET /api/memory/search
pub async fn handle_search(
    state: State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Json<SearchResponse> {
    let start = std::time::Instant::now();
    
    let results = state.kernel.memory()
        .semantic_search(&params.q, params.limit, params.threshold)
        .await
        .unwrap_or_default();
    
    Json(SearchResponse {
        query: params.q,
        results,
        latency_ms: start.elapsed().as_millis() as u64,
    })
}

/// Memory stats response.
#[derive(Serialize)]
pub struct MemoryStats {
    total_entries: usize,
    index_size: usize,
    dimensions: usize,
    engine: String,
}

/// GET /api/memory/stats
pub async fn handle_stats(
    state: State<Arc<AppState>>,
) -> Json<MemoryStats> {
    let memory = state.kernel.memory();
    
    Json(MemoryStats {
        total_entries: memory.total_entries(),
        index_size: memory.index_size(),
        dimensions: memory.dimensions(),
        engine: memory.engine_name(),
    })
}

/// Migration request.
#[derive(Deserialize)]
pub struct MigrateRequest {
    source: String,  // "tfidf"
}

/// GET /api/memory/migrate
pub async fn handle_migrate(
    state: State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let progress = state.kernel.memory()
        .migrate_from_tfidf(|p| {
            tracing::info!(
                migrated = p.migrated,
                failed = p.failed,
                total = p.total,
                percent = p.percent(),
                "Migration progress"
            );
        })
        .await;
    
    match progress {
        Ok(p) => Json(serde_json::json!({
            "status": "completed",
            "migrated": p.migrated,
            "failed": p.failed,
            "errors": p.errors,
        })),
        Err(e) => Json(serde_json::json!({
            "status": "failed",
            "error": e.to_string(),
        })),
    }
}

/// Memory routes.
pub fn memory_routes() -> Router {
    Router::new()
        .route("/api/memory/search", get(handle_search))
        .route("/api/memory/stats", get(handle_stats))
        .route("/api/memory/migrate", post(handle_migrate))
}
```

---

## 10. Implementation Phases

### Phase 1: Foundation (Week 1-2)
- [ ] `memory/error.rs` — Error types
- [ ] `memory/hnsw.rs` — HNSW index wrapper (usearch)
- [ ] `memory/engine.rs` — EmbeddingEngine trait
- [ ] `memory/engine.rs` — OnnxEngine (stub, then full)
- [ ] SQLite schema and basic operations

### Phase 2: Core Integration (Week 3)
- [ ] `MemoryManager` → HNSW integration
- [ ] `semantic_search()` implementation
- [ ] Event bus integration
- [ ] Web API endpoints

### Phase 3: Polish (Week 4)
- [ ] Batch embedding optimization
- [ ] INT8 quantization (memory savings)
- [ ] Migration from TF-IDF
- [ ] Performance benchmarking

### Phase 4: Advanced (Week 5+)
- [ ] OpenAI fallback engine
- [ ] Hybrid embedding
- [ ] ReasoningBank integration
- [ ] SONA self-learning

---

## 11. Testing Strategy

### 11.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dense_vector_cosine() {
        let a = DenseVector::new(vec![1.0, 0.0, 0.0, 0.0]).unwrap();
        let b = DenseVector::new(vec![1.0, 0.0, 0.0, 0.0]).unwrap();
        let c = DenseVector::new(vec![0.0, 1.0, 0.0, 0.0]).unwrap();
        
        assert!((a.cosine_similarity(&b) - 1.0).abs() < 0.001);
        assert!(a.cosine_similarity(&c).abs() < 0.001);
    }
    
    #[test]
    fn test_int8_quantization() {
        let v = DenseVector::new(vec![0.5, -0.5, 0.0, 1.0]).unwrap();
        let quantized = v.to_int8();
        let restored = DenseVector::from_int8(&quantized, v.norm.unwrap());
        
        // Should be close to original
        for (a, b) in v.as_slice().iter().zip(restored.as_slice().iter()) {
            assert!((a - b).abs() < 0.1);
        }
    }
}
```

### 11.2 Integration Tests

```rust
#[tokio::test]
async fn test_store_and_search() {
    let tmpdir = tempfile::tempdir().unwrap();
    let state_store = Arc::new(StateStore::new(tmpdir.path()).unwrap());
    let config = MemoryConfig::default();
    
    let memory = MemoryManager::new_hnsw(state_store, &config, tmpdir.path()).await
        .unwrap();
    
    // Store entries
    memory.store(MemoryEntryInput {
        memory_type: MemoryType::Fact,
        content: "Rust is a systems programming language".to_string(),
        source: "test".to_string(),
        session_id: None,
        tags: vec!["rust".into(), "programming".into()],
        importance: 0.8,
        metadata: None,
    }).await.unwrap();
    
    // Search
    let results = memory.semantic_search("systems language rust", 5, 0.5).await.unwrap();
    
    assert!(!results.is_empty());
    assert_eq!(results[0].entry.content.contains("Rust"), true);
}
```

### 11.3 Benchmark

```rust
#[cfg(test)]
mod benchmarks {
    use test::Bencher;
    
    #[bench]
    fn bench_search_1k_entries(b: &mut Bencher) {
        // Setup 1000 entries...
        b.iter(|| {
            // runtime.block_on(async {
            //     memory.semantic_search("query", 10, 0.6)
            // });
        });
    }
}
```

---

## 12. Open Questions & Decisions

| Question | Decision | Status |
|----------|----------|--------|
| ONNX model loading | Embedded bytes vs download on first run | ⏳ |
| Quantization level | FP16 only, or FP16 + INT8 hybrid? | ⏳ |
| Batch size | Fixed 32, or auto-tune? | ⏳ |
| Search fallback | TF-IDF fallback during migration only | ⏳ |
| Model tokenizer | Tokenizers crate or simple char mapping | ⏳ |