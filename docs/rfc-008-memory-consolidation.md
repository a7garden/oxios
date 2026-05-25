# RFC-008: Memory Consolidation — Tiered Memory with Dream-Time Compaction

> **상태:** 초안  
> **날짜:** 2026-05-25  
> **이전:** rfc-003 (Knowledge Separation), rfc-004 (Knowledge System), rfc-005 (Knowledge Integration)  
> **범위:** `crates/oxios-kernel/src/memory/`, `crates/oxios-kernel/src/config.rs`, `src/kernel.rs`

---

## 0. 설계 원칙 (Design Principles)

이 시스템은 **사용자가 메모리의 존재를 인식하지 않아도 작동**해야 한다.

| 원칙 | 의미 |
|------|------|
| **Zero Maintenance** | 사용자가 pin, 타입 지정, 중요도 설정을 할 필요 없다. 시스템이 행동 패턴에서 자동 추론한다. |
| **Automatic Protection** | 반복해서 참조된 정보, 여러 세션에 걸쳐 나타난 패턴, 사용자가 정정한 내용은 자동으로 보호된다. |
| **Optional Override** | 기본은 전자동. 사용자가 원하면 수동 편집도 가능하지만, 안 해도 된다. |
| **Progressive Importance** | 중요도는 고정값이 아니라 점진적으로 쌓인다. 한 번 언급 = 낮음, 세 번 반복 = 높음, 직접 "기억해" = 보호. |
| **Graceful Forgetting** | 잊어버리는 것도 자연스럽게. 90일 동안 한 번도 참조 안 된 건 조용히 사라진다. 사용자가 눈치채지 못하게. |

**사용자 경험:**
```
사용자: 그냥 채팅함
시스템: (백그라운드에서) 
  → 대화에서 자동으로 중요한 것 추출
  → 반복 패턴 감지해서 자동 보호
  → 모순되는 과거 기억 자동 갱신
  → 안 쓰는 기억 조용히 삭제
  → 다음 세션에 알맞은 맥락 자동 주입
사용자: "어? 이거 지난번에 얘기했잖아" → 시스템이 이미 알고 있음
```

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

**문제 6: 수동 관리 부담 (Manual Management Burden)**
현재 설계는 사용자가 직접 중요도를 평가하고, 타입을 지정하고, pin을 설정해야 한다. 이는 귀찮고, 안 하면 시스템이 제대로 작동하지 않는다.

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
| **Spaced Repetition** | Activation increases with each exposure | 접근 빈도 기반 자동 보호 |

### 1.3 목표

1. **세션 압축 (Session Compaction)**: 긴 세션을 자동 요약, 핵심 결정 보존, 모순 감지
2. **메모리 계층 (Memory Tiering)**: Short-term → Working → Long-term 3계층
3. **자동 보호 (Automatic Protection)**: 반복 참조, 다중 세션 출현, 사용자 정정 → 자동 중요도 승격
4. **자동 분류 (Automatic Classification)**: 메모리 타입을 내용과 맥락에서 자동 추론
5. **중요도 점수 (Importance Scoring)**: 접근 빈도, 최신성, 반복 패턴 기반 자동 계산
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
│  │                                                             │ │
│  │ → 사용자가 설정한 게 아님. 시스템이 자동으로 구성.          │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│  recall_for_context(query)  ← 자동 호출, 사용자 개입 없음       │
│  ├── 1. ROOT.md triage (O(1) — topic index lookup)              │
│  ├── 2. Manifest-based LLM selection (cross-domain)             │
│  ├── 3. HNSW vector search (semantic)                           │
│  └── 4. Keyword fallback (BM25-style)                           │
│                                                                   │
│  remember(entry) → Tier 1 (Hot)  ← 자동 호출                    │
│  forget(id)      → Tier downshift or deletion  ← Dream이 자동   │
│  consolidate()   → Dream process  ← 백그라운드 자동              │
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
│  │           ★ Auto-protect important patterns                  ││
│  │           ★ Auto-classify memory types                       ││
│  │           ★ Auto-promote repeated references                 ││
│  │  Phase 3: Consolidate — compress, dedupe, resolve conflicts ││
│  │  Phase 4: Prune & Index — update ROOT, remove stale entries ││
│  └──────────────────────────────────────────────────────────────┘│
│                                                                   │
│  ┌──────────────────────────────────────────────────────────────┐│
│  │              Supporting Systems                              ││
│  │                                                              ││
│  │  • ImportanceScorer — access freq × recency × repetition    ││
│  │  • DecayEngine — Ebbinghaus-inspired forgetting curve        ││
│  │  • AutoProtector — pattern-based automatic pinning           ││
│  │  • AutoClassifier — content-based type inference             ││
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
                    │  (사용자 모름, 시스템이 자동 생성)      │
                    ▼                                        │
            ┌───────────────┐                               │
            │  Tier 1: Hot  │ ◄─── remember() 자동 호출     │
            │  (always in   │                                │
            │   context)    │ ──── access() → 통계 자동 업데이트│
            └───────┬───────┘                                │
                    │                                        │
          capacity? │ over budget (Dream이 자동 처리)         │
                    ▼                                        │
            ┌───────────────┐                               │
            │  Tier 2: Warm │ ◄─── shift_down() 자동        │
            │  (on-demand)  │                                │
            └───────┬───────┘                                │
                    │                                        │
          decay?    │ importance < threshold (Dream이 자동)   │
                    ▼                                        │
            ┌───────────────┐                               │
            │  Tier 3: Cold │ ◄─── archive() 자동           │
            │  (compressed) │                                │
            └───────┬───────┘                                │
                    │                                        │
          expired?  │ past retention + below min importance  │
                    ▼                                        │
               [DELETED] ──── 자동 삭제, 사용자 모름 ────────┘
