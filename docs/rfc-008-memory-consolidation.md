# RFC-008: Memory Consolidation — Tiered Memory with Dream-Time Compaction

> **상태:** 초안  
> **날짜:** 2026-05-25  
> **이전:** rfc-003 (Knowledge Separation), rfc-004 (Knowledge System), rfc-005 (Knowledge Integration)  
> **범위:** `crates/oxios-kernel/src/memory/`, `crates/oxios-kernel/src/config.rs`, `src/kernel.rs`

---

## 1. 동기 (Motivation)

### 1.1 현재 문제

Oxios의 메모리 시스템은 기능적으로 작동하지만, 장기 운영에서 근본적 한계에 직면한다:

**문제 1: 평면적 메모리 (Flat Memory)**
모든 메모리 엔트리가 동일한 우선순위로 저장된다. 어제의 대화 요약과 3개월 전의 아키텍처 결정이 같은 `importance: 0.5`를 가진다. 시간이 지날수록 노이즈가 누적되고 신호가 묻힌다.

**문제 2: 압축 없음 (No Compaction)**
세션이 길어져도 자동 요약이 없다. `ConversationBuffer`는 최근 50턴만 유지하고, `summarize_session()`은 세션 종료 시 한 번만 실행된다. Claude Code의 Auto Dream처럼 세션 간 메모리를 정리하는 메커니즘이 없다.

**문제 3: 능동적 회상 없음 (No Proactive Recall)**
메모리는 검색 기반(search-based)으로만 접근한다. 사용자가 명시적으로 관련 주제를 언급하지 않으면, 3주 전에 내린 결정이 현재 작업과 연결되지 않는다. "모르는 것을 모르는" 문제.

**문제 4: 망각 없음 (No Forgetting)**
Ebbinghaus의 망각 곡선에 따르면, 시간이 지남에 따라 정보의 중요성은 감쇠한다. 현재 Oxios에는 메모리 감쇠 메커니즘이 없다. `retention_days` 설정은 있지만 실제로 사용되지 않는다 (항상 0 = 무제한).

**문제 5: 계층적 인덱스 없음 (No Hierarchical Index)**
Hipocampus의 ROOT.md 개념처럼, 에이전트가 "내가 무엇을 알고 있는가?"를 O(1)에 파악할 수 있는 인덱스가 없다. 모든 recall은 HNSW 벡터 검색에 의존한다.

### 1.2 영감

이 RFC는 다음 시스템들로부터 영감을 받았다:

| 시스템 | 핵심 아이디어 | Oxios에의 적용 |
|--------|-------------|----------------|
| **Claude Code Auto Dream** | 4-stage consolidation (Orient → Gather Signal → Consolidate → Prune & Index) | Dream 프로세스: 백그라운드 메모리 정리 |
| **Hipocampus** | 3-tier Hot/Warm/Cold + 5-level compaction tree + ROOT.md | 메모리 계층 + 압축 트리 |
| **MemGPT/Letta** | Core/Archival/Recall memory hierarchy | 세 계층 메모리 모델 |
| **Zep** | Temporal knowledge graph with state change reasoning | 시간 메타데이터 추적 |
| **Ebbinghaus** | Forgetting curve: R = e^(-t/S) | 중요도 감쇠 공식 |
| **SOAR/ACT-R** | Episodic/Semantic/Procedural memory 분리 | 메모리 타입 분류 |
| **Human hippocampus** | Short-term → long-term consolidation during sleep | Dream-time consolidation |

### 1.3 목표

1. **세션 압축 (Session Compaction)**: 긴 세션을 자동 요약, 핵심 결정 보존, 모순 감지
2. **메모리 계층 (Memory Tiering)**: Short-term → Working → Long-term 3계층
3. **메모리 수명주기 (Memory Lifecycle)**: creation → access → decay → consolidation → archival → deletion
4. **메모리 타입 (Memory Types)**: fact, episode, skill, preference, decision, user_profile
5. **중요도 점수 (Importance Scoring)**: 접근 빈도, 최신성, 타입, 명시적 표시 기반
6. **망각/가지치기 (Forgetting/Pruning)**: Ebbinghaus 영감 감쇠, 설정 가능한 보존 정책
7. **압축 트리 (Compaction Tree)**: Raw → Daily → Weekly → Monthly → Root
8. **능동적 회상 (Proactive Recall)**: 관련 메모리를 컨텍스트에 자동 주입
9. **벡터 검색 (Vector Search)**: 기존 HNSW 인덱스 유지 및 강화
10. **메모리 예산 (Memory Budgets)**: 계층별 토큰 한도, 타입별 최대 엔트리

---

## 2. 아키텍처 개요

### 2.1 전체 구조

```
┌─────────────────────────────────────────────────────────────────┐
│                     Agent Runtime (agent_runtime.rs)              │
│                                                                   │
│  System Prompt                                                    │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ ## Active Context (Hot Tier, ~3K tokens)                   │ │
│  │ - ROOT index: [project/active, recent patterns, topics]    │ │
│  │ - User profile: [preferences, expertise, language]         │ │
│  │ - Active session context                                    │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│  recall_for_context(query)                                        │
│  ├── 1. ROOT.md triage (O(1) — topic index lookup)              │
│  ├── 2. Manifest-based LLM selection (cross-domain)             │
│  ├── 3. HNSW vector search (semantic)                           │
│  └── 4. Keyword fallback (BM25-style)                           │
│                                                                   │
│  remember(entry) → Tier 1 (Hot)                                  │
│  forget(id)      → Tier downshift or deletion                    │
│  consolidate()   → Dream process                                 │
└───────────────────────────┬─────────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────────┐
│                   MemoryManager (memory/mod.rs)                    │
│                                                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │  Tier 1: Hot │  │ Tier 2: Warm │  │  Tier 3: Cold          │ │
│  │  (Always     │  │ (On-Demand)  │  │  (Compressed Archive)  │ │
│  │   Loaded)    │  │              │  │                        │ │
│  │              │  │              │  │                        │ │
│  │ • ROOT index │  │ • Daily logs │  │ • Compaction tree:     │ │
│  │ • User prefs │  │ • Knowledge  │  │   Raw→Daily→Weekly→    │ │
│  │ • Active ctx │  │ • Plans      │  │   Monthly→Root         │ │
│  │ • ~3K tokens │  │ • Recent     │  │ • HNSW vector index    │ │
│  │              │  │   episodes   │  │ • Deep knowledge       │ │
│  └──────┬───────┘  └──────┬───────┘  └───────────┬────────────┘ │
│         │                  │                       │              │
│         │    compaction    │     archival          │              │
│         │    (Dream)       │     (decay)           │              │
│         ◄──────────────────►◄─────────────────────►              │
│                                                                   │
│  ┌──────────────────────────────────────────────────────────────┐│
│  │              Dream Process (background consolidation)        ││
│  │                                                              ││
│  │  Phase 1: Orient — scan current state, build map            ││
│  │  Phase 2: Gather Signal — find patterns, contradictions     ││
│  │  Phase 3: Consolidate — compress, dedupe, resolve conflicts ││
│  │  Phase 4: Prune & Index — update ROOT, remove stale entries ││
│  └──────────────────────────────────────────────────────────────┘│
│                                                                   │
│  ┌──────────────────────────────────────────────────────────────┐│
│  │              Supporting Systems                              ││
│  │                                                              ││
│  │  • ImportanceScorer — access freq × recency × type × mark   ││
│  │  • DecayEngine — Ebbinghaus-inspired forgetting curve        ││
│  │  • CompactionTree — Raw→Daily→Weekly→Monthly→Root           ││
│  │  • MemoryGraph — PageRank co-access importance               ││
│  │  • HNSW Index — semantic vector search (usearch)             ││
│  │  • EmbeddingCache — LRU + TTL for embedding vectors          ││
│  └──────────────────────────────────────────────────────────────┘│
└───────────────────────────────────────────────────────────────────┘
```

