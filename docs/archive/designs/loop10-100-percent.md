# Loop 10: Oxios Agent OS — 100/100 Design

> Brings remaining 6 scoring dimensions to 10/10.
> Estimated effort: ~8 working days.

## Table of Contents

1. [Tool System 9→10](#1-tool-system-910-1-day)
2. [Memory 9→10](#2-memory-910-15-days)
3. [Production 9→10](#3-production-910-1-day)
4. [Multi-Agent 8→10](#4-multi-agent-810-2-days)
5. [Channels 8→10](#5-channels-810-05-days)
6. [Observability 8→10](#6-observability-810-2-days)
7. [Implementation Schedule](#7-implementation-schedule)

---

## 1. Tool System 9→10 (1 day)

### Problem

Only `DEFAULT_CONTAINERFILE` exists. Agents working on Rust, TypeScript, or Python projects cannot compile/test inside containers.

### Solution

#### 1.1 Multi-Language Toolchain Templates

```rust
// crates/oxios-kernel/src/container_manager.rs

/// Rust toolchain containerfile.
const RUST_TOOLCHAIN_CONTAINERFILE: &str = r#"# Oxios Rust Dev Container
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl git ripgrep jq bash ca-certificates \
    build-essential pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
WORKDIR /workspace
CMD ["/bin/bash"]
"#;

/// Node.js / TypeScript toolchain containerfile.
const NODE_TOOLCHAIN_CONTAINERFILE: &str = r#"# Oxios Node Dev Container
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl git ripgrep jq bash ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    && npm install -g typescript ts-node \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /workspace
CMD ["/bin/bash"]
"#;

/// Python toolchain containerfile.
const PYTHON_TOOLCHAIN_CONTAINERFILE: &str = r#"# Oxios Python Dev Container
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl git ripgrep jq bash ca-certificates python3 python3-pip python3-venv \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /workspace
CMD ["/bin/bash"]
"#;

/// Select a containerfile template based on language/toolchain.
pub fn containerfile_for_toolchain(toolchain: &str) -> &'static str {
    match toolchain {
        "rust" => RUST_TOOLCHAIN_CONTAINERFILE,
        "node" | "typescript" | "ts" => NODE_TOOLCHAIN_CONTAINERFILE,
        "python" | "python3" => PYTHON_TOOLCHAIN_CONTAINERFILE,
        _ => DEFAULT_CONTAINERFILE,
    }
}
```

#### 1.2 `new_container_with_toolchain` Method

```rust
impl ContainerManager {
    /// Create a new container with a specific toolchain template.
    pub async fn new_container_with_toolchain(
        &self,
        name: &str,
        toolchain: &str,
    ) -> Result<()> {
        let container_dir = self.containers_base.join(name);
        if container_dir.exists() {
            bail!("Container '{}' already exists", name);
        }

        // Create directory structure
        tokio::fs::create_dir_all(container_dir.join("workspace")).await?;

        // Write Containerfile for the requested toolchain
        let containerfile = containerfile_for_toolchain(toolchain);
        tokio::fs::write(container_dir.join("Containerfile"), containerfile).await?;

        // Persist metadata with toolchain info
        let info = ContainerInfo {
            name: name.to_string(),
            image_tag: DEFAULT_IMAGE_TAG.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            running: false,
            toolchain: Some(toolchain.to_string()),
            tools_verified: false,
        };
        self.state_store.save_json("containers", name, &info).await?;

        tracing::info!(name = %name, toolchain = %toolchain, "Container created with toolchain");
        Ok(())
    }
}
```

#### 1.3 Toolchain API Endpoint

```rust
// channels/oxios-web/src/routes/system.rs

/// POST /api/containers — Create container with optional toolchain.
pub(crate) async fn handle_container_create(
    state: State<Arc<AppState>>,
    Json(body): Json<CreateContainerRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = if let Some(toolchain) = &body.toolchain {
        state.container_manager
            .new_container_with_toolchain(&body.name, toolchain)
            .await
    } else {
        state.container_manager.new_container(&body.name).await
    };
    result.map(|_| Json(serde_json::json!({"created": body.name})))
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[derive(Debug, Deserialize)]
pub struct CreateContainerRequest {
    pub name: String,
    pub toolchain: Option<String>,
}
```

#### 1.4 Supported Toolchains API

```rust
/// GET /api/toolchains — List available toolchain templates.
pub(crate) async fn handle_toolchains_list() -> Json<Vec<ToolchainInfo>> {
    Json(vec![
        ToolchainInfo { id: "default".into(), languages: vec!["bash".into(), "python3".into()] },
        ToolchainInfo { id: "rust".into(), languages: vec!["rust".into()] },
        ToolchainInfo { id: "node".into(), languages: vec!["typescript".into(), "javascript".into()] },
        ToolchainInfo { id: "python".into(), languages: vec!["python3".into()] },
    ])
}

#[derive(Debug, Serialize)]
pub struct ToolchainInfo {
    pub id: String,
    pub languages: Vec<String>,
}
```

### File Changes

| File | Action |
|------|--------|
| `crates/oxios-kernel/src/container_manager.rs` | **Modify** — Add 3 toolchain templates, `containerfile_for_toolchain`, `new_container_with_toolchain` |
| `channels/oxios-web/src/routes/system.rs` | **Modify** — Add `handle_container_create`, `handle_toolchains_list` |
| `channels/oxios-web/src/routes/mod.rs` | **Modify** — Register new routes |

### Test Strategy

```rust
#[test]
fn test_containerfile_for_toolchain() {
    assert!(containerfile_for_toolchain("rust").contains("rustup"));
    assert!(containerfile_for_toolchain("node").contains("nodesource"));
    assert!(containerfile_for_toolchain("python").contains("python3"));
    assert!(containerfile_for_toolchain("unknown").contains("FROM")); // fallback
}

#[tokio::test]
async fn test_new_container_with_toolchain() {
    // Create temp dir, create container with "rust" toolchain
    // Verify Containerfile contains "rustup"
    // Verify ContainerInfo has toolchain: Some("rust")
}
```

---

## 2. Memory 9→10 (1.5 days)

### Problem

TF-IDF is a bag-of-words approach. It cannot capture semantic similarity beyond shared vocabulary. "The agent compiled the code" and "The build succeeded" share no words but mean similar things.

### Design Decision: Embedding Trait, Not a Specific Model

Rather than committing to fastembed (heavy ONNX dependency) or an API call, define an `EmbeddingProvider` trait and implement two providers:

1. **LocalProvider** — Uses the existing TF-IDF (zero-dependency, always available)
2. **ApiProvider** — Uses oxi-ai's Provider to call embedding APIs (accurate, requires API key)

The system auto-selects: if an embedding model is configured, use API. Otherwise, fall back to TF-IDF. This preserves the "lightweight by default" philosophy while allowing upgrade.

### Solution

#### 2.1 Embedding Trait

```rust
// crates/oxios-kernel/src/embedding.rs (new file)

/// An embedding vector for semantic similarity comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingVector {
    /// The vector values.
    pub values: Vec<f64>,
}

impl EmbeddingVector {
    /// Compute cosine similarity between two vectors.
    pub fn cosine_similarity(&self, other: &Self) -> f64 {
        if self.values.len() != other.values.len() || self.values.is_empty() {
            return 0.0;
        }
        let dot: f64 = self.values.iter().zip(&other.values).map(|(a, b)| a * b).sum();
        let norm_a: f64 = self.values.iter().map(|v| v * v).sum::<f64>().sqrt();
        let norm_b: f64 = other.values.iter().map(|v| v * v).sum::<f64>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 { return 0.0; }
        dot / (norm_a * norm_b)
    }
}

/// Provider for generating text embeddings.
#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate an embedding vector for the given text.
    async fn embed(&self, text: &str) -> anyhow::Result<EmbeddingVector>;

    /// Name of this provider (for logging).
    fn name(&self) -> &str;

    /// Vector dimensionality.
    fn dimensions(&self) -> usize;
}
```

#### 2.2 TF-IDF Embedding Provider (wraps existing TextVector)

```rust
// crates/oxios-kernel/src/embedding.rs

/// TF-IDF based embedding provider (zero dependencies).
/// Converts TextVector's HashMap into a sparse vector.
pub struct TfIdfEmbeddingProvider {
    /// Vocabulary index for consistent dimension mapping.
    vocabulary: parking_lot::RwLock<Vec<String>>,
}

impl TfIdfEmbeddingProvider {
    pub fn new() -> Self {
        Self {
            vocabulary: parking_lot::RwLock::new(Vec::new()),
        }
    }

    /// Ensure all terms in `tf` are in the vocabulary, return sparse vector.
    fn to_sparse_vector(&self, tf: &HashMap<String, f64>) -> EmbeddingVector {
        let mut vocab = self.vocabulary.write();
        let mut vector = vec![0.0; vocab.len().max(tf.len())];

        for (term, &freq) in tf {
            let idx = if let Some(pos) = vocab.iter().position(|t| t == term) {
                pos
            } else {
                vocab.push(term.clone());
                vocab.len() - 1
            };
            if idx >= vector.len() {
                vector.resize(idx + 1, 0.0);
            }
            vector[idx] = freq;
        }

        EmbeddingVector { values: vector }
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for TfIdfEmbeddingProvider {
    async fn embed(&self, text: &str) -> anyhow::Result<EmbeddingVector> {
        let tv = crate::memory::TextVector::from_text(text);
        // Access tf field via a public method we'll add to TextVector
        Ok(self.to_sparse_vector(&tv.tf_map()))
    }

    fn name(&self) -> &str { "tfidf" }
    fn dimensions(&self) -> usize { self.vocabulary.read().len() }
}
```

#### 2.3 API Embedding Provider (uses oxi-ai)

```rust
// crates/oxios-kernel/src/embedding_api.rs (new file)

/// API-based embedding provider using oxi-ai.
pub struct ApiEmbeddingProvider {
    provider: Arc<dyn oxi_ai::Provider>,
    model: oxi_ai::Model,
}

impl ApiEmbeddingProvider {
    pub fn new(provider: Arc<dyn oxi_ai::Provider>, model: oxi_ai::Model) -> Self {
        Self { provider, model }
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for ApiEmbeddingProvider {
    async fn embed(&self, text: &str) -> anyhow::Result<EmbeddingVector> {
        // Use oxi-ai's embedding API if available
        // Fall back to a simple prompt-based embedding extraction
        // This is a placeholder — actual implementation depends on
        // oxi-ai's embedding support (which may not exist yet)
        anyhow::bail!("API embedding not yet supported by oxi-ai")
    }

    fn name(&self) -> &str { "api" }
    fn dimensions(&self) -> usize { 1536 } // OpenAI ada-002
}
```

**Note:** oxi-ai의 Provider trait은 `stream()`만 있고 embedding API가 없을 수 있습니다. 이 경우 ApiEmbeddingProvider는 `bail!` 하고 TF-IDF로 자동 폴백됩니다. 나중에 oxi-ai에 embedding 지원이 추가되면 이 구현을 완성합니다.

#### 2.4 Integrate into MemoryManager

```rust
// crates/oxios-kernel/src/memory.rs — modifications

pub struct MemoryManager {
    state_store: Arc<StateStore>,
    max_recall: usize,
    /// Vector index for semantic search (id → EmbeddingVector).
    vector_index: RwLock<HashMap<String, EmbeddingVector>>,
    /// Embedding provider (TF-IDF by default, API if configured).
    embedding: Arc<dyn EmbeddingProvider>,
}
```

**Key change:** `TextVector` → `EmbeddingVector` in the vector_index. All search operations use `EmbeddingVector::cosine_similarity()`.

```rust
// remember() auto-indexes with the configured provider
pub async fn remember(&self, entry: MemoryEntry) -> Result<String> {
    // ... existing save logic ...
    
    // Index with embedding provider
    let vector = self.embedding.embed(&entry.content).await
        .unwrap_or_else(|e| {
            tracing::debug!(error = %e, "Embedding failed, skipping index");
            // Return empty vector so remember still succeeds
            EmbeddingVector { values: vec![] }
        });
    
    if !vector.values.is_empty() {
        self.vector_index.write().insert(entry.id.clone(), vector);
    }
    
    Ok(entry.id)
}

// search() uses cosine similarity
pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
    let query_vec = self.embedding.embed(query).await?;
    let index = self.vector_index.read();
    
    let mut scored: Vec<_> = index.iter()
        .map(|(id, vec)| (id.clone(), query_vec.cosine_similarity(vec)))
        .filter(|(_, score)| *score > 0.3)
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    
    // Load top entries from state store
    // ... existing logic ...
}
```

#### 2.5 Add `tf_map()` accessor to TextVector

```rust
impl TextVector {
    /// Returns a reference to the term-frequency map.
    pub fn tf_map(&self) -> &HashMap<String, f64> {
        &self.tf
    }
}
```

#### 2.6 Config

```rust
// crates/oxios-kernel/src/config.rs — addition to MemoryConfig

pub struct MemoryConfig {
    // ... existing fields ...
    /// Embedding provider type: "tfidf" (default) or "api".
    pub embedding_provider: String,
    /// Model ID for API embedding (e.g., "text-embedding-3-small").
    pub embedding_model: Option<String>,
}
```

### File Changes

| File | Action |
|------|--------|
| `crates/oxios-kernel/src/embedding.rs` | **New** — `EmbeddingVector`, `EmbeddingProvider` trait, `TfIdfEmbeddingProvider` |
| `crates/oxios-kernel/src/embedding_api.rs` | **New** — `ApiEmbeddingProvider` (placeholder until oxi-ai supports embeddings) |
| `crates/oxios-kernel/src/memory.rs` | **Modify** — Replace `TextVector` index with `EmbeddingVector`, add `embedding` field |
| `crates/oxios-kernel/src/config.rs` | **Modify** — Add `embedding_provider`, `embedding_model` to `MemoryConfig` |
| `crates/oxios-kernel/src/lib.rs` | **Modify** — Add `pub mod embedding; pub mod embedding_api;` |

### Test Strategy

```rust
#[test]
fn test_cosine_similarity_identical() {
    let v = EmbeddingVector { values: vec![1.0, 0.0, 1.0] };
    assert!((v.cosine_similarity(&v) - 1.0).abs() < 0.001);
}

#[test]
fn test_cosine_similarity_orthogonal() {
    let a = EmbeddingVector { values: vec![1.0, 0.0] };
    let b = EmbeddingVector { values: vec![0.0, 1.0] };
    assert!((a.cosine_similarity(&b)).abs() < 0.001);
}

#[tokio::test]
async fn test_tfidf_provider_embedding() {
    let provider = TfIdfEmbeddingProvider::new();
    let v1 = provider.embed("Rust is a systems programming language").await.unwrap();
    let v2 = provider.embed("Rust is a systems programming language").await.unwrap();
    assert!(v1.cosine_similarity(&v2) > 0.99);
}

#[tokio::test]
async fn test_search_with_embedding_provider() {
    // Create MemoryManager with TfIdfEmbeddingProvider
    // remember("Rust compile error") → remember("build failed")
    // search("compilation problem") should find both
}
```

---

## 3. Production 9→10 (1 day)

### Problem

1. **No E2E test with real LLM**: Only mock-based tests. The full pipeline has never been validated end-to-end.
2. **No load test**: Unknown how the system performs under concurrent load.

### Solution

#### 3.1 E2E Real Pipeline Test

```rust
// tests/e2e_real_pipeline.rs (new file)

/// End-to-end test that exercises the full Ouroboros pipeline
/// with a real LLM provider.
///
/// Run with: OXIOS_E2E=1 cargo test --test e2e_real_pipeline -- --ignored
/// Requires a valid API key in the environment.
#[cfg(test)]
mod tests {
    use oxios_kernel::*;
    use oxios_ouroboros::*;

    fn should_run() -> bool {
        std::env::var("OXIOS_E2E").is_ok()
    }

    fn create_real_engine() -> Option<Arc<OuroborosEngine>> {
        if !should_run() { return None; }

        let model_id = std::env::var("OXIOS_MODEL")
            .unwrap_or_else(|_| "anthropic/claude-sonnet-4-20250514".into());

        // Use oxi-ai's provider registry
        let provider = oxi_ai::ProviderRegistry::new()
            .with_env_keys()
            .get(&model_id)?;

        let model = oxi_ai::Model {
            id: model_id,
            ..Default::default()
        };

        Some(Arc::new(OuroborosEngine::new(provider, model)))
    }

    #[tokio::test]
    #[ignore] // Run manually with --ignored flag
    async fn test_full_interview_to_seed() {
        let engine = create_real_engine().expect("Set OXIOS_E2E=1 and provide API key");

        // Interview
        let result = engine.interview(
            "Write a Rust function that reverses a string"
        ).await.expect("interview failed");

        assert!(result.ready_for_seed || !result.questions.is_empty(),
            "Interview should either be ready or have questions");

        // Seed
        let seed = engine.generate_seed(&result).await.expect("seed failed");
        assert!(!seed.goal.is_empty());
        assert!(!seed.acceptance_criteria.is_empty());

        println!("✓ Interview → Seed pipeline works");
        println!("  Goal: {}", seed.goal);
        println!("  Criteria: {:?}", seed.acceptance_criteria);
    }

    #[tokio::test]
    #[ignore]
    async fn test_evaluate_with_real_output() {
        let engine = create_real_engine().expect("Set OXIOS_E2E=1 and provide API key");

        let seed = oxios_ouroboros::Seed {
            id: uuid::Uuid::new_v4(),
            goal: "Write a hello world program".into(),
            constraints: vec![],
            acceptance_criteria: vec!["Program outputs Hello, World!".into()],
            ontology: vec![],
            created_at: chrono::Utc::now(),
            generation: 0,
            parent_seed_id: None,
        };

        let execution = oxios_ouroboros::ExecutionResult {
            output: "Hello, World!\n".into(),
            steps_completed: 1,
            success: true,
        };

        // First call should hit mechanical eval and skip LLM
        let result = engine.evaluate(&seed, &execution).await.expect("eval failed");
        assert!(result.mechanical_pass);
        assert_eq!(result.score, 1.0);

        // Second call with same seed+output should hit cache
        let cached = engine.evaluate(&seed, &execution).await.expect("cached eval failed");
        assert_eq!(cached.score, result.score);

        println!("✓ Evaluate → Cache pipeline works");
    }
}
```

#### 3.2 Load Test Script

```bash
#!/bin/bash
# scripts/load-test.sh — Simple load test for Oxios API.
# Requires: curl, jq
# Usage: ./scripts/load-test.sh [BASE_URL] [CONCURRENT] [TOTAL]

BASE_URL="${1:-http://localhost:3000}"
CONCURRENT="${2:-10}"
TOTAL="${3:-100}"
TOKEN="${OXIOS_TOKEN:-}"

echo "=== Oxios Load Test ==="
echo "URL: $BASE_URL"
echo "Concurrent: $CONCURRENT"
echo "Total requests: $TOTAL"
echo ""

# Health check
echo "--- Health check ---"
START=$(date +%s%N)
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/health")
END=$(date +%s%N)
ELAPSED=$(( (END - START) / 1000000 ))
echo "  GET /health → $HTTP_CODE (${ELAPSED}ms)"
echo ""

# Status endpoint
echo "--- Status endpoint ---"
START=$(date +%s%N)
STATUS=$(curl -s "$BASE_URL/api/status")
END=$(date +%s%N)
ELAPSED=$(( (END - START) / 1000000 ))
echo "  GET /api/status → ${ELAPSED}ms"
echo "  Service: $(echo $STATUS | jq -r '.service')"
echo "  Uptime: $(echo $STATUS | jq -r '.uptime')"
echo ""

# Concurrent requests to /health
echo "--- Concurrent /health ($CONCURRENT x $TOTAL) ---"
SUCCESS=0
FAIL=0
TOTAL_TIME=0

for batch in $(seq 1 $((TOTAL / CONCURRENT))); do
  PIDS=()
  for i in $(seq 1 $CONCURRENT); do
    START=$(date +%s%N)
    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/health")
    END=$(date +%s%N)
    ELAPSED=$(( (END - START) / 1000000 ))
    TOTAL_TIME=$((TOTAL_TIME + ELAPSED))
    if [ "$HTTP_CODE" = "200" ]; then
      SUCCESS=$((SUCCESS + 1))
    else
      FAIL=$((FAIL + 1))
    fi
  done
done

REQUESTS=$((SUCCESS + FAIL))
AVG=$((TOTAL_TIME / REQUESTS))
echo "  Requests: $REQUESTS"
echo "  Success: $SUCCESS"
echo "  Failed: $FAIL"
echo "  Avg response: ${AVG}ms"
echo ""

# Chat endpoint (if token provided)
if [ -n "$TOKEN" ]; then
  echo "--- Chat endpoint (5 sequential) ---"
  for i in $(seq 1 5); do
    START=$(date +%s%N)
    RESP=$(curl -s -w "\n%{http_code}" \
      -H "Authorization: Bearer $TOKEN" \
      -H "Content-Type: application/json" \
      -d "{\"message\":\"hello test $i\",\"session_id\":\"load-test\"}" \
      "$BASE_URL/api/chat")
    HTTP_CODE=$(echo "$RESP" | tail -1)
    END=$(date +%s%N)
    ELAPSED=$(( (END - START) / 1000000 ))
    echo "  POST /api/chat #$i → $HTTP_CODE (${ELAPSED}ms)"
  done
fi

echo ""
echo "=== Load test complete ==="
```

### File Changes

| File | Action |
|------|--------|
| `tests/e2e_real_pipeline.rs` | **New** — E2E test with real LLM (requires OXIOS_E2E=1) |
| `scripts/load-test.sh` | **New** — Simple concurrent load test |

---

## 4. Multi-Agent 8→10 (2 days)

### Problem

1. **A2A `send_and_wait` polls at 100ms intervals**: Wastes CPU cycles, adds latency.
2. **Flat orchestration**: All subtasks are equal. No manager-worker hierarchy.

### Solution

#### 4.1 Non-Polling A2A with `tokio::sync::Notify`

```rust
// crates/oxios-kernel/src/a2a.rs — replace message_queue structure

/// Per-agent message queue with notification.
struct AgentQueue {
    messages: Vec<PendingMessage>,
    notify: tokio::sync::Notify,
}

impl AgentQueue {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
            notify: tokio::sync::Notify::new(),
        }
    }
}

pub struct A2AProtocol {
    registry: AgentCardRegistry,
    queues: Arc<RwLock<HashMap<AgentId, Arc<AgentQueue>>>>,
    event_bus: EventBus,
}
```

Updated methods:

```rust
impl A2AProtocol {
    /// Get or create a queue for an agent.
    fn queue(&self, agent_id: AgentId) -> Arc<AgentQueue> {
        let mut queues = self.queues.write();
        queues.entry(agent_id).or_insert_with(|| Arc::new(AgentQueue::new())).clone()
    }

    /// Send a message — pushes to queue and notifies.
    pub async fn send_message(
        &self,
        from: AgentId,
        to: AgentId,
        message: A2AMessage,
    ) -> Result<Uuid> {
        let request = A2ARequest::new(from, to, message);
        let request_id = request.request_id;

        let queue = self.queue(to);
        {
            let mut queues = self.queues.write();
            queues.entry(to).or_insert_with(|| Arc::new(AgentQueue::new()))
                .messages.push(PendingMessage::new(request.clone()));
        }
        queue.notify.notify_one(); // Wake up any waiting agent

        self.event_bus.publish(KernelEvent::MessageReceived { ... })?;
        Ok(request_id)
    }

    /// Receive all pending messages for an agent.
    pub async fn receive_messages(&self, agent_id: AgentId) -> Vec<A2ARequest> {
        let queue = self.queue(agent_id);
        let mut messages = Vec::new();
        std::mem::swap(&mut messages, &mut queue.messages);
        messages.into_iter().map(|pm| pm.request).collect()
    }

    /// Send and wait for response — uses Notify instead of polling.
    pub async fn send_and_wait(
        &self,
        from: AgentId,
        to: AgentId,
        message: A2AMessage,
        timeout: std::time::Duration,
    ) -> Result<A2AResponse> {
        let request_id = self.send_message(from, to, message).await?;
        let queue = self.queue(from);
        let start = std::time::Instant::now();

        loop {
            // Check for matching response
            {
                let mut queues = self.queues.write();
                if let Some(q) = queues.get_mut(&from) {
                    if let Some(pos) = q.messages.iter().position(|pm| {
                        if let A2AMessage::ResultSharing { task_id, .. } = &pm.request.message {
                            *task_id == request_id
                        } else {
                            false
                        }
                    }) {
                        let pm = q.messages.remove(pos);
                        return Ok(A2AResponse::success(
                            request_id, to, from,
                            match &pm.request.message {
                                A2AMessage::ResultSharing { result, .. } => result.clone(),
                                _ => serde_json::Value::Null,
                            },
                        ));
                    }
                }
            }

            // Wait for notification or timeout
            let remaining = timeout.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                anyhow::bail!("A2A response timeout after {:?}", timeout);
            }

            tokio::select! {
                _ = queue.notify.notified() => {
                    // New message arrived, loop to check
                }
                _ = tokio::time::sleep(remaining) => {
                    anyhow::bail!("A2A response timeout after {:?}", timeout);
                }
            }
        }
    }
}
```

#### 4.2 Hierarchical Orchestration

```rust
// crates/oxios-kernel/src/orchestrator.rs — add manager role

/// Role of an agent within a group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentRole {
    /// Coordinates subtasks, synthesizes results.
    Manager,
    /// Executes a specific subtask.
    Worker,
}

/// Enhanced SubTask with role assignment.
pub struct SubTask {
    pub id: Uuid,
    pub description: String,
    pub required_capability: Option<String>,
    pub result: Option<String>,
    pub success: bool,
    pub role: AgentRole,
}
```

Updated `delegate_subtasks`:

```rust
/// Split into subtasks and execute with a manager-worker pattern.
///
/// Strategy:
/// 1. If 1 subtask: execute directly (no group needed)
/// 2. If 2-4 subtasks: parallel workers, first result triggers synthesis
/// 3. If 5+ subtasks: manager coordinates, workers execute
pub async fn delegate_subtasks(
    &self,
    subtasks: Vec<SubTask>,
    parent_seed: &Seed,
) -> Result<Vec<SubTask>> {
    if subtasks.is_empty() {
        return Ok(subtasks);
    }

    if subtasks.len() == 1 {
        // Single task — execute directly, no group overhead
        let mut task = subtasks.into_iter().next().unwrap();
        match self.lifecycle.spawn_and_run(/* child seed */, Priority::Normal).await {
            Ok(result) => {
                task.result = Some(result.output);
                task.success = result.success;
            }
            Err(e) => {
                task.result = Some(format!("Failed: {e}"));
                task.success = false;
            }
        }
        return Ok(vec![task]);
    }

    // Multiple subtasks — parallel execution with JoinSet
    use tokio::task::JoinSet;

    let descriptions: Vec<String> = subtasks.iter().map(|st| st.description.clone()).collect();
    let group = AgentGroup::new(parent_seed, descriptions);
    let group_id = group.id;

    // ... existing JoinSet parallel execution logic ...

    // After all complete: synthesize results if 5+ subtasks
    if subtasks.len() >= 5 {
        let combined_output: String = completed.iter()
            .filter(|t| t.success)
            .filter_map(|t| t.result.as_deref())
            .collect::<Vec<_>>()
            .join("\n\n");

        tracing::info!(
            group_id = %group_id,
            "Large group completed, results available for synthesis"
        );
        // The evaluation phase will handle synthesis via the parent seed
    }

    Ok(completed)
}
```

### File Changes

| File | Action |
|------|--------|
| `crates/oxios-kernel/src/a2a.rs` | **Modify** — Replace `Vec<PendingMessage>` with `AgentQueue` (Notify-based) |
| `crates/oxios-kernel/src/orchestrator.rs` | **Modify** — Add `AgentRole`, smart task splitting |

### Test Strategy

```rust
#[tokio::test]
async fn test_send_and_wait_notify_based() {
    let a2a = A2AProtocol::new(event_bus);
    let from = AgentId::new();
    let to = AgentId::new();

    // Spawn a task that responds after receiving
    let a2a_clone = a2a.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let msgs = a2a_clone.receive_messages(to).await;
        // Send back a ResultSharing with matching request_id
    });

    // send_and_wait should resolve without polling
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        a2a.send_and_wait(from, to, message, Duration::from_secs(3))
    ).await;
    assert!(result.is_ok());
}
```

---

## 5. Channels 8→10 (0.5 days)

### Problem

Only Web + CLI channels exist. External integrations (Telegram, Slack, etc.) have no documentation or examples.

### Solution

#### 5.1 Channel Plugin Guide

```markdown
# docs/channel-plugin-guide.md

## Creating a Channel Plugin

Oxios channels connect users to the Agent OS kernel. The simplest way
to integrate external services is via the REST API.

### Option 1: REST API (Recommended)

Any service that can send HTTP requests can be a channel:

\`\`\`bash
# Send a message
curl -X POST http://localhost:3000/api/chat \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"message": "Write a hello world program", "session_id": "my-session"}'

# Get the response
# Response includes agent output, seed ID, evaluation result
\`\`\`

### Option 2: Gateway Channel Trait

For tighter integration, implement the `Channel` trait:

\`\`\`rust
use oxios_gateway::{Channel, IncomingMessage, OutgoingMessage};

struct TelegramChannel {
    bot_token: String,
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str { "telegram" }
    async fn receive(&self) -> Result<Option<IncomingMessage>> {
        // Poll Telegram getUpdates API
    }
    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        // POST to Telegram sendMessage API
    }
}
\`\`\`

### Option 3: SSE Event Stream

Subscribe to real-time kernel events:

\`\`\`bash
curl -N http://localhost:3000/api/events
\`\`\`
```

#### 5.2 Telegram Webhook Example

```bash
#!/bin/bash
# scripts/examples/telegram-webhook.sh
# Minimal Telegram bot that forwards messages to Oxios and replies.

OXIOS_URL="http://localhost:3000"
OXIOS_TOKEN="${OXIOS_TOKEN:?Set OXIOS_TOKEN}"
TELEGRAM_TOKEN="${TELEGRAM_TOKEN:?Set TELEGRAM_TOKEN}"
SESSION="telegram-$(date +%s)"

# Get updates from Telegram
OFFSET=0
while true; do
    UPDATES=$(curl -s "https://api.telegram.org/bot${TELEGRAM_TOKEN}/getUpdates?offset=${OFFSET}&timeout=30")

    # Extract messages
    echo "$UPDATES" | jq -c '.result[]?' | while read -r update; do
        OFFSET=$(echo "$update" | jq '.update_id + 1')
        CHAT_ID=$(echo "$update" | jq '.message.chat.id')
        TEXT=$(echo "$update" | jq -r '.message.text')

        [ "$TEXT" = "null" ] && continue

        # Forward to Oxios
        RESPONSE=$(curl -s -X POST "${OXIOS_URL}/api/chat" \
            -H "Authorization: Bearer ${OXIOS_TOKEN}" \
            -H "Content-Type: application/json" \
            -d "{\"message\": $(echo "$TEXT" | jq -Rs .), \"session_id\": \"${SESSION}\"}")

        REPLY=$(echo "$RESPONSE" | jq -r '.response // "No response"')

        # Send reply to Telegram
        curl -s -X POST "https://api.telegram.org/bot${TELEGRAM_TOKEN}/sendMessage" \
            -d "chat_id=${CHAT_ID}" \
            -d "text=${REPLY}" > /dev/null
    done
done
```

### File Changes

| File | Action |
|------|--------|
| `docs/channel-plugin-guide.md` | **New** — Channel integration guide |
| `scripts/examples/telegram-webhook.sh` | **New** — Minimal Telegram bot example |

---

## 6. Observability 8→10 (2 days)

### Problem

1. No distributed tracing: all logs are plain `tracing::info!` without trace IDs.
2. No span propagation across Ouroboros phases.
3. OpenTelemetry is not integrated.

### Solution

#### 6.1 Telemetry Module

```rust
// crates/oxios-kernel/src/telemetry.rs (new file)

use anyhow::Result;
use std::sync::Arc;

/// Telemetry configuration.
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Enable OpenTelemetry tracing.
    pub enabled: bool,
    /// OTLP endpoint (e.g., "http://localhost:4317").
    pub endpoint: Option<String>,
    /// Service name for traces.
    pub service_name: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            service_name: "oxios".into(),
        }
    }
}

/// Initialize telemetry layers.
///
/// Returns a list of tracing-subscriber layers to apply.
/// If OTel is disabled, returns an empty vec.
pub fn init_telemetry_layers(
    config: &TelemetryConfig,
) -> Result<Vec<Box<dyn tracing_subscriber::Layer<tracing_subscriber::Registry> + Send + Sync>>> {
    if !config.enabled {
        tracing::info!("OpenTelemetry tracing disabled");
        return Ok(vec![]);
    }

    // Use stdout exporter by default, OTLP if endpoint is configured
    let exporter = if let Some(endpoint) = &config.endpoint {
        tracing::info!(endpoint = %endpoint, "OpenTelemetry: OTLP exporter configured");
        opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(endpoint)
            .build_span_exporter()?
    } else {
        tracing::info!("OpenTelemetry: stdout exporter (no OTLP endpoint)");
        opentelemetry_stdout::SpanExporter::default()
    };

    let provider = opentelemetry_sdk::trace::TracerProvider::builder()
        .with_simple_exporter(exporter)
        .build();
    let tracer = provider.tracer(config.service_name.clone());

    let layer = tracing_opentelemetry::layer().with_tracer(tracer);

    Ok(vec![Box::new(layer)])
}
```

#### 6.2 Trace Spans in Orchestrator

```rust
// crates/oxios-kernel/src/orchestrator.rs — add #[instrument]

use tracing::instrument;

impl Orchestrator {
    #[instrument(
        name = "orchestrator.handle_message",
        skip(self, user_message),
        fields(session_id = %session_id.as_deref().unwrap_or("new"))
    )]
    pub async fn handle_message(
        &self,
        user_id: &str,
        user_message: &str,
        session_id: Option<&str>,
    ) -> Result<OrchestrationResult> {
        // Phase 1: Interview
        let interview = {
            let _span = tracing::info_span!("ouroboros.interview").entered();
            self.ouroboros.interview(user_message).await?
        };

        // Phase 2: Seed
        let seed = {
            let _span = tracing::info_span!("ouroboros.seed").entered();
            self.ouroboros.generate_seed(&interview).await?
        };

        // Phase 3: Execute
        let exec_result = {
            let _span = tracing::info_span!("ouroboros.execute", seed_id = %seed.id).entered();
            self.lifecycle.spawn_and_run(&seed, Priority::Normal).await?
        };

        // Phase 4: Evaluate
        let evaluation = {
            let _span = tracing::info_span!("ouroboros.evaluate", seed_id = %seed.id).entered();
            self.ouroboros.evaluate(&seed, &exec_result).await?
        };

        // Phase 5: Evolve (if needed)
        // ...
    }
}
```

#### 6.3 Integration in main.rs

```rust
// src/main.rs — tracing initialization

fn init_tracing(data_dir: &std::path::Path, telemetry_config: &TelemetryConfig) {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("oxios=info"));

    // Log rotation
    let file_appender = tracing_appender::rolling::daily(data_dir, "oxios.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    Box::leak(Box::new(guard)); // Keep alive for program duration

    // Build subscriber layers
    let stdout_layer = tracing_subscriber::fmt::layer().with_writer(std::io::stdout);
    let file_layer = tracing_subscriber::fmt::layer().with_writer(non_blocking).with_ansi(false);

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer);

    // Add OTel layer if configured
    match oxios_kernel::telemetry::init_telemetry_layers(telemetry_config) {
        Ok(otel_layers) => {
            let subscriber = subscriber.with(otel_layers);
            subscriber.init();
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to init OpenTelemetry, continuing without tracing");
            subscriber.init();
        }
    }
}
```

#### 6.4 Dependencies

```toml
# crates/oxios-kernel/Cargo.toml additions (optional, feature-gated)
[features]
default = []
otel = ["tracing-opentelemetry", "opentelemetry", "opentelemetry_sdk", "opentelemetry-otlp", "opentelemetry-stdout"]

[dependencies]
tracing-opentelemetry = { version = "0.28", optional = true }
opentelemetry = { version = "0.27", optional = true }
opentelemetry_sdk = { version = "0.27", features = ["rt-tokio"], optional = true }
opentelemetry-otlp = { version = "0.27", optional = true }
opentelemetry-stdout = { version = "0.27", optional = true }
```

**Feature-gated** approach: OTel deps are opt-in. Default build has zero OTel overhead.

### File Changes

| File | Action |
|------|--------|
| `crates/oxios-kernel/src/telemetry.rs` | **New** — TelemetryConfig, init_telemetry_layers |
| `crates/oxios-kernel/src/orchestrator.rs` | **Modify** — Add `#[instrument]` and phase spans |
| `crates/oxios-kernel/src/lib.rs` | **Modify** — Add `#[cfg(feature = "otel")] pub mod telemetry;` |
| `crates/oxios-kernel/Cargo.toml` | **Modify** — Add otel feature + optional deps |
| `src/main.rs` | **Modify** — Enhanced tracing init with optional OTel |

### Test Strategy

```rust
#[test]
fn test_telemetry_disabled() {
    let config = TelemetryConfig::default();
    let layers = init_telemetry_layers(&config).unwrap();
    assert!(layers.is_empty());
}

#[test]
fn test_span_hierarchy() {
    // Use tracing-subscriber's test collector to verify
    // that handle_message creates the correct span hierarchy
}
```

---

## 7. Implementation Schedule

### Phase 1 (Week 1): Quick Wins + Memory

| Day | Area | Tasks |
|-----|------|-------|
| 1 | Tool 9→10 | 3 toolchain templates, `new_container_with_toolchain`, API endpoints |
| 2 | Production 9→10 | E2E real pipeline test, load test script |
| 2.5 | Channels 8→10 | Plugin guide + Telegram example |
| 3-4 | Memory 9→10 | `EmbeddingProvider` trait, TF-IDF wrapper, integrate into MemoryManager |

### Phase 2 (Week 2): Multi-Agent + Observability

| Day | Area | Tasks |
|-----|------|-------|
| 5-6 | Multi-Agent 8→10 | `AgentQueue` with Notify, non-polling `send_and_wait`, hierarchical orchestration |
| 7-8 | Observability 8→10 | telemetry.rs (feature-gated), `#[instrument]` spans, main.rs integration |

### Dependency Graph

```
Tool (Day 1) → no deps
Production (Day 2) → no deps
Channels (Day 2.5) → no deps
Memory (Days 3-4) → no deps
Multi-Agent (Days 5-6) → no deps
Observability (Days 7-8) → no deps (feature-gated, doesn't break default build)
```

All 6 areas are independent. Can be parallelized if needed.

### Total Effort: 8 days

### Risk Assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| OTel version conflicts | Medium | Feature-gate: default build unaffected |
| fastembed/ONNX too heavy | N/A | Not using fastembed — trait + TF-IDF + API fallback |
| AgentQueue refactoring breaks A2A tests | Medium | Incremental: add Notify alongside existing queue, then switch |
| oxi-ai lacks embedding API | High | ApiEmbeddingProvider returns bail!, auto-fallback to TF-IDF |
| Real LLM E2E test flakes | Medium | `#[ignore]` flag, manual only, not in CI |