```

### 2.3 Dream Process 흐름

```
Idle (min 24h since last dream, min 5 sessions since last dream)
  │
  │  ★ 트리거도 자동. 사용자가 뭘 해야 하는 게 아님.
  ▼
Phase 1: Orient ─── Scan all tiers, build current state map
  │
  ▼
Phase 2: Gather Signal ─── 자동 분석:
  │  • 반복 참조된 메모리 → auto-protect 후보
  │  • 여러 세션에 출현한 패턴 → auto-promote
  │  • 사용자 정정 → 기존 메모리 자동 갱신
  │  • 중복 감지
  │  • 모순 감지
  │  • 상대적 날짜 → 절대 날짜 변환
  │  • 메모리 타입 자동 분류 (명시적 타입 없으면)
  ▼
Phase 3: Consolidate ─── 자동 처리:
  │  • 중요도 재계산 (접근 패턴 기반)
  │  • auto-protect 승격
  │  • 타입 자동 분류 적용
  │  • 중복 병합
  │  • 모순 해결 (최신 유지)
  │  • 압축
  │  • 계층 이동
  ▼
Phase 4: Prune & Index ─── 자동 정리:
  │  • ROOT 인덱스 재구축
  │  • 감쇠 임계치 이하 삭제
  │  • 압축 트리 업데이트
  │  • DreamReport 저장
  ▼
[Complete] ─── Resume idle. 사용자는 아무것도 모름.
```

---

## 3. 자동 보호 시스템 (Auto-Protection System)

이것이 이 RFC의 핵심 차별화 요소다. 사용자가 pin을 직접 설정할 필요가 없다.

### 3.1 보호 등급 (Protection Tiers)

메모리는 사용자 개입 없이 행동 패턴에 따라 자동으로 보호 등급이 결정된다:

```rust
/// 자동 보호 등급. 사용자가 아는 필요 없음.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ProtectionLevel {
    /// 보호 없음. 감쇠 + 삭제 가능.
    None = 0,
    /// 감쇠는 느리지만 삭제는 가능.
    /// 트리거: 2회 이상 참조
    Low = 1,
    /// 감쇠 매우 느림. 삭제는 retention_days 경과 후에만.
    /// 트리거: 3회+ 참조 또는 2개 세션에서 출현
    Medium = 2,
    /// 사실상 삭제 불가. LLM 압축에서도 보존.
    /// 트리거: 5회+ 참조, 3개+ 세션, 또는 사용자가 "기억해" 언급
    High = 3,
    /// 절대 삭제 + 절대 압축 안 함.
    /// 트리거: UserProfile/Preference 타입, 또는 사용자가 명시적으로 pin
    Permanent = 4,
}
```

### 3.2 자동 승격 규칙 (Auto-Promotion Rules)

Dream 프로세스의 Phase 2에서 자동으로 평가:

```rust
/// 자동 보호 결정 로직. Dream이 매 실행마다 호출.
fn compute_protection(entry: &MemoryEntry, stats: &AccessStats) -> ProtectionLevel {
    // 1. 타입 기반 기본 보호
    match entry.memory_type {
        MemoryType::UserProfile | MemoryType::Preference => return ProtectionLevel::Permanent,
        _ => {}
    }

    // 2. 명시적 pin (사용자가 한 경우)
    if entry.pinned { return ProtectionLevel::Permanent; }

    // 3. 접근 패턴 기반 자동 승격
    let access_count = entry.access_count;
    let session_span = stats.sessions_with_access(&entry.id);
    let days_since_creation = stats.days_since(entry.created_at);
    let has_user_correction = stats.was_corrected(&entry.id);

    // 사용자가 정정한 내용은 높은 보호
    // (사용자가 "아니야, 그게 아니라 이거야"라고 한 경우)
    if has_user_correction {
        return ProtectionLevel::High;
    }

    // 5회+ 참조 또는 3개+ 세션에 출현 = High
    if access_count >= 5 || session_span >= 3 {
        return ProtectionLevel::High;
    }

    // 3회+ 참조 또는 2개+ 세션 = Medium
    if access_count >= 3 || session_span >= 2 {
        return ProtectionLevel::Medium;
    }

    // 2회+ 참조 = Low
    if access_count >= 2 {
        return ProtectionLevel::Low;
    }

    // 나머지 = None (정상 감쇠 + 삭제 가능)
    ProtectionLevel::None
}
```

### 3.3 보호 등급별 효과

| 등급 | 감쇠 속도 | 삭제 조건 | 압축 | 계층 이동 |
|------|----------|----------|------|----------|
| None | 정상 (타입별 decay_rate) | retention_days + decay < threshold | 적극적 | 자유롭게 |
| Low | ×0.5 (절반 속도) | retention_days 경과 후만 | 일반적 | 하위만 |
| Medium | ×0.2 | retention_days × 2 경과 후만 | 보존적 | 하위만 |
| High | ×0.05 (거의 안 감소) | 삭제 안 함 | 최소한만 | Dream 판단 |
| Permanent | 0 (변화 없음) | 절대 안 함 | 절대 안 함 | 절대 안 함 |

### 3.4 감쇠 공식 (보호 등급 반영)

```
effective_decay_rate = base_decay_rate
                     × protection_decay_multiplier   ← 보호 등급
                     × (1 + ln(1 + access_count))    ← 접근 부스트
                     × global_multiplier              ← 설정

R(t) = e^(-effective_decay_rate × t_hours)