### 2.2 데이터 흐름

```
                    ┌─────────── 새 메모리 생성 ───────────┐
                    │                                        │
                    ▼                                        │
            ┌───────────────┐                               │
            │  Tier 1: Hot  │ ◄─── remember()               │
            │  (always in   │                                │
            │   context)    │ ──── access() → update stats   │
            └───────┬───────┘                                │
                    │                                        │
          capacity? │ over budget                            │
                    ▼                                        │
            ┌───────────────┐                               │
            │  Tier 2: Warm │ ◄─── shift_down()             │
            │  (on-demand)  │                                │
            └───────┬───────┘                                │
                    │                                        │
          decay?    │ importance < threshold                 │
                    ▼                                        │
            ┌───────────────┐                               │
            │  Tier 3: Cold │ ◄─── archive()                │
            │  (compressed) │                                │
            └───────┬───────┘                                │
                    │                                        │
          expired?  │ past retention + below min importance  │
                    ▼                                        │
               [DELETED] ──── forget() ──────────────────────┘
```

### 2.3 Dream Process 흐름

```
Idle (min 24h since last dream, min 5 sessions since last dream)
  │
  ▼
Phase 1: Orient ─── Scan all tiers, build current state map
  │
  ▼
Phase 2: Gather Signal ─── Analyze recent sessions for:
  │  • User corrections
  │  • Recurring themes
  │  • Key decisions
  │  • Explicit saves
  │  • Contradictions with existing memory
  ▼
Phase 3: Consolidate ─── Process:
  │  • Convert relative dates → absolute dates
  │  • Remove contradictions (keep newer)
  │  • Merge duplicates
  │  • Compress verbose entries → concise summaries
  │  • Promote high-value entries to higher tiers
  │  • Downgrade low-value entries to lower tiers
  ▼
Phase 4: Prune & Index ─── Finalize:
  │  • Update ROOT index
  │  • Rebuild HNSW index if changed >10%
  │  • Remove entries below decay threshold
  │  • Persist compaction tree updates
  │  • Log dream report
  ▼
[Complete] ─── Resume idle
```

---

## 3. 메모리 계층 모델 (Memory Tier Model)

### 3.1 Tier 1: Hot (항상 로드됨, ~3K tokens)

세션 시작 시 항상 컨텍스트에 로드되는 작은 고정 크기 인덱스.

**내용:**
- **ROOT 인덱스**: 모든 과거 지식의 압축된 주제 인덱스 (~100 lines)
- **User Profile**: 사용자 선호도, 전문 분야, 언어
- **Active Context**: 현재 작업, 최근 결정 (최근 7일)
- **Recent Patterns**: 최근에 반복된 패턴

**특징:**
- 항상 에이전트 컨텍스트에 주입
- ~3K 토큰으로 고정 (설정 가능)
- 세션 간 일관성 보장
- Hipocampus의 ROOT.md에서 영감

### 3.2 Tier 2: Warm (온디맨드 로드)

필요할 때만 로드되는 중간 계층.

**내용:**
- 일별 로그 (`daily/YYYY-MM-DD.md`)
- 지식 베이스 항목
- 계획/작업 상태
- 최근 에피소드 메모리 (30일 이내)

**특징:**
- 요청 시에만 로드
- 압축되지 않은 원본 내용
- 토큰 예산 내에서 자유롭게 접근

### 3.3 Tier 3: Cold (압축 아카이브)

장기 보관을 위해 압축된 아카이브.

**내용:**
- 압축 트리: Raw → Daily → Weekly → Monthly → Root
- 심층 지식 베이스
- 과거 에피소드 (30일 초과)
- HNSW 벡터 인덱스

**특징:**
- 압축 요약 형태
- HNSW 검색으로만 접근
- 감쇠 점수 적용
- 보존 정책에 따라 자동 삭제

---

## 4. 데이터 구조 (Rust Structs)

### 4.1 MemoryTier

```rust
/// Memory tier classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTier {
    /// Always loaded into agent context (~3K tokens).
    Hot,
    /// Loaded on demand (recent sessions, knowledge).
    Warm,
    /// Compressed archive (long-term storage).
    Cold,
}

impl MemoryTier {
    /// Maximum entries per tier (configurable).
    pub fn default_max_entries(&self) -> usize {
        match self {
            MemoryTier::Hot => 50,
            MemoryTier::Warm => 500,
            MemoryTier::Cold => 10_000,
        }
    }

    /// Maximum token budget per tier.
    pub fn default_token_budget(&self) -> usize {
        match self {
            MemoryTier::Hot => 3_000,
            MemoryTier::Warm => 50_000,
            MemoryTier::Cold => usize::MAX,
        }
    }
}
```

### 4.2 MemoryType (확장)

```rust
/// Memory entry type — expanded from 5 to 8 types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    // Existing types
    /// Conversation compaction summary (auto-generated).
    Conversation,
    /// Session-end summary (auto-generated).
    Session,

    // New types (from SOAR/ACT-R cognitive model)
    /// A factual statement (e.g., "API uses port 3000").
    Fact,
    /// An event or experience (e.g., "deployed v0.2.0 on 2026-05-20").
    Episode,
    /// A learned procedure or pattern (e.g., "run cargo test before commit").
    Skill,
    /// A user preference (e.g., "use Korean for user-facing messages").
    Preference,
    /// A recorded decision with rationale (e.g., "chose HNSW over FAISS because...").
    Decision,
    /// User identity and expertise profile.
    UserProfile,
}

impl MemoryType {
    /// Base importance for each type.
    pub fn base_importance(&self) -> f32 {
        match self {
            MemoryType::UserProfile => 0.95,  // Always preserve
            MemoryType::Preference => 0.90,   // Almost always preserve
            MemoryType::Decision => 0.80,     // Important, may age
            MemoryType::Skill => 0.75,        // Valuable, decays slowly
            MemoryType::Fact => 0.60,         // Useful, normal decay
            MemoryType::Episode => 0.50,      // Contextual, faster decay
            MemoryType::Session => 0.40,      // Auto-generated, compacts well
            MemoryType::Conversation => 0.35, // Auto-generated, fastest decay
        }
    }

    /// Decay rate for each type (higher = faster forgetting).
    /// Based on Ebbinghaus: how quickly importance drops.
    pub fn decay_rate(&self) -> f32 {
        match self {
            MemoryType::UserProfile => 0.001,  // Nearly permanent
            MemoryType::Preference => 0.002,
            MemoryType::Decision => 0.005,
            MemoryType::Skill => 0.008,
            MemoryType::Fact => 0.015,
            MemoryType::Episode => 0.025,
            MemoryType::Session => 0.040,
            MemoryType::Conversation => 0.060,
        }
    }

    /// Category name used in StateStore.
    pub fn category(&self) -> &'static str {
        match self {
            MemoryType::Conversation => "memory/conversations",
            MemoryType::Session => "memory/sessions",
            MemoryType::Fact => "memory/facts",
            MemoryType::Episode => "memory/episodes",
            MemoryType::Skill => "memory/skills",
            MemoryType::Preference => "memory/preferences",
            MemoryType::Decision => "memory/decisions",
            MemoryType::UserProfile => "memory/profiles",
        }
    }
}
```

### 4.3 MemoryEntry (확장)

```rust
/// A single memory entry — extended with lifecycle metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    // ── Identity ──────────────────────────────────────
    /// Unique ID.
    pub id: String,
    /// Memory type.
    pub memory_type: MemoryType,
    /// Current tier.
    #[serde(default = "default_tier")]
    pub tier: MemoryTier,

    // ── Content ───────────────────────────────────────
    /// Content (Markdown).
    pub content: String,
    /// Content hash for deduplication.
    #[serde(default)]
    pub content_hash: u64,
    /// Tags for search.
    #[serde(default)]
    pub tags: Vec<String>,

    // ── Source ────────────────────────────────────────
    /// Creator (agent name, "compaction", "system", "dream").
    pub source: String,
    /// Related session ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Related space ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,

    // ── Importance ────────────────────────────────────
    /// Base importance (0.0–1.0), set by type or explicitly.
    #[serde(default = "default_importance")]
    pub importance: f32,
    /// Whether user explicitly marked this as important.
    #[serde(default)]
    pub pinned: bool,

    // ── Lifecycle ─────────────────────────────────────
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last access timestamp.
    pub accessed_at: DateTime<Utc>,
    /// Last modification timestamp.
    #[serde(default = "default_now")]
    pub modified_at: DateTime<Utc>,
    /// Access count.
    #[serde(default)]
    pub access_count: u32,
    /// Current decay score (0.0–1.0), computed by DecayEngine.
    #[serde(default = "default_importance")]
    pub decay_score: f32,
    /// Compaction level (0 = raw, 1 = daily, 2 = weekly, 3 = monthly, 4 = root).
    #[serde(default)]
    pub compaction_level: u8,
    /// IDs of entries this was compacted from.
    #[serde(default)]
    pub compacted_from: Vec<String>,

    // ── Relationships ─────────────────────────────────
    /// IDs of related memory entries.
    #[serde(default)]
    pub related_ids: Vec<String>,
    /// Contradicts a previous entry (ID of the contradicted entry).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contradicts: Option<String>,
}
```

### 4.4 RootIndex

```rust
/// ROOT index — the "table of contents" for all agent knowledge.
///
/// Inspired by Hipocampus ROOT.md. Loaded into every session at ~3K tokens.
/// Gives the agent O(1) awareness of everything it knows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootIndex {
    /// Index version (incremented on each update).
    pub version: u64,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Active context entries (recent ~7 days).
    pub active_context: Vec<RootEntry>,
    /// Recent patterns observed across sessions.
    pub recent_patterns: Vec<String>,
    /// Historical summary (monthly breakdowns).
    pub historical_summary: Vec<HistoricalPeriod>,
    /// Topic index — all known topics with type and freshness.
    pub topics: Vec<TopicEntry>,
}

/// A single entry in the ROOT index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootEntry {
    /// Topic description (1 line).
    pub topic: String,
    /// Memory type classification.
    pub memory_type: MemoryType,
    /// Age in days since last access.
    pub age_days: u32,
    /// Reference to warm/cold file for drill-down.
    pub reference: String,
}

/// A historical period summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalPeriod {
    /// Period label (e.g., "2026-01~02", "2026-03").
    pub period: String,
    /// Key events/decisions in this period.
    pub summary: String,
}

/// A topic in the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicEntry {
    /// Topic name.
    pub name: String,
    /// Category: "project", "feedback", "user", "reference", "skill".
    pub category: String,
    /// Age in days.
    pub age_days: u32,
    /// Brief description.
    pub description: String,
    /// Reference path for drill-down.
    pub reference: String,
}
```

### 4.5 DreamReport

```rust
/// Report from a dream (consolidation) run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamReport {
    /// Dream start time.
    pub started_at: DateTime<Utc>,
    /// Dream end time.
    pub completed_at: DateTime<Utc>,
    /// Total entries before dream.
    pub entries_before: usize,
    /// Total entries after dream.
    pub entries_after: usize,
    /// Number of entries compacted (merged).
    pub compacted: usize,
    /// Number of entries promoted (tier upgrade).
    pub promoted: usize,
    /// Number of entries demoted (tier downgrade).
    pub demoted: usize,
    /// Number of entries deleted (expired/decayed).
    pub deleted: usize,
    /// Number of contradictions resolved.
    pub contradictions_resolved: usize,
    /// Number of duplicates merged.
    pub duplicates_merged: usize,
    /// ROOT index entries updated.
    pub root_updated: bool,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}
```

### 4.6 ConsolidationConfig