Where:
  protection_decay_multiplier:
    None     = 1.0
    Low      = 0.5
    Medium   = 0.2
    High     = 0.05
    Permanent = 0.0  (항상 R = 1.0)
```

---

## 4. 자동 분류 시스템 (Auto-Classification System)

### 4.1 문제

메모리를 저장할 때 타입(Fact, Decision, Preference 등)을 지정해야 하면,
그것도 사용자 부담이다. 시스템이 내용에서 타입을 추론해야 한다.

### 4.2 분류 규칙

```rust
/// 내용 기반 자동 타입 분류.
/// 명시적 타입이 지정되지 않은 경우에만 사용.
fn infer_memory_type(content: &str, context: &str) -> MemoryType {
    let content_lower = content.to_lowercase();

    // 1. 사용자 정반 (correction) 감지
    // "아니야", "그게 아니라", "actually", "no, it's"
    if is_correction(&content_lower) {
        return MemoryType::Fact;
    }

    // 2. 선호도/취향 감지
    // "나는 ~ 좋아해", "항상 ~로 해", "prefer", "always use"
    if is_preference_statement(&content_lower) {
        return MemoryType::Preference;
    }

    // 3. 결정 감지
    // "~하기로 했어", "선택했어", "decided", "chose", "let's go with"
    if is_decision_statement(&content_lower) {
        return MemoryType::Decision;
    }

    // 4. 절차/패턴 감지
    // "항상 ~ 한다", "每次", "always run", "before commit"
    if is_skill_statement(&content_lower) {
        return MemoryType::Skill;
    }

    // 5. 프로필 정보 감지
    // 사용자 이름, 역할, 소속, 전문 분야
    if is_profile_information(&content_lower, context) {
        return MemoryType::UserProfile;
    }

    // 6. 이벤트/경험 감지
    // 날짜 + 동작 조합, "~했음", "deployed", "released"
    if is_episode(&content_lower) {
        return MemoryType::Episode;
    }

    // 7. 기본값: Fact
    MemoryType::Fact
}
```

### 4.3 분류 힌트 (Classifier Hints)

Dream Phase 2에서 재분류 가능:

```rust
// Dream이 발견하는 패턴:
// - "cargo test"가 5번 언급됨 → Fact에서 Skill로 승격
// - "한국어로 해"가 3번 반복됨 → Fact에서 Preference로 승격
// - "포트는 3000"이 한 번 나옴 → Fact 유지
//
// 타입 승격은 중요도를 올리므로, 
// 잘못 승격하면 안 되는 것도 보호됨 (false positive가 false negative보다 안전)
```

---

## 5. 데이터 구조 (Rust Structs)

### 5.1 MemoryTier

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

### 5.2 MemoryType (확장)

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
            MemoryType::UserProfile => 0.95,
            MemoryType::Preference => 0.90,
            MemoryType::Decision => 0.80,
            MemoryType::Skill => 0.75,
            MemoryType::Fact => 0.60,
            MemoryType::Episode => 0.50,
            MemoryType::Session => 0.40,
            MemoryType::Conversation => 0.35,
        }
    }

    /// Base decay rate for each type.
    pub fn base_decay_rate(&self) -> f32 {
        match self {
            MemoryType::UserProfile => 0.001,
            MemoryType::Preference => 0.002,
            MemoryType::Decision => 0.005,
            MemoryType::Skill => 0.008,
            MemoryType::Fact => 0.015,
            MemoryType::Episode => 0.025,
            MemoryType::Session => 0.040,
            MemoryType::Conversation => 0.060,
        }
    }

    /// Whether this type is automatically protected from deletion.
    pub fn is_auto_protected(&self) -> bool {
        matches!(self, MemoryType::UserProfile | MemoryType::Preference)
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

### 5.3 MemoryEntry (확장)

```rust
/// A single memory entry — extended with lifecycle + auto-protection metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    // ── Identity ──────────────────────────────────────
    /// Unique ID.
    pub id: String,
    /// Memory type (auto-classified if not explicitly set).
    pub memory_type: MemoryType,
    /// Current tier (auto-managed by Dream).
    #[serde(default = "default_tier")]
    pub tier: MemoryTier,

    // ── Content ───────────────────────────────────────
    /// Content (Markdown).
    pub content: String,
    /// Content hash for deduplication.
    #[serde(default)]
    pub content_hash: u64,
    /// Tags (auto-extracted from content).
    #[serde(default)]
    pub tags: Vec<String>,

    // ── Source ────────────────────────────────────────
    /// Creator (agent name, "compaction", "system", "dream", "auto-classify").
    pub source: String,
    /// Related session ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Related space ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,

    // ── Importance ────────────────────────────────────
    /// Base importance (0.0–1.0), set by type or auto-computed.
    #[serde(default = "default_importance")]
    pub importance: f32,
    /// Whether user explicitly pinned (optional override).
    #[serde(default)]
    pub pinned: bool,

    // ── Auto-Protection ───────────────────────────────
    /// Auto-computed protection level. Dream이 매 실행마다 재계산.
    #[serde(default)]
    pub protection: ProtectionLevel,
    /// Whether the type was auto-classified (vs explicit).
    #[serde(default)]
    pub auto_classified: bool,
    /// Number of distinct sessions this entry was accessed in.
    #[serde(default)]
    pub session_appearances: u32,
    /// Whether the user has corrected/contradicted this entry's topic.
    #[serde(default)]
    pub user_corrected: bool,

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

### 5.4 RootIndex