```rust
/// Memory consolidation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationConfig {
    // ── Dream Process ─────────────────────────────────
    /// Enable the dream (background consolidation) process.
    #[serde(default = "default_true")]
    pub dream_enabled: bool,
    /// Minimum hours between dream runs.
    #[serde(default = "default_dream_interval")]
    pub dream_interval_hours: u64,
    /// Minimum sessions since last dream.
    #[serde(default = "default_dream_min_sessions")]
    pub dream_min_sessions: u32,

    // ── Tier Budgets ──────────────────────────────────
    /// Maximum entries in Hot tier.
    #[serde(default = "default_hot_max")]
    pub hot_max_entries: usize,
    /// Maximum entries in Warm tier.
    #[serde(default = "default_warm_max")]
    pub warm_max_entries: usize,
    /// Maximum entries in Cold tier.
    #[serde(default = "default_cold_max")]
    pub cold_max_entries: usize,
    /// Token budget for Hot tier (injected into context).
    #[serde(default = "default_hot_token_budget")]
    pub hot_token_budget: usize,

    // ── Decay ─────────────────────────────────────────
    /// Enable Ebbinghaus-inspired decay.
    #[serde(default = "default_true")]
    pub decay_enabled: bool,
    /// Global decay multiplier (1.0 = standard, 0.5 = slower decay).
    #[serde(default = "default_one")]
    pub decay_multiplier: f32,
    /// Minimum decay score before deletion.
    #[serde(default = "default_decay_threshold")]
    pub decay_threshold: f32,
    /// Days before an unpinned, unaccessed entry is eligible for deletion.
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,

    // ── Compaction ────────────────────────────────────
    /// Maximum lines before compaction triggers (raw→daily).
    #[serde(default = "default_compaction_threshold")]
    pub compaction_line_threshold: usize,
    /// Enable LLM-based compaction (vs. simple truncation).
    #[serde(default = "default_true")]
    pub llm_compaction: bool,

    // ── Proactive Recall ──────────────────────────────
    /// Enable proactive recall (auto-inject relevant memories).
    #[serde(default = "default_true")]
    pub proactive_recall: bool,
    /// Maximum memories to proactively inject per session.
    #[serde(default = "default_proactive_limit")]
    pub proactive_recall_limit: usize,
    /// Minimum relevance score for proactive injection.
    #[serde(default = "default_proactive_threshold")]
    pub proactive_recall_threshold: f32,
}

// Default values
fn default_dream_interval() -> u64 { 24 }
fn default_dream_min_sessions() -> u32 { 5 }
fn default_hot_max() -> usize { 50 }
fn default_warm_max() -> usize { 500 }
fn default_cold_max() -> usize { 10_000 }
fn default_hot_token_budget() -> usize { 3_000 }
fn default_one() -> f32 { 1.0 }
fn default_decay_threshold() -> f32 { 0.05 }
fn default_retention_days() -> u32 { 90 }
fn default_compaction_threshold() -> usize { 200 }
fn default_proactive_limit() -> usize { 5 }
fn default_proactive_threshold() -> f32 { 0.6 }
```

---

## 5. 메모리 수명주기 (Memory Lifecycle)

### 5.1 생성 (Creation)

```rust
/// How a memory entry is created.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CreationSource {
    /// Agent explicitly stored during session.
    AgentAction,
    /// Auto-generated session summary.
    SessionSummary,
    /// Auto-generated conversation compaction.
    ConversationCompaction,
    /// Dream-time consolidation produced this.
    DreamConsolidation,
    /// User explicitly saved (via tool or UI).
    UserExplicit,
    /// Knowledge base sync produced this.
    KnowledgeSync,
    /// Imported from external system (MEMORY.md, etc.).
    ExternalImport,
}
```

모든 메모리는 다음 필드를 초기화한다:
- `tier = MemoryTier::Hot` (모든 새 메모리는 Hot에서 시작)
- `importance = memory_type.base_importance()` (타입별 기본값)
- `decay_score = 1.0` (최대 신선도)
- `compaction_level = 0` (raw)
- `content_hash = content_hash(&content)` (중복 감지용)
- `created_at = accessed_at = modified_at = Utc::now()`

### 5.2 접근 (Access)

메모리가 recall 또는 search에 의해 접근되면:
- `access_count += 1`
- `accessed_at = Utc::now()`
- `decay_score = f32::max(decay_score, recompute_decay(entry))` (접근 시 감쇠 부분 복구)

### 5.3 감쇠 (Decay)

Ebbinghaus 망각 곡선 기반 감쇠 공식:

```
R(t) = e^(-decay_rate × t_hours × decay_multiplier)

Where:
  R(t)         = retention score at time t (0.0–1.0)
  decay_rate   = per-type decay rate (see MemoryType::decay_rate)
  t_hours      = hours since last access
  decay_multiplier = global config multiplier
```

**접근 부스터 (Access Booster):**
```
effective_decay = base_decay × (1 + ln(1 + access_count))
```
자주 접근된 메모리는 감쇠가 느려진다.

**핀 고정 (Pinned Override):**
`pinned = true`인 엔트리는 절대 감쇠하지 않는다 (`decay_score = 1.0`).

```rust
impl DecayEngine {
    /// Compute current decay score for an entry.
    pub fn compute_decay(&self, entry: &MemoryEntry, now: DateTime<Utc>) -> f32 {
        if entry.pinned {
            return 1.0;
        }

        let hours_since_access = (now - entry.accessed_at).num_hours().max(0) as f32;
        let type_decay_rate = entry.memory_type.decay_rate();
        let access_boost = (1.0 + (1.0_f32 + entry.access_count as f32).ln());
        let effective_rate = type_decay_rate * self.multiplier / access_boost;

        let retention = (-effective_rate * hours_since_access).exp();
        retention.clamp(0.0, 1.0)
    }
}
```

### 5.4 압축 (Consolidation / Dream)

Dream 프로세스에 의한 압축:

1. **중복 제거**: `content_hash` 기준 + 벡터 유사도 > 0.95
2. **모순 해결**: 최신 엔트리 유지, 구버전에 `superseded_by` 마킹
3. **타입별 압축**:
   - `UserProfile`, `Preference`: 압축하지 않음 (항상 보존)
   - `Decision`: 핵심 근거만 보존
   - `Fact`, `Episode`: 중요도 기반 선택적 보존
   - `Conversation`, `Session`: 적극적 압축 (summary로 교체)
4. **계층 이동**:
   - Hot → Warm: Hot 용량 초과 시, 가장 낮은 effective_importance 엔트리 이동
   - Warm → Cold: 30일+ 미접근 시, 또는 decay_score < 0.2 시
   - Cold → Deleted: retention_days 초과 + decay_score < threshold

### 5.5 보관 (Archival)

Cold tier로 이동된 메모리:
- 압축 트리에 통합 (Raw → Daily → Weekly → Monthly → Root)
- HNSW 인덱스에서는 유지 (검색 가능)
- 원본 내용은 압축 요약으로 교체 가능

### 5.6 삭제 (Deletion)

삭제 조건 (모두 만족해야 함):
1. `retention_days` 경과
2. `decay_score < decay_threshold`
3. `pinned == false`
4. `MemoryType::UserProfile` 또는 `MemoryType::Preference`가 아님
5. 다른 메모리의 `related_ids`에 포함되지 않음 (고아만)

---

## 6. 압축 트리 (Compaction Tree)

### 6.1 구조

Hipocampus의 5-level compaction tree를 채택:

```
┌─────────────────────────────────────────────────────────┐
│                    Root (ROOT index)                      │
│  ~100 lines, ~3K tokens, always loaded                   │
│  Updated incrementally on each dream                     │
└───────────────────────────┬─────────────────────────────┘
                            │ compaction
┌───────────────────────────▼─────────────────────────────┐
│                   Monthly Summaries                       │
│  2026-01.md, 2026-02.md, 2026-03.md, ...                │
│  ~500 lines each, ~3K tokens                             │
│  One per month, created at month boundary                │
└───────────────────────────┬─────────────────────────────┘
                            │ compaction
┌───────────────────────────▼─────────────────────────────┐
│                   Weekly Summaries                        │
│  2026-W01.md, 2026-W02.md, ...                           │
│  ~300 lines each, ~2K tokens                             │
│  One per week, created at week boundary                  │
└───────────────────────────┬─────────────────────────────┘
                            │ compaction
┌───────────────────────────▼─────────────────────────────┐
│                   Daily Summaries                         │
│  2026-05-20.md, 2026-05-21.md, ...                       │
│  ~200 lines each, ~1.5K tokens                           │
│  Created at end of day or when raw log exceeds threshold │
└───────────────────────────┬─────────────────────────────┘
                            │ compaction
┌───────────────────────────▼─────────────────────────────┐
│                   Raw Session Logs                        │
│  session-{id}.json, per-session entries                  │
│  Uncompressed, verbatim records                          │
│  Compacted when daily log exceeds threshold              │
└─────────────────────────────────────────────────────────┘
```

### 6.2 Compaction Algorithm

```rust
/// Compaction level in the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompactionLevel {
    /// Raw session data (level 0).
    Raw = 0,
    /// Daily summary (level 1).
    Daily = 1,
    /// Weekly summary (level 2).
    Weekly = 2,
    /// Monthly summary (level 3).
    Monthly = 3,
    /// Root index entry (level 4).
    Root = 4,
}

impl CompactionLevel {
    /// Line threshold before compaction triggers.
    pub fn threshold(&self) -> usize {
        match self {
            CompactionLevel::Raw => 200,
            CompactionLevel::Daily => 300,
            CompactionLevel::Weekly => 500,
            CompactionLevel::Monthly => usize::MAX, // Manual or time-boundary only
            CompactionLevel::Root => usize::MAX,    // Always recompaction
        }
    }

    /// Compaction strategy.
    pub fn strategy(&self) -> CompactionStrategy {
        match self {
            CompactionLevel::Raw => CompactionStrategy::CopyBelowThreshold,
            CompactionLevel::Daily => CompactionStrategy::CopyBelowThreshold,
            CompactionLevel::Weekly => CompactionStrategy::ConcatThenSummarize,
            CompactionLevel::Monthly => CompactionStrategy::ConcatThenSummarize,
            CompactionLevel::Root => CompactionStrategy::RecursiveRecompaction,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CompactionStrategy {
    /// Below threshold: copy verbatim. Above: LLM summarize.
    CopyBelowThreshold,
    /// Concatenate entries, then LLM summarize.
    ConcatThenSummarize,
    /// Full recompaction from all children.
    RecursiveRecompaction,
}
```

### 6.3 Temporal Drill-Down

에이전트는 ROOT 인덱스에서 시작해 계층적으로 drill-down할 수 있다:

```
ROOT.md: "rate limiting [project, 14d]"
  → monthly/2026-05.md: "2026-05-12: Token bucket rate limiting decision"
    → weekly/2026-W20.md: "Discussed rate limiting strategies..."
      → daily/2026-05-12.md: "Full conversation about token bucket vs sliding window..."
        → Raw session log: Complete verbatim exchange
```

---

## 7. API 표면 (API Surface)

### 7.1 MemoryManager 확장 메서드

```rust
impl MemoryManager {
    // ── Existing (unchanged) ───────────────────────────
    pub async fn remember(&self, entry: MemoryEntry) -> Result<String>;
    pub async fn forget(&self, id: &str, memory_type: MemoryType) -> Result<bool>;
    pub async fn get(&self, id: &str, memory_type: MemoryType) -> Result<Option<MemoryEntry>>;
    pub async fn list(&self, memory_type: MemoryType, limit: usize) -> Result<Vec<MemoryEntry>>;
    pub async fn search(&self, query: &str, memory_type: Option<MemoryType>, limit: usize) -> Result<Vec<MemoryEntry>>;
    pub async fn recall(&self, query: &str) -> Result<Vec<MemoryEntry>>;

    // ── New: Tier Management ───────────────────────────
    /// Get the current ROOT index for context injection.
    pub async fn get_root_index(&self) -> Result<RootIndex>;
    /// Get memories by tier.
    pub async fn list_by_tier(&self, tier: MemoryTier, limit: usize) -> Result<Vec<MemoryEntry>>;
    /// Move an entry between tiers.
    pub async fn shift_tier(&self, id: &str, from: MemoryTier, to: MemoryTier) -> Result<()>;

    // ── New: Importance & Decay ────────────────────────
    /// Pin a memory (prevent decay and deletion).
    pub async fn pin(&self, id: &str) -> Result<()>;
    /// Unpin a memory.
    pub async fn unpin(&self, id: &str) -> Result<()>;
    /// Manually set importance for an entry.
    pub async fn set_importance(&self, id: &str, importance: f32) -> Result<()>;
    /// Recompute decay scores for all entries.
    pub async fn recompute_all_decay(&self) -> Result<usize>;
    /// Get effective importance (base × access boost × decay).
    pub fn effective_importance(entry: &MemoryEntry) -> f32;

    // ── New: Proactive Recall ──────────────────────────
    /// Proactively recall relevant memories for a new query.
    /// Unlike search, this uses ROOT index + topic matching
    /// to find connections the user didn't explicitly ask for.
    pub async fn proactive_recall(
        &self,
        query: &str,
        current_context: &[MemoryEntry],
        limit: usize,
    ) -> Result<Vec<MemoryEntry>>;

    // ── New: Dream Process ─────────────────────────────
    /// Run a dream (consolidation) cycle.
    pub async fn dream(&self) -> Result<DreamReport>;
    /// Check if dream should run (interval + session count).
    pub fn should_dream(&self, config: &ConsolidationConfig) -> bool;
    /// Spawn background dream task.
    pub fn spawn_dream_task(self: &Arc<Self>, config: ConsolidationConfig);

    // ── New: Compaction Tree ───────────────────────────
    /// Get the compaction tree node for a given level and period.
    pub async fn get_compaction_node(
        &self,
        level: CompactionLevel,
        period: &str,
    ) -> Result<Option<String>>;
    /// Drill down from a topic to its source entries.
    pub async fn drill_down(&self, topic: &str, max_depth: u8) -> Result<Vec<MemoryEntry>>;

    // ── New: Context Injection ─────────────────────────
    /// Build the Hot tier context for agent prompt injection.
    pub async fn build_hot_context(&self) -> Result<String>;
    /// Blend hot context + proactive recall into system prompt.
    pub async fn build_full_context(
        &self,
        query: &str,
        system_prompt: &str,
    ) -> Result<String>;
}
```

### 7.2 Kernel Tool 확장

새 메모리 관련 커널 도구:

```rust
// tools/kernel/memory_tool.rs (신규 또는 기존 확장)

/// Memory management tool for agents.
pub struct MemoryTool;

// Tool functions exposed to agents:
// - memory.remember(content, type, tags, importance)
// - memory.recall(query, limit)
// - memory.search(query, type, limit)
// - memory.forget(id)
// - memory.pin(id)
// - memory.unpin(id)
// - memory.list(type, tier, limit)
// - memory.dream()  // manual trigger
```