```rust
/// ROOT index — the "table of contents" for all agent knowledge.
/// 에이전트가 자신이 아는 모든 것을 O(1)에 파악하는 인덱스.
/// 사용자가 설정하는 게 아니라 Dream이 자동 구성.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootIndex {
    /// Index version (incremented on each dream).
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootEntry {
    pub topic: String,
    pub memory_type: MemoryType,
    pub protection: ProtectionLevel,
    pub age_days: u32,
    pub reference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalPeriod {
    pub period: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicEntry {
    pub name: String,
    pub category: String,
    pub age_days: u32,
    pub description: String,
    pub reference: String,
}
```

### 5.5 DreamReport

```rust
/// Report from a dream (consolidation) run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamReport {
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub entries_before: usize,
    pub entries_after: usize,
    pub compacted: usize,
    pub promoted: usize,
    pub demoted: usize,
    pub deleted: usize,
    pub contradictions_resolved: usize,
    pub duplicates_merged: usize,
    pub auto_protected: usize,       // NEW: 자동 보호된 항목 수
    pub auto_classified: usize,      // NEW: 자동 분류된 항목 수
    pub type_promotions: usize,      // NEW: 타입 승격 수 (Fact→Decision 등)
    pub root_updated: bool,
    pub duration_ms: u64,
}
```

### 5.6 ConsolidationConfig

```rust
/// Memory consolidation configuration.
/// 모든 값에 합리적인 기본값이 있어서 사용자가 설정 안 해도 됨.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationConfig {
    // ── Dream Process ─────────────────────────────────
    #[serde(default = "default_true")]
    pub dream_enabled: bool,
    #[serde(default = "default_dream_interval")]
    pub dream_interval_hours: u64,
    #[serde(default = "default_dream_min_sessions")]
    pub dream_min_sessions: u32,

    // ── Tier Budgets ──────────────────────────────────
    #[serde(default = "default_hot_max")]
    pub hot_max_entries: usize,
    #[serde(default = "default_warm_max")]
    pub warm_max_entries: usize,
    #[serde(default = "default_cold_max")]
    pub cold_max_entries: usize,
    #[serde(default = "default_hot_token_budget")]
    pub hot_token_budget: usize,

    // ── Decay ─────────────────────────────────────────
    #[serde(default = "default_true")]
    pub decay_enabled: bool,
    #[serde(default = "default_one")]
    pub decay_multiplier: f32,
    #[serde(default = "default_decay_threshold")]
    pub decay_threshold: f32,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,

    // ── Auto-Protection ───────────────────────────────
    /// Enable auto-protection based on access patterns.
    #[serde(default = "default_true")]
    pub auto_protection: bool,
    /// Minimum access count for Low protection.
    #[serde(default = "default_protection_low_threshold")]
    pub protection_low_access: u32,
    /// Minimum access count for Medium protection.
    #[serde(default = "default_protection_medium_threshold")]
    pub protection_medium_access: u32,
    /// Minimum access count for High protection.
    #[serde(default = "default_protection_high_threshold")]
    pub protection_high_access: u32,
    /// Minimum session appearances for Medium protection.
    #[serde(default = "default_protection_medium_sessions")]
    pub protection_medium_sessions: u32,
    /// Minimum session appearances for High protection.
    #[serde(default = "default_protection_high_sessions")]
    pub protection_high_sessions: u32,

    // ── Auto-Classification ───────────────────────────
    /// Enable auto type classification.
    #[serde(default = "default_true")]
    pub auto_classification: bool,
    /// Minimum repetitions before type promotion (e.g., Fact → Skill).
    #[serde(default = "default_type_promotion_threshold")]
    pub type_promotion_repetitions: u32,

    // ── Compaction ────────────────────────────────────
    #[serde(default = "default_compaction_threshold")]
    pub compaction_line_threshold: usize,
    #[serde(default = "default_true")]
    pub llm_compaction: bool,

    // ── Proactive Recall ──────────────────────────────
    #[serde(default = "default_true")]
    pub proactive_recall: bool,
    #[serde(default = "default_proactive_limit")]
    pub proactive_recall_limit: usize,
    #[serde(default = "default_proactive_threshold")]
    pub proactive_recall_threshold: f32,
}

// Defaults
fn default_dream_interval() -> u64 { 24 }
fn default_dream_min_sessions() -> u32 { 5 }
fn default_hot_max() -> usize { 50 }
fn default_warm_max() -> usize { 500 }
fn default_cold_max() -> usize { 10_000 }
fn default_hot_token_budget() -> usize { 3_000 }
fn default_one() -> f32 { 1.0 }
fn default_decay_threshold() -> f32 { 0.05 }
fn default_retention_days() -> u32 { 90 }
fn default_protection_low_access() -> u32 { 2 }
fn default_protection_medium_access() -> u32 { 3 }
fn default_protection_high_access() -> u32 { 5 }
fn default_protection_medium_sessions() -> u32 { 2 }
fn default_protection_high_sessions() -> u32 { 3 }
fn default_type_promotion_repetitions() -> u32 { 3 }
fn default_compaction_threshold() -> usize { 200 }
fn default_proactive_limit() -> usize { 5 }
fn default_proactive_threshold() -> f32 { 0.6 }
```

---

## 6. 메모리 수명주기 (Memory Lifecycle)

### 6.1 생성 (Creation) — 완전 자동

```rust
/// How a memory entry is created.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CreationSource {
    /// Agent automatically stored during session.
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
    /// Imported from external system.
    ExternalImport,
}
```

모든 메모리는 자동으로 초기화된다 (사용자 개입 없음):
- `tier = MemoryTier::Hot` — 모든 새 메모리는 Hot에서 시작
- `memory_type = infer_memory_type(content, context)` — 자동 분류
- `importance = memory_type.base_importance()` — 타입별 기본값
- `protection = ProtectionLevel::None` — 초기에는 보호 없음
- `decay_score = 1.0` — 최대 신선도
- `auto_classified = true` — 자동 분류 마크
- `content_hash = content_hash(&content)` — 중복 감지용
- `tags = extract_tags(&content)` — 자동 태그 추출
- `created_at = accessed_at = modified_at = Utc::now()`