---

## 8. 설정 (Configuration)

### 8.1 config.toml 확장

```toml
[memory]
# 기존 설정
enabled = true
max_recall = 10
auto_summarize = true
capture_compaction = true
retention_days = 90
cache_enabled = true
cache_ttl_secs = 3600
cache_max_entries = 10000

# 신규: Dream 프로세스
dream_enabled = true
dream_interval_hours = 24
dream_min_sessions = 5

# 신규: Tier 예산
hot_max_entries = 50
warm_max_entries = 500
cold_max_entries = 10000
hot_token_budget = 3000

# 신규: Decay
decay_enabled = true
decay_multiplier = 1.0
decay_threshold = 0.05

# 신규: Compaction
compaction_line_threshold = 200
llm_compaction = true

# 신규: Proactive Recall
proactive_recall = true
proactive_recall_limit = 5
proactive_recall_threshold = 0.6
```

---

## 9. Dream 프로세스 상세

### 9.1 트리거 조건

Dream은 다음 조건이 **모두** 충족될 때 실행된다:
1. `dream_enabled = true`
2. 마지막 dream 이후 `dream_interval_hours` (기본 24시간) 경과
3. 마지막 dream 이후 `dream_min_sessions` (기본 5) 세션 누적
4. 백그라운드에서 실행 (활성 세션을 차단하지 않음)
5. Lock file로 동시 실행 방지

### 9.2 Phase 1: Orient (지도 구축)

```rust
async fn dream_orient(&self) -> Result<DreamState> {
    // 1. 모든 tier의 엔트리 수 확인
    let hot_count = self.count_tier(MemoryTier::Hot).await?;
    let warm_count = self.count_tier(MemoryTier::Warm).await?;
    let cold_count = self.count_tier(MemoryTier::Cold).await?;

    // 2. 현재 ROOT 인덱스 로드
    let root = self.get_root_index().await?;

    // 3. 각 타입별 엔트리 분포
    let type_distribution = self.type_distribution().await?;

    // 4. decay 점수 분포
    let decay_stats = self.decay_statistics().await?;

    Ok(DreamState {
        total_entries: hot_count + warm_count + cold_count,
        hot_count,
        warm_count,
        cold_count,
        root_version: root.version,
        type_distribution,
        decay_stats,
    })
}
```

### 9.3 Phase 2: Gather Signal (신호 수집)

```rust
async fn dream_gather_signal(&self) -> Result<Vec<MemorySignal>> {
    let mut signals = Vec::new();

    // 1. 중복 감지 (content_hash + vector similarity)
    let duplicates = self.find_duplicates().await?;
    for dup in duplicates {
        signals.push(MemorySignal::Duplicate(dup));
    }

    // 2. 모순 감지 (같은 주제, 상반된 내용)
    let contradictions = self.find_contradictions().await?;
    for c in contradictions {
        signals.push(MemorySignal::Contradiction(c));
    }

    // 3. 상대적 날짜 감지 ("yesterday", "last week" 등)
    let relative_dates = self.find_relative_dates().await?;
    for rd in relative_dates {
        signals.push(MemorySignal::RelativeDate(rd));
    }

    // 4. 만료된 참조 감지 (삭제된 파일/세션 참조)
    let stale_refs = self.find_stale_references().await?;
    for sr in stale_refs {
        signals.push(MemorySignal::StaleReference(sr));
    }

    // 5. 빈번히 접근된 패턴 (promotion 후보)
    let hot_patterns = self.find_hot_patterns(10).await?;
    for hp in hot_patterns {
        signals.push(MemorySignal::PromotionCandidate(hp));
    }

    // 6. 감쇠 임계치 이하 엔트리 (deletion 후보)
    let decayed = self.find_decayed_entries().await?;
    for d in decayed {
        signals.push(MemorySignal::DecayCandidate(d));
    }

    Ok(signals)
}
```

### 9.4 Phase 3: Consolidate (압축)

```rust
async fn dream_consolidate(&self, signals: &[MemorySignal]) -> Result<ConsolidationPlan> {
    let mut plan = ConsolidationPlan::default();

    for signal in signals {
        match signal {
            MemorySignal::Duplicate(ids) => {
                // 최신 엔트리 유지, 나머지는 compacted_from에 통합
                plan.merge.push(MergePlan {
                    keep: ids.newest(),
                    merge_from: ids.others(),
                });
            }
            MemorySignal::Contradiction(c) => {
                // 최신 정보로 교체, 구버전은 contradicted 마킹
                plan.resolve_contradiction.push(ContradictionPlan {
                    keep: c.newer_id,
                    mark_superseded: c.older_id,
                });
            }
            MemorySignal::RelativeDate(entry_id) => {
                // "yesterday" → "2026-05-24" 변환
                plan.fix_dates.push(entry_id.clone());
            }
            MemorySignal::StaleReference(entry_id) => {
                // 삭제된 파일/세션 참조 제거
                plan.remove_stale.push(entry_id.clone());
            }
            MemorySignal::PromotionCandidate(entry) => {
                // Warm → Hot 승격
                plan.promote.push(entry.id.clone());
            }
            MemorySignal::DecayCandidate(entry) => {
                // 감쇠 임계치 이하, Cold로 강등 또는 삭제
                if entry.tier == MemoryTier::Cold {
                    plan.delete.push(entry.id.clone());
                } else {
                    plan.demote.push(entry.id.clone());
                }
            }
        }
    }

    // Tier 예산 초과 시 추가 강등
    let hot_count = self.count_tier(MemoryTier::Hot).await?;
    if hot_count > self.config.hot_max_entries {
        let overflow = hot_count - self.config.hot_max_entries;
        let candidates = self.find_least_important(MemoryTier::Hot, overflow).await?;
        plan.demote.extend(candidates);
    }

    Ok(plan)
}
```

### 9.5 Phase 4: Prune & Index (정리)

```rust
async fn dream_prune_and_index(&self, plan: &ConsolidationPlan) -> Result<()> {
    // 1. 병합 실행
    for merge in &plan.merge {
        self.execute_merge(merge).await?;
    }

    // 2. 모순 해결
    for c in &plan.resolve_contradiction {
        self.execute_contradiction_resolution(c).await?;
    }

    // 3. 날짜 수정
    for id in &plan.fix_dates {
        self.fix_relative_dates(id).await?;
    }

    // 4. 스테일 참조 제거
    for id in &plan.remove_stale {
        self.remove_stale_references(id).await?;
    }

    // 5. 승격
    for id in &plan.promote {
        self.shift_tier(id, MemoryTier::Warm, MemoryTier::Hot).await?;
    }

    // 6. 강등
    for id in &plan.demote {
        let entry = self.get_by_id(id).await?;
        if let Some(e) = entry {
            let from = e.tier;
            let to = match from {
                MemoryTier::Hot => MemoryTier::Warm,
                MemoryTier::Warm => MemoryTier::Cold,
                MemoryTier::Cold => MemoryTier::Cold, // 이미 Cold
            };
            self.shift_tier(id, from, to).await?;
        }
    }

    // 7. 삭제
    for id in &plan.delete {
        let entry = self.get_by_id(id).await?;
        if let Some(e) = entry {
            self.forget(id, e.memory_type).await?;
        }
    }

    // 8. ROOT 인덱스 재구축
    self.rebuild_root_index().await?;

    // 9. HNSW 인덱스 재구축 (변경 >10% 시)
    if plan.total_changes() > self.total_entries().await / 10 {
        self.rebuild_hnsw_index_all().await?;
    }

    // 10. 압축 트리 업데이트
    self.update_compaction_tree().await?;

    Ok(())
}
```

---

## 10. Proactive Recall (능동적 회상)

### 10.1 문제

기존 `recall()`은 사용자가 명시적으로 언급한 키워드 기반이다. 사용자가 "결제 흐름 리팩토링"을 요청할 때, 3주 전에 논의한 "속도 제한" 결정이 연결되지 않는다. 이는 검색 실패가 아니라 **인지 실패**다.

### 10.2 해결: 3-Step Selective Recall

Hipocampus의 3-step selective recall을 채택:

```
Step 1: ROOT.md Triage (O(1))
  ├── Topic index에서 직접 매칭
  ├── 대부분의 쿼리를 즉시 해결
  └── 예: "결제" → ROOT에 "결제 [project, 5d]" 존재 → 관련 warm 파일 로드

Step 2: Manifest-based LLM Selection
  ├── 키워드가 직접 매칭되지 않는 교차 도메인 쿼리
  ├── Compaction tree frontmatter만 읽음 (<500 tokens)
  └── LLM이 상위 5개 관련 파일 선택

Step 3: HNSW Vector Search
  ├── 의미적 유사도 기반 검색
  ├── 키워드 오버랩이 없는 관련 메모리 발견
  └── 예: "배포" ↔ "deployment", "CI/CD" ↔ "github-actions"
```

### 10.3 구현

```rust
impl MemoryManager {
    /// Proactive recall: find relevant memories without explicit search.
    pub async fn proactive_recall(
        &self,
        query: &str,
        current_context: &[MemoryEntry],
        limit: usize,
    ) -> Result<Vec<MemoryEntry>> {
        let mut results = Vec::new();
        let mut seen_ids: HashSet<String> = current_context.iter().map(|e| e.id.clone()).collect();

        // Step 1: ROOT index triage
        let root = self.get_root_index().await?;
        for topic in &root.topics {
            if self.topic_matches_query(topic, query) {
                // Load the referenced warm/cold entry
                if let Ok(Some(entry)) = self.load_by_reference(&topic.reference).await {
                    if !seen_ids.contains(&entry.id) {
                        seen_ids.insert(entry.id.clone());
                        results.push(entry);
                    }
                }
            }
            if results.len() >= limit { break; }
        }

        // Step 2: Manifest-based selection (if Step 1 was insufficient)
        if results.len() < limit {
            let manifest_entries = self.select_by_manifest(query, limit).await?;
            for entry in manifest_entries {
                if !seen_ids.contains(&entry.id) {
                    seen_ids.insert(entry.id.clone());
                    results.push(entry);
                }
                if results.len() >= limit { break; }
            }
        }

        // Step 3: HNSW vector search (final fallback)
        if results.len() < limit {
            let remaining = limit - results.len();
            let semantic = self.search(query, None, remaining).await.unwrap_or_default();
            for entry in semantic {
                if !seen_ids.contains(&entry.id) {
                    seen_ids.insert(entry.id.clone());
                    results.push(entry);
                }
                if results.len() >= limit { break; }
            }
        }

        // Filter by proactive threshold
        results.retain(|e| {
            Self::effective_importance(e) >= self.config.proactive_recall_threshold
        });

        Ok(results)
    }

    /// Check if a topic entry matches a query (fuzzy matching).
    fn topic_matches_query(&self, topic: &TopicEntry, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        let topic_lower = topic.name.to_lowercase();
        let desc_lower = topic.description.to_lowercase();

        // Direct substring match
        if topic_lower.contains(&query_lower) || desc_lower.contains(&query_lower) {
            return true;
        }

        // Keyword overlap
        let query_terms = extract_keywords(query);
        let topic_terms = extract_keywords(&topic.name);
        let overlap = query_terms.iter()
            .filter(|t| topic_terms.iter().any(|tt| tt.contains(*t) || t.contains(tt.as_str())))
            .count();

        overlap >= 1 // At least one keyword overlap
    }
}
```

---

## 11. 마이그레이션 계획 (Migration Plan)

### 11.1 Phase 1: Data Model Extension (비파괴적)

**변경 파일:**
| 파일 | 변경 | 설명 |
|------|------|------|
| `memory/mod.rs` | 수정 | `MemoryTier`, `ConsolidationConfig`, `RootIndex` 추가 |
| `memory/store.rs` | 수정 | `shift_tier()`, `list_by_tier()`, 기존 메서드는 그대로 |
| `memory/budget.rs` | 수정 | `MemoryBudget`에 tier 기반 필드 추가 |
| `memory/graph.rs` | 유지 | PageRank 기반 중요도는 그대로 활용 |
| `memory/hnsw.rs` | 유지 | HNSW 인덱스는 그대로 활용 |
| `memory/embedding_cache.rs` | 유지 | 캐시는 그대로 |
| `memory/chunking.rs` | 유지 | 청킹은 그대로 |
| `memory/auto_memory_bridge.rs` | 수정 | `RootIndex` 인식하도록 |
| `memory/migrate.rs` | 수정 | 기존 엔트리에 `tier`, `decay_score` 필드 추가 |
| `memory/normalizer.rs` | 유지 | 정규화 유틸리티 |
| `memory/hyperbolic.rs` | 유지 | 하이퍼볼릭 임베딩 |
| `memory/flash_attention.rs` | 유지 | 어텐션 유틸리티 |
| `memory/sona.rs` | 유지 | SONA 학습 엔진 |
| `memory/rvf_store.rs` | 유지 | RVF 지속성 |
| `memory/reasoning_bank.rs` | 유지 | 추론 패턴 |
| `config.rs` | 수정 | `ConsolidationConfig` 필드 추가 |

**신규 파일:**
| 파일 | 설명 |
|------|------|
| `memory/decay.rs` | `DecayEngine` — Ebbinghaus 감쇠 계산 |
| `memory/dream.rs` | `DreamProcess` — 4-phase consolidation |
| `memory/root_index.rs` | `RootIndex` 관리 (빌드, 업데이트, 직렬화) |
| `memory/compaction.rs` | `CompactionTree` — 5-level tree 관리 |
| `memory/proactive.rs` | `ProactiveRecall` — 3-step selective recall |