### 6.2 접근 (Access) — 자동 추적

메모리가 recall 또는 search에 의해 접근되면 (자동):
- `access_count += 1`
- `accessed_at = Utc::now()`
- `session_appearances += 1` (새 세션인 경우만)
- `decay_score = f32::max(decay_score, recompute_decay(entry))` — 접근 시 감쇠 부분 복구

### 6.3 감쇠 (Decay) — 자동 계산

Ebbinghaus 망각 곡선 기반 + 보호 등급 반영:

```rust
impl DecayEngine {
    /// Compute current decay score for an entry.
    /// 보호 등급이 높을수록 감쇠가 느려짐.
    pub fn compute_decay(&self, entry: &MemoryEntry, now: DateTime<Utc>) -> f32 {
        // Permanent 보호 = 항상 1.0
        if entry.pinned || entry.protection == ProtectionLevel::Permanent {
            return 1.0;
        }

        let hours_since_access = (now - entry.accessed_at).num_hours().max(0) as f32;
        let base_rate = entry.memory_type.base_decay_rate();

        // 접근 부스트: 자주 읽힌 메모리는 감쇠가 느려짐
        let access_boost = 1.0 + (1.0_f32 + entry.access_count as f32).ln();

        // 보호 등급 감쇠 배율
        let protection_mult = match entry.protection {
            ProtectionLevel::None => 1.0,
            ProtectionLevel::Low => 0.5,
            ProtectionLevel::Medium => 0.2,
            ProtectionLevel::High => 0.05,
            ProtectionLevel::Permanent => 0.0,
        };

        let effective_rate = base_rate * self.multiplier * protection_mult / access_boost;
        let retention = (-effective_rate * hours_since_access).exp();
        retention.clamp(0.0, 1.0)
    }
}
```

### 6.4 자동 보호 재계산 — Dream이 자동

```rust
/// Dream Phase 2: 매 실행마다 모든 엔트리의 보호 등급 재계산.
async fn dream_recompute_protection(&self) -> Result<Vec<ProtectionChange>> {
    let mut changes = Vec::new();
    let entries = self.list_all().await?;

    for entry in &entries {
        let old_protection = entry.protection;
        let new_protection = compute_protection(entry, &self.access_stats);

        if old_protection != new_protection {
            changes.push(ProtectionChange {
                id: entry.id.clone(),
                from: old_protection,
                to: new_protection,
                reason: format!(
                    "access_count={}, sessions={}, corrected={}",
                    entry.access_count, entry.session_appearances, entry.user_corrected
                ),
            });
            // 업데이트 적용
            self.update_protection(&entry.id, new_protection).await?;
        }
    }

    Ok(changes)
}
```

### 6.5 삭제 (Deletion) — 안전한 자동 삭제

삭제 조건 (모두 만족해야 함):
1. `protection == ProtectionLevel::None` 또는 `Low`
2. `retention_days` 경과 (Low면 ×2)
3. `decay_score < decay_threshold`
4. `pinned == false`
5. `MemoryType::UserProfile` 또는 `MemoryType::Preference`가 아님
6. 고아 엔트리 (다른 메모리의 `related_ids`에 포함되지 않음)
7. 다른 메모리의 `compacted_from`에 소스로 포함되지 않음

---

## 7. 압축 트리 (Compaction Tree)

### 7.1 구조

```
┌─────────────────────────────────────────────────────────┐
│                    Root (ROOT index)                      │
│  ~100 lines, ~3K tokens, always loaded                   │
│  Dream이 자동 구성. 사용자는 모름.                        │
└───────────────────────────┬─────────────────────────────┘
                            │ compaction (Dream이 자동)
┌───────────────────────────▼─────────────────────────────┐
│                   Monthly Summaries                       │
│  2026-01.md, 2026-02.md, ...                             │
└───────────────────────────┬─────────────────────────────┘
                            │ compaction
┌───────────────────────────▼─────────────────────────────┐
│                   Weekly Summaries                        │
│  2026-W01.md, 2026-W02.md, ...                           │
└───────────────────────────┬─────────────────────────────┘
                            │ compaction
┌───────────────────────────▼─────────────────────────────┐
│                   Daily Summaries                         │
│  2026-05-20.md, 2026-05-21.md, ...                       │
└───────────────────────────┬─────────────────────────────┘
                            │ compaction
┌───────────────────────────▼─────────────────────────────┐
│                   Raw Session Logs                        │
│  session-{id}.json, per-session entries                  │
└─────────────────────────────────────────────────────────┘
```

### 7.2 Compaction 규칙

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompactionLevel {
    Raw = 0,
    Daily = 1,
    Weekly = 2,
    Monthly = 3,
    Root = 4,
}

impl CompactionLevel {
    pub fn threshold(&self) -> usize {
        match self {
            CompactionLevel::Raw => 200,
            CompactionLevel::Daily => 300,
            CompactionLevel::Weekly => 500,
            CompactionLevel::Monthly => usize::MAX,
            CompactionLevel::Root => usize::MAX,
        }
    }
}
```

### 7.3 압축 시 보호 고려

```rust
/// 압축할 때 보호 등급이 높은 엔트리는 보존.
fn should_compact(entry: &MemoryEntry) -> bool {
    match entry.protection {
        ProtectionLevel::Permanent => false, // 절대 압축 안 함
        ProtectionLevel::High => false,      // 원문 보존
        ProtectionLevel::Medium => true,     // 압축하되 키워드 밀도 유지
        ProtectionLevel::Low => true,        // 일반 압축
        ProtectionLevel::None => true,       // 적극 압축
    }
}
```

---

## 8. API 표면 (API Surface)

### 8.1 MemoryManager 확장 메서드

```rust
impl MemoryManager {
    // ── Existing (unchanged) ───────────────────────────
    pub async fn remember(&self, entry: MemoryEntry) -> Result<String>;
    pub async fn forget(&self, id: &str, memory_type: MemoryType) -> Result<bool>;
    pub async fn get(&self, id: &str, memory_type: MemoryType) -> Result<Option<MemoryEntry>>;
    pub async fn list(&self, memory_type: MemoryType, limit: usize) -> Result<Vec<MemoryEntry>>;
    pub async fn search(&self, query: &str, memory_type: Option<MemoryType>, limit: usize) -> Result<Vec<MemoryEntry>>;
    pub async fn recall(&self, query: &str) -> Result<Vec<MemoryEntry>>;

    // ── New: Tier Management (auto, but overridable) ──
    /// Get the current ROOT index for context injection.
    pub async fn get_root_index(&self) -> Result<RootIndex>;
    /// Get memories by tier.
    pub async fn list_by_tier(&self, tier: MemoryTier, limit: usize) -> Result<Vec<MemoryEntry>>;
    /// Move an entry between tiers.
    pub async fn shift_tier(&self, id: &str, from: MemoryTier, to: MemoryTier) -> Result<()>;

    // ── New: Protection (auto, but overridable) ────────
    /// Pin a memory (user explicit override — Permanent protection).
    pub async fn pin(&self, id: &str) -> Result<()>;
    /// Unpin a memory (reverts to auto-computed protection).
    pub async fn unpin(&self, id: &str) -> Result<()>;
    /// Manually set importance for an entry (user override).
    pub async fn set_importance(&self, id: &str, importance: f32) -> Result<()>;
    /// Recompute decay scores for all entries (Dream calls this).
    pub async fn recompute_all_decay(&self) -> Result<usize>;
    /// Effective importance (base × access boost × decay × protection).
    pub fn effective_importance(entry: &MemoryEntry) -> f32;

    // ── New: Proactive Recall (auto) ───────────────────
    pub async fn proactive_recall(
        &self,
        query: &str,
        current_context: &[MemoryEntry],
        limit: usize,
    ) -> Result<Vec<MemoryEntry>>;

    // ── New: Dream Process (auto, manually triggerable) ─
    pub async fn dream(&self) -> Result<DreamReport>;
    pub fn should_dream(&self, config: &ConsolidationConfig) -> bool;
    pub fn spawn_dream_task(self: &Arc<Self>, config: ConsolidationConfig);

    // ── New: Compaction Tree (auto) ────────────────────
    pub async fn get_compaction_node(
        &self,
        level: CompactionLevel,
        period: &str,
    ) -> Result<Option<String>>;
    pub async fn drill_down(&self, topic: &str, max_depth: u8) -> Result<Vec<MemoryEntry>>;