**기존 엔트리 마이그레이션:**
```sql
-- 논리적 마이그레이션 (실제로는 Rust 코드):
UPDATE memory_entries SET
    tier = 'hot',             -- 모든 기존 엔트리는 Hot으로
    decay_score = 1.0,        -- 최대 신선도
    compaction_level = 0,     -- Raw
    content_hash = hash(content)
WHERE tier IS NULL;
```

Serde의 `#[serde(default)]`를 사용하므로 기존 JSON 파일은 자동 호환된다.

### 11.2 Phase 2: Dream Process (백그라운드)

- `DreamProcess` 구현체를 `kernel.rs`에서 백그라운드 태스크로 스폰
- `DreamReport`를 StateStore에 저장
- 기존 `spawn_curation_task`를 `spawn_dream_task`로 교체

### 11.3 Phase 3: Root Index & Proactive Recall

- `RootIndex` 빌드 로직 구현
- `agent_runtime.rs`에서 `build_full_context()` 호출
- 기존 `blend_into_prompt`를 `build_full_context`로 교체

### 11.4 Phase 4: Compaction Tree

- 압축 트리 파일 포맷 정의 (JSON per level)
- `drill_down()` API 구현
- Knowledge Tool에 `memory.drill_down` 추가

---

## 12. 파일 위치 (File Locations)

### 12.1 신규 파일

```
crates/oxios-kernel/src/memory/
├── decay.rs              # DecayEngine — Ebbinghaus-inspired decay
├── dream.rs              # DreamProcess — 4-phase background consolidation
├── root_index.rs         # RootIndex — always-loaded topic index
├── compaction.rs         # CompactionTree — 5-level hierarchy
└── proactive.rs          # ProactiveRecall — 3-step selective recall
```

### 12.2 기존 파일 수정

```
crates/oxios-kernel/src/memory/
├── mod.rs                # MemoryTier, MemoryType 확장, MemoryEntry 확장
├── store.rs              # tier-aware operations, root index I/O
├── budget.rs             # tier-based budget
├── auto_memory_bridge.rs # RootIndex 인식
├── migrate.rs            # 새 필드 마이그레이션

crates/oxios-kernel/src/
├── config.rs             # ConsolidationConfig 추가
├── agent_runtime.rs      # build_full_context() 사용
├── kernel.rs             # Dream process 스폰
├── tools/kernel_bridge.rs # MemoryTool 등록
```

### 12.3 데이터 파일

```
~/.oxios/workspace/spaces/{space-id}/memory/
├── root_index.json                    # ROOT 인덱스 (Tier 1)
├── conversations/                     # 대화 압축 (기존)
├── sessions/                          # 세션 요약 (기존)
├── facts/                             # 사실 (기존)
├── episodes/                          # 에피소드 (기존)
├── skills/                            # 기술 (신규)
├── preferences/                       # 선호도 (신규)
├── decisions/                         # 결정 (신규)
├── profiles/                          # 사용자 프로필 (신규)
├── compaction/                        # 압축 트리 (신규)
│   ├── daily/                         # 일별 압축 노드
│   │   └── 2026-05-20.json
│   ├── weekly/                        # 주별 압축 노드
│   │   └── 2026-W21.json
│   └── monthly/                       # 월별 압축 노드
│       └── 2026-05.json
├── dream_reports/                     # Dream 보고서 (신규)
│   └── dream-2026-05-25.json
└── vector_index_snapshot.json         # 기존
```

---

## 13. 성공 기준 (Success Criteria)

| 기준 | 측정 방법 |
|------|----------|
| Hot tier가 항상 ~3K 토큰 이내 | `build_hot_context().len()` |
| Dream이 24시간 주기로 자동 실행 | `dream_reports/` 파일 존재 확인 |
| 감쇠된 엔트리가 자동 삭제 | decay_score < threshold 엔트리 수 = 0 |
| 중복/모순 자동 해결 | DreamReport의 `duplicates_merged > 0` |
| Proactive recall이 검색 외 연결 발견 | ROOT index hit rate > 50% |
| 기존 테스트 모두 통과 | `cargo test --workspace` |
| 기존 데이터 호환 | serde default로 기존 JSON 로드 가능 |

---

## 14. 리스크 (Risks)

### 14.1 Dream이 중요한 메모리를 삭제

**완화**: `pinned` 메커니즘 + `UserProfile`/`Preference` 타입은 절대 삭제하지 않음. Dream은 read-only on code, memory files만 수정.

### 14.2 ROOT 인덱스 품질

**완화**: ROOT 인덱스는 LLM 기반 압축에 의존. 초기에는 간단한 키워드 추출로 빌드하고, 점진적으로 LLM 압축 품질을 개선.

### 14.3 성능 오버헤드

**완화**: Dream은 백그라운드에서 실행. Decay 계산은 O(1) per entry. ROOT 인덱스 재구축은 변경 >10% 시에만. HNSW 인덱스는 증분 업데이트.

### 14.4 기존 시스템과의 호환성

**완화**: 모든 새 필드는 `#[serde(default)]`로 기존 JSON과 호환. 기존 `MemoryType` 5개는 그대로 유지 + 3개 추가. 기존 API는 모두 유지.

---

## 15. 참고 문헌 (References)

1. **Claude Code Auto Dream** — Anthropic (2026). 4-stage memory consolidation for Claude Code: Orient → Gather Signal → Consolidate → Prune & Index. Triggered every 24h + 5 sessions.
2. **Hipocampus** — kevin-hs-sohn (2025). 3-tier Hot/Warm/Cold memory with 5-level compaction tree and ROOT.md topic index. MIT License.
3. **MemGPT / Letta** — UC Berkeley (2023–2025). Hierarchical memory: Core (in-context blocks), Recall (conversation history), Archival (external storage). Sleep-time compute for async consolidation.
4. **Zep** — Rasmussen et al. (2025). "Zep: A Temporal Knowledge Graph Architecture for Agent Memory." Temporal knowledge graphs with state change reasoning.
5. **Ebbinghaus Forgetting Curve** — Hermann Ebbinghaus (1885). R = e^(-t/S). Spaced repetition for memory retention.
6. **SOAR Cognitive Architecture** — Laird, Newell, Rosenbloom (CMU). Episodic/Semantic/Procedural memory integration in cognitive agents.
7. **ACT-R** — John Anderson (CMU). Declarative memory with activation-based retrieval, base-level learning equation.
8. **Sleep-time Compute** — "Sleep-time Compute: Beyond Inference Scaling at Test-time" (2025). Preprocessing context during idle periods reduces inference costs 5×.
9. **RFC-003** — Knowledge Base 독립 분리 (Oxios, 2026-05-20).
10. **RFC-004** — Knowledge System — Files.md 통합 설계 (Oxios, 2026-05-19).
11. **RFC-005** — Knowledge System — 실제 통합 설계 (Oxios, 2026-05-20).