    // ── New: Context Injection (auto) ──────────────────
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

---

## 9. 설정 (Configuration)

모든 값에 합리적인 기본값이 있어서 **사용자가 아무것도 설정 안 해도 됨**:

```toml
[memory]
# 기존 설정 (그대로)
enabled = true
max_recall = 10
auto_summarize = true

# ── 아래부터 전부 기본값으로 작동. 설정 안 해도 됨 ──

# Dream 프로세스
dream_enabled = true
dream_interval_hours = 24
dream_min_sessions = 5

# Tier 예산
hot_max_entries = 50
warm_max_entries = 500
cold_max_entries = 10000
hot_token_budget = 3000

# Decay
decay_enabled = true
decay_multiplier = 1.0
decay_threshold = 0.05
retention_days = 90

# Auto-Protection (자동 보호)
auto_protection = true
protection_low_access = 2          # 2회 참조 → Low 보호
protection_medium_access = 3       # 3회 참조 → Medium 보호
protection_high_access = 5         # 5회 참조 → High 보호
protection_medium_sessions = 2     # 2개 세션 출현 → Medium
protection_high_sessions = 3       # 3개 세션 출현 → High

# Auto-Classification (자동 분류)
auto_classification = true
type_promotion_repetitions = 3     # 3회 반복 → 타입 승격

# Compaction
compaction_line_threshold = 200
llm_compaction = true

# Proactive Recall
proactive_recall = true
proactive_recall_limit = 5
proactive_recall_threshold = 0.6
```

---

## 10. Dream 프로세스 상세

### 10.1 트리거 조건 (자동)

Dream은 다음 조건이 **모두** 충족될 때 자동 실행된다:
1. `dream_enabled = true` (기본값)
2. 마지막 dream 이후 24시간 경과
3. 마지막 dream 이후 5세션 누적
4. 백그라운드 실행 (활성 세션 차단 없음)
5. Lock file로 동시 실행 방지

### 10.2 Phase 1: Orient (지도 구축)

```rust
async fn dream_orient(&self) -> Result<DreamState> {
    let hot_count = self.count_tier(MemoryTier::Hot).await?;
    let warm_count = self.count_tier(MemoryTier::Warm).await?;
    let cold_count = self.count_tier(MemoryTier::Cold).await?;
    let root = self.get_root_index().await?;
    let type_distribution = self.type_distribution().await?;
    let decay_stats = self.decay_statistics().await?;
    let protection_distribution = self.protection_statistics().await?;

    Ok(DreamState {
        total_entries: hot_count + warm_count + cold_count,
        hot_count,
        warm_count,
        cold_count,
        root_version: root.version,
        type_distribution,
        decay_stats,
        protection_distribution,
    })
}
```

### 10.3 Phase 2: Gather Signal (신호 수집 + 자동 보호/분류)

```rust
async fn dream_gather_signal(&self) -> Result<Vec<MemorySignal>> {
    let mut signals = Vec::new();

    // ── 기존 시그널 ──
    // 중복, 모순, 상대적 날짜, 만료 참조

    // ── 자동 보호 재계산 ──
    let protection_changes = self.dream_recompute_protection().await?;
    for change in protection_changes {
        signals.push(MemorySignal::ProtectionChanged(change));
    }

    // ── 자동 분류 ──
    let untyped = self.find_auto_classifiable_entries().await?;
    for entry in untyped {
        let inferred = infer_memory_type(&entry.content, "");
        signals.push(MemorySignal::AutoClassify {
            id: entry.id.clone(),
            new_type: inferred,
        });
    }

    // ── 타입 승격 ──
    // 반복 패턴 감지: "cargo test"가 3번+ 언급 → Fact → Skill 승격
    let promotion_candidates = self.find_type_promotion_candidates().await?;
    for promo in promotion_candidates {
        signals.push(MemorySignal::TypePromotion(promo));
    }

    // ── 빈번히 접근된 패턴 (Hot 승격 후보) ──
    let hot_patterns = self.find_hot_patterns(10).await?;
    for hp in hot_patterns {
        signals.push(MemorySignal::PromotionCandidate(hp));
    }

    // ── 감쇠 임계치 이하 (삭제 후보) ──
    let decayed = self.find_decayed_entries().await?;
    for d in decayed {
        signals.push(MemorySignal::DecayCandidate(d));
    }

    Ok(signals)
}
```

### 10.4 Phase 3: Consolidate (압축)

```rust
async fn dream_consolidate(&self, signals: &[MemorySignal]) -> Result<ConsolidationPlan> {
    let mut plan = ConsolidationPlan::default();

    for signal in signals {
        match signal {
            // 기존: 중복, 모순, 날짜 수정, 스테일 참조...

            MemorySignal::ProtectionChanged(change) => {
                // 보호 등급 변경 사항을 플랜에 반영
                plan.protection_updates.push(change.clone());
            }

            MemorySignal::AutoClassify { id, new_type } => {
                // 자동 분류 적용
                plan.reclassify.push(ReclassifyPlan {
                    id: id.clone(),
                    new_type: *new_type,
                });
            }

            MemorySignal::TypePromotion(promo) => {
                // 타입 승격 (Fact → Skill, Fact → Decision 등)
                plan.reclassify.push(ReclassifyPlan {
                    id: promo.id.clone(),
                    new_type: promo.suggested_type,
                });
            }

            // ... 나머지 기존 로직 ...
        }
    }

    // Tier 예산 초과 시 추가 강등
    let hot_count = self.count_tier(MemoryTier::Hot).await?;
    if hot_count > self.config.hot_max_entries {
        let overflow = hot_count - self.config.hot_max_entries;
        // 보호 등급이 높은 건 강등에서 제외
        let candidates = self.find_demotable(MemoryTier::Hot, overflow).await?;
        plan.demote.extend(candidates);
    }

    Ok(plan)
}
```

### 10.5 Phase 4: Prune & Index (정리)

```rust
async fn dream_prune_and_index(&self, plan: &ConsolidationPlan) -> Result<()> {
    // 1-6. 기존: 병합, 모순 해결, 날짜 수정, 스테일 제거, 승격, 강등

    // 7. 보호 등급 업데이트
    for change in &plan.protection_updates {
        self.update_protection(&change.id, change.to).await?;
    }

    // 8. 자동 분류 적용
    for reclassify in &plan.reclassify {
        self.update_memory_type(&reclassify.id, reclassify.new_type).await?;
    }

    // 9. 삭제 (보호 등급 확인)
    for id in &plan.delete {
        let entry = self.get_by_id(id).await?;
        if let Some(e) = entry {
            // 삭제 전 보호 등급 재확인
            if e.protection <= ProtectionLevel::Low {
                self.forget(id, e.memory_type).await?;
            }
        }
    }

    // 10-12. 기존: ROOT 재구축, HNSW 재구축, 압축 트리 업데이트
    self.rebuild_root_index().await?;
    if plan.total_changes() > self.total_entries().await / 10 {
        self.rebuild_hnsw_index_all().await?;
    }
    self.update_compaction_tree().await?;

    Ok(())
}
```

---

## 11. Proactive Recall (능동적 회상)

### 11.1 3-Step Selective Recall

```
Step 1: ROOT.md Triage (O(1))
  ├── Topic index에서 직접 매칭
  └── 예: "결제" → ROOT에 "결제 [project, 5d]" 존재 → 관련 warm 파일 로드

Step 2: Manifest-based LLM Selection
  ├── 키워드가 직접 매칭되지 않는 교차 도메인 쿼리
  └── LLM이 상위 5개 관련 파일 선택

Step 3: HNSW Vector Search
  ├── 의미적 유사도 기반 검색
  └── 예: "배포" ↔ "deployment", "CI/CD" ↔ "github-actions"
```

### 11.2 구현

```rust
impl MemoryManager {
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
                if let Ok(Some(entry)) = self.load_by_reference(&topic.reference).await {
                    if !seen_ids.contains(&entry.id) {
                        seen_ids.insert(entry.id.clone());
                        results.push(entry);
                    }
                }
            }
            if results.len() >= limit { break; }
        }

        // Step 2: Manifest-based selection
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

        // Step 3: HNSW vector search
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

        results.retain(|e| Self::effective_importance(e) >= self.config.proactive_recall_threshold);
        Ok(results)
    }
}
```

---

## 12. 마이그레이션 계획 (Migration Plan)

### 12.1 Phase 1: Data Model Extension (비파괴적)

**신규 파일:**
| 파일 | 설명 |
|------|------|
| `memory/decay.rs` | `DecayEngine` — Ebbinghaus 감쇠 + 보호 등급 반영 |
| `memory/dream.rs` | `DreamProcess` — 4-phase consolidation + 자동 보호/분류 |
| `memory/root_index.rs` | `RootIndex` 관리 |
| `memory/compaction.rs` | `CompactionTree` — 5-level tree |
| `memory/proactive.rs` | `ProactiveRecall` — 3-step selective recall |
| `memory/auto_protect.rs` | `AutoProtector` — 패턴 기반 자동 보호 |
| `memory/auto_classify.rs` | `AutoClassifier` — 내용 기반 자동 분류 |

**기존 파일 수정:**
| 파일 | 변경 |
|------|------|
| `memory/mod.rs` | `MemoryTier`, `ProtectionLevel`, `MemoryType` 확장, `MemoryEntry` 확장 |
| `memory/store.rs` | tier-aware operations, protection-aware operations |
| `config.rs` | `ConsolidationConfig` 추가 |
| `agent_runtime.rs` | `build_full_context()` 사용 |
| `kernel.rs` | Dream process 스폰 |

**기존 데이터 호환:** 모든 새 필드는 `#[serde(default)]`로 기존 JSON과 자동 호환.

### 12.2 Phase 2-4: 순차 구현

1. Phase 2: `DecayEngine` + `AutoProtector` + `AutoClassifier`
2. Phase 3: `DreamProcess` (Phase 1-4) → 백그라운드 스폰
3. Phase 4: `RootIndex` + `ProactiveRecall` → 컨텍스트 주입

---

## 13. 파일 위치 (File Locations)

### 13.1 데이터 파일

```
~/.oxios/workspace/spaces/{space-id}/memory/
├── root_index.json                    # ROOT 인덱스 (자동 관리)
├── conversations/                     # 기존
├── sessions/                          # 기존
├── facts/                             # 기존
├── episodes/                          # 기존
├── skills/                            # 신규
├── preferences/                       # 신규
├── decisions/                         # 신규
├── profiles/                          # 신규
├── compaction/                        # 압축 트리 (자동 관리)
│   ├── daily/
│   ├── weekly/
│   └── monthly/
└── dream_reports/                     # Dream 보고서 (자동 생성)
```

---

## 14. 성공 기준 (Success Criteria)

| 기준 | 측정 |
|------|------|
| 사용자가 설정 없이 기본 작동 | 모든 config에 합리적 기본값 |
| Hot tier 항상 ~3K 토큰 이내 | `build_hot_context().len()` |
| Dream 24시간 주기 자동 실행 | `dream_reports/` 파일 확인 |
| 반복 참조된 메모리 자동 보호 | ProtectionLevel > None인 항목 존재 |
| 안 쓰는 메모리 조용히 삭제 | 90일+ 미접근 + None 보호 = 0개 |
| 기존 데이터 호환 | serde default로 기존 JSON 로드 |
| 기존 테스트 모두 통과 | `cargo test --workspace` |

---

## 15. 리스크 (Risks)

### 15.1 잘못된 자동 보호 (False Positive)
시스템이 중요하지 않은 걸 보호할 수 있음.

**완화:** False positive(안 중요한데 보호)는 false negative(중요한데 삭제)보다 훨씬 안전. 보호 등급은 점진적으로 올라가고 (None → Low → Medium → High), 용량 초과 시 가장 낮은 보호 등급부터 강등.

### 15.2 잘못된 자동 분류
내용 분석이 틀릴 수 있음 (Fact를 Decision으로 분류 등).

**완화:** 분류 오류는 중요도에만 영향. 잘못 높이면 보호가 더 되고(문제없음), 잘못 낮추면 보호가 덜 되지만 감쇠로 자연스럽게 처리됨. 타입 승격은 3회 반복 기준이므로 오타 한 번으로는 안 됨.

### 15.3 Dream이 중요한 메모리를 삭제
**완화:** 5단계 삭제 조건 모두 만족해야. `ProtectionLevel >= Medium`이면 삭제 불가. `UserProfile`/`Preference`는 영구 보호.

### 15.4 ROOT 인덱스 품질
**완화:** 초기에는 간단한 키워드 추출로 빌드. 점진적으로 LLM 압축 품질 개선.

---

## 16. 참고 문헌 (References)

1. **Claude Code Auto Dream** — Anthropic (2026). 4-stage memory consolidation.
2. **Hipocampus** — kevin-hs-sohn (2025). 3-tier memory with 5-level compaction tree.
3. **MemGPT / Letta** — UC Berkeley (2023–2025). Hierarchical memory.
4. **Zep** — Rasmussen et al. (2025). Temporal knowledge graphs.
5. **Ebbinghaus Forgetting Curve** — Hermann Ebbinghaus (1885).
6. **SOAR Cognitive Architecture** — Laird, Newell, Rosenbloom (CMU).
7. **ACT-R** — John Anderson (CMU). Activation-based retrieval.
8. **Sleep-time Compute** — "Beyond Inference Scaling at Test-time" (2025).
9. **Spaced Repetition** — Leitner system. Activation increases with exposure frequency.
10. **RFC-003** — Knowledge Base 독립 분리 (Oxios, 2026-05-20).
