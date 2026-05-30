# RFC-020: Proactive Recall & Sona 학습 엔진 활성화 (Revised)

> **상태:** ✅ 구현 완료
> **원본:** 2026-05-27
> **개정:** 2026-05-27
> **우선순위:** P3
> **범위:** `crates/oxios-kernel/src/memory/{proactive,sona,store}.rs`, `crates/oxios-kernel/src/agent_runtime.rs`, `crates/oxios-kernel/src/memory/dream.rs`
> **선행:** RFC-017 (미사용 모듈 정리)
> **후행:** 없음

---

## 0. 코드 교차 검증 결과 (원본 RFC 오류)

원본 RFC를 코드베이스 실제 구현과 교차 검증한 결과, **5건의 치명적 오류**가 발견되었다. 재작성 전 정리한다.

### 오류 1: `build_full_context()`의 소재 잘못 기술

원본: "`build_full_context()`는 `HnswMemoryIndex`의 메서드이고"
실제: `store.rs`의 `impl MemoryManager`에 정의됨 (라인 ~570). `HnswMemoryIndex`에는 이 메서드가 없음.

### 오류 2: `build_full_context()`가 "호출되지 않는 메서드"

`agent_runtime.rs`의 실제 메모리 주입 경로는:

```rust
// agent_runtime.rs:257-264
match memory_manager.recall(&seed.goal).await {
    Ok(memories) if !memories.is_empty() => {
        system_prompt = memory_manager.blend_into_prompt(&memories, &system_prompt);
    }
    ...
}
```

`recall()` → `blend_into_prompt()` 경로로 **전체 tier 검색**이 이미 이루어지고 있다.
`build_full_context()`는 어디에서도 호출되지 않는다.

### 오류 3: `RecallTiming`을 `AgentRuntime`에 배치하는 설계 오류

`AgentRuntime`은 **Seed 실행 단위**로, 한 번의 `execute()` 호출로 하나의 Seed를 완료한다.
`RecallTiming`은 "세션 단위" 상태(다중 메시지, 토픽 변경 추적)인데, `AgentRuntime` 구조체에 추가하면 매 Seed마다 초기화되어 상태가 유지되지 않는다.

### 오류 4: `current_context` 중복 방지 로직의 `&[]` 버그

원본의 `proactive_recall_context()`:

```rust
let entries = proactive.recall(self, query, &[]).await?;
```

빈 슬라이스 `&[]`를 전달하면 `seen_ids`가 비어 있어, Hot tier 항목이 중복 주입된다.

### 오류 5: `SonaEngine` → `Arc<SonaEngine>` 이중 래핑 문제

`MemoryManager`는 이미 `Arc<MemoryManager>`로 사용된다. `sona_engine: Option<Arc<SonaEngine>>`을 추가하면 `kernel_handle.agents.memory_manager()` 호출 시 이중 Arc가 된다.

---

## 1. 동기

### 1.1 현황

`proactive` (220줄)와 `sona` (585줄)는 **완전히 구현되고 테스트까지 작성된 모듈**이지만,
프로덕션에서 한 번도 실행된 적이 없다.

| 모듈 | 컴파일 | re-export | 인스턴스화 | 호출 |
|------|--------|-----------|-----------|------|
| `proactive` | ✅ | ✅ (`lib.rs`) | ❌ 0건 | ❌ 0건 |
| `sona` | ✅ | ✅ (`mod.rs`) | ❌ 0건 | ❌ 0건 |

### 1.2 각 모듈이 풀려는 문제

#### proactive — "묻기 전에 기억 꺼내주기"

현재 메모리 주입 경로: `recall()` → `blend_into_prompt()`

```rust
// agent_runtime.rs:257 — 실제 경로
match memory_manager.recall(&seed.goal).await {
    Ok(memories) if !memories.is_empty() => {
        system_prompt = memory_manager.blend_into_prompt(&memories, &system_prompt);
    }
}
```

`recall()`은 SQLite backend일 때 BM25+벡터 하이브리드, 없으면 전체 vector index를 뒤진다.
**"Hot tier만 주입"이라는 원본 전제는 현재 코드와 일치하지 않는다.**

하지만 `recall()`은 최근 conversation/session만 우선하고, Warm/Cold의 Decision/Preference를 세션 첫 메시지에 미리 주입하지는 않는다. `proactive`는 이 **선택적 보강** 역할을 담당한다.

3가지 트리거 시점:
1. **세션 첫 메시지** — `RecallTiming::should_recall()`가 `message_count == 0`일 때
2. **토픽 변경** — 키워드 Jaccard 유사도 < 0.3, 메시지 3개 이상 이후
3. **주기적 (10메시지마다)** — `message_count >= 10`일 때

#### sona — "성공한 실행 패턴 학습하기"

현재 Oxios는 Ouroboros의 `evaluate()`에서 verdict(Success/Failure)만 추출한다.
하지만 성공 경로의 **"방법"**(어떤 tool을 어떤 순서로 호출했는지)을 학습하지 않아,
동일 작업 반복 시마다 처음부터 planning을 다시 해야 한다.

`sona`는:
1. 에이전트 실행 trajectory를 기록 (`record`)
2. embedding 기반 클러스터링 후 클러스터 내에서 공통 패턴 추출 (`distill`)
3. 새 작업에서 유사 패턴을 찾아 프롬프트에 힌트로 주입 (`adapt`)

### 1.3 위험

| 위험 | 원인 | 영향 |
|------|------|------|
| 중복 주입 | `recall()`이 이미 전체 검색, proactive recall이 또 검색 | 토큰浪费 |
| TF-IDF 품질 낮음 | sona embedding이 TF-IDF 기반 | 의미 없는 패턴 |
| 성능 저하 | sona record embedding 매 실행 | 응답 지연 |
| 이중 Arc 문제 | SonaEngine을 MemoryManager 내부에 Arc로 배치 | 복잡성 증가 |
| oxi-sdk 의존성 | ToolExecutionEnd에 세부 필드 없으면 trajectory 추적 불가 | 구현 불가 |

---

## 2. 설계 (개선안 기반)

### 2.1 Phase 0: 실험 기반 (1일)

#### 2.1.1 Proactive recall 벤치마크

```bash
# benchmarks/oxios-bench/src/memory_bench.rs

#[bench]
fn bench_proactive_recall_full(b: &mut Bencher) {
    // SQLite: 1000개 entries, SQLiteStore::recall() vs ProactiveRecall.recall() 비교
    // 목표: proactive recall 추가 시간 < 30ms (전체 recall에 포함되므로)
}

#[bench]
fn bench_recall_timing(b: &mut Bencher) {
    // RecallTiming::should_recall() < 0.01ms (단일 메서드 호출)
}
```

**중요**: `proactive.recall()`은 독립적으로 벤치마크하지 말고, **기존 `recall()`에 통합했을 때**의 추가 비용을 측정한다.

| 항목 | 허용 기준 | 측정 방법 |
|------|----------|----------|
| proactive 추가 시간 | < 30ms | 토탈 recall 시간 비교 |
| 토큰 증가 | < 500 tokens 추가 | 프롬프트 길이 측정 |
| 토픽 감지 정확도 | 수동 평가 | 20개 시나리오 (Jaccard > 0.3 트리거 확인) |

#### 2.1.2 Sona distill 품질 평가

현재 `distill()` 알고리즘 문제:
- 임계값 2가 너무 낮음 (2개 trajectory로는 패턴 불안정)
- 도메인 내 클러스터링 없음 (서로 다른 작업을 같은 도메인으로 묶음)

**평가 스크립트**: 5개 도메인 × 각각 10개 성공 trajectory → distill → 패턴 품질 평가

| 항목 | 허용 기준 | 측정 방법 |
|------|----------|----------|
| distill 소요 시간 | < 100ms (500개 trajectory) | 벤치마크 |
| 패턴 품질 | 의미 있는 전략 (수동) ≥ 60% | 5명 평가자 × 5개 패턴 |
| adapt 정확도 | 유사 작업에서 패턴 반환 ≥ 70% | 20개 쿼리 |

**Phase 0 판정**:
- 품질 기준 미달 → 개선안 D (클러스터링 추가) 적용 후 재측정
- 개선 후에도 미달 → distill 알고리즘 전면 재설계 또는 모듈 삭제

### 2.2 Phase 1: Proactive Recall 연결 (1일)

#### 2.2.1 올바른 아키텍처: `recall()` 내부 통합

`build_enriched_context()`를 새 메서드로 만들지 않는다. 기존 `recall()`을 확장한다:

```rust
// store.rs — recall() 확장
pub async fn recall(
    &self,
    query: &str,
) -> Result<Vec<MemoryEntry>> {
    #[cfg(feature = "sqlite-memory")]
    if let Some(ref sqlite) = self.sqlite_store {
        return sqlite.recall(query, self.max_recall).await;
    }
    // ... 기존 구현 ...
}

/// Proactive recall이 적용된 recall.
/// RecallTiming 상태를 외부에서 관리하여 세션 단위 추적 가능.
/// 사용처: AgentRuntime (다중 메시지 세션에서 recall_timing 상태 유지)
pub async fn recall_with_proactive(
    &self,
    query: &str,
    recall_timing: &mut Option<proactive::RecallTiming>,
) -> Result<Vec<MemoryEntry>> {
    // Step 1: 기본 recall (Hot + 검색)
    let mut combined = self.recall(query).await?;

    // Step 2: proactive 보강 — should_recall 트리거 시
    let should_recall = recall_timing
        .as_mut()
        .map(|t| t.should_recall(query))
        .unwrap_or(true);

    if should_recall && combined.len() < self.max_recall {
        let proactive = proactive::ProactiveRecall::new(5, 0.6);
        // ✅ 수정: current_context에 이미 주입된 항목을 전달하여 중복 방지
        let extra = proactive.recall(self, query, &combined).await?;
        combined.extend(extra);
        dedup_by_id(&mut combined);
        combined.truncate(self.max_recall);
    }

    Ok(combined)
}
```

**핵심 변경**: `recall_with_proactive()`는 `RecallTiming`을 인자로 받아서 세션 상태를 추적한다. `RecallTiming`은 `AgentRuntime`이 아니라 **호출자(Orchestrator/Session)가 관리**한다.

#### 2.2.2 RecallTiming 관리 위치

`RecallTiming`은 `AgentRuntime`에 두지 않고, **세션 컨텍스트**에서 관리:

```rust
// orchestrator.rs 또는 session.rs — 세션 레벨
pub struct SessionContext {
    pub recall_timing: Option<proactive::RecallTiming>,
}

impl SessionContext {
    pub fn new() -> Self {
        Self {
            recall_timing: Some(proactive::RecallTiming::new()),
        }
    }
}
```

`AgentRuntime::execute()` 호출 시 `SessionContext`를 인자로 전달:

```rust
pub async fn execute(
    &self,
    agent_id: AgentId,
    seed: &Seed,
    session_ctx: &SessionContext,  // ✅ 추가
) -> Result<ExecutionResult> {
    // ...
    let memories = memory_manager
        .recall_with_proactive(&seed.goal, &mut session_ctx.recall_timing.clone())
        .await?;
    // ...
}
```

#### 2.2.3 config 활용

이미 `ConsolidationConfig`에 `proactive_recall`, `proactive_recall_limit`, `proactive_recall_threshold`가 존재한다:

```toml
# config.toml
[memory.consolidation]
proactive_recall = true         # ✅ 이미 존재
proactive_recall_limit = 5      # ✅ 이미 존재
proactive_recall_threshold = 0.6 # ✅ 이미 존재
```

별도의 새 필드 추가 불필요. `ConsolidationConfig`를 통해 초기화 시 반영.

### 2.3 Phase 2: Sona 학습 루프 연결 (1-2일)

#### 2.3.1 SonaEngine 배치: 이중 Arc 문제 회피

`SonaEngine`은 `MemoryManager` 내부에 두지 않고, `KernelHandle` 레벨에서 관리:

```rust
// kernel_handle/mod.rs 또는 memory/mod.rs의 새 구조체

/// Sona 학습 엔진의 공유 참조.
/// MemoryManager와 DreamProcess에서 동시에 접근하므로,
/// KernelHandle 레벨에서 단일 Arc<SonaEngine>으로 관리.
/// MemoryManager는 필요시 KernelHandle을 통해 접근.
pub struct SonaManager {
    engine: std::sync::Arc<sona::SonaEngine>,
}

impl SonaManager {
    pub fn new(mode: sona::SonaMode, embedding: std::sync::Arc<dyn EmbeddingProvider>) -> Self {
        Self {
            engine: std::sync::Arc::new(sona::SonaEngine::new(mode, embedding)),
        }
    }

    pub fn engine(&self) -> &std::sync::Arc<sona::SonaEngine> {
        &self.engine
    }
}
```

#### 2.3.2 Trajectory 추적: oxi-sdk 이벤트 검증 선행

**전제 조건**: `AgentEvent::ToolExecutionEnd`에 다음 필드가 있어야 함:
- `tool_name: String`
- `duration_ms: u64`
- `result: ToolResult`

oxi-sdk 버전에 따라 필드가 없을 수 있으므로, **Phase 0 또는 Phase 2 초기에 확인**한다:

```rust
// agent_event_types.rs (oxi-sdk) — 확인 필요한 필드
AgentEvent::ToolExecutionEnd {
    tool_call_id: String,
    tool_name: Option<String>,  // ✅ 이 필드가 있는지?
    duration_ms: Option<u64>,   // ✅ 이 필드가 있는지?
    result: Option<ToolResult>,  // ✅ 이 필드가 있는지?
    is_error: bool,
}
```

필드가 없으면 **oxi-sdk PR 또는 feature request**가 선행되어야 한다.
필드가 있다면 `run_streaming` 콜백에서 수집:

```rust
// agent_runtime.rs — run_agent()의 콜백 내부

// TrajectoryCollector는 Mutex<Vec<TrajectoryStep>>으로 스레드 안전
struct TrajectoryCollector {
    steps: parking_lot::Mutex<Vec<sona::TrajectoryStep>>,
}

AgentEvent::ToolExecutionEnd { tool_name, duration_ms, result, is_error, .. } => {
    let collector = trajectory_collector.lock();
    collector.push(sona::TrajectoryStep {
        input: tool_name.unwrap_or_default(),
        output: summarize_result(&result),
        duration_ms: duration_ms.unwrap_or(0),
        confidence: if is_error { 0.3 } else { 0.8 },
    });
}
```

#### 2.3.3 Distill: Dream의 기존 패턴 persist와 중복 제거

`dream.rs`의 `dream_prune_and_index()` Phase 9:

```rust
// dream.rs:Phase 9 — 기존 코드
#[cfg(feature = "sqlite-memory")]
if let Some(ref sqlite) = self.memory_manager.sqlite_store() {
    let _ = sqlite.auto_promote_patterns(0.8, 3);  // ← 이게 SONA와 무엇 관계?
}
```

**문제**: `auto_promote_patterns()`와 `SonaEngine`의 관계가 불분명.
이것이 SonaEngine의 persist를 대체하는지, 독립적인지 확인 필요.

**해결**: Phase 2 초기에서 `auto_promote_patterns()`의 역할을 명확히 하고, SonaEngine의 `distill()` + `persist_to_sqlite()`와 통합한다:

```rust
// dream.rs — Phase 9 revised
let patterns_persisted = {
    if let Some(ref sona) = sona_manager.engine() {
        match sona.distill().await {
            Ok(patterns) => {
                tracing::info!(count = patterns.len(), "SONA patterns distilled");
                // ✅ SonaEngine의 persist 사용 (기존 auto_promote_patterns와 별도)
                #[cfg(feature = "sqlite-memory")]
                if let Some(ref sqlite) = self.memory_manager.sqlite_store() {
                    if let Err(e) = sona.persist_to_sqlite(sqlite) {
                        tracing::warn!(error = %e, "SONA persist failed");
                    }
                }
                patterns.len()
            }
            Err(e) => {
                tracing::warn!(error = %e, "SONA distillation failed");
                0
            }
        }
    } else {
        0
    }
};
```

#### 2.3.4 학습된 패턴 주입

```rust
// agent_runtime.rs — build_system_prompt()에 추가

fn build_system_prompt(
    &self,
    seed: &Seed,
    persona_prompt: Option<&str>,
    capabilities_xml: Option<&str>,
    kernel_manifest: Option<&str>,
    learned_pattern: Option<&sona::LearnedPattern>,  // ✅ 추가
) -> String {
    // ... 기존 로직 ...
    
    // Sona 학습 패턴 주입
    if let Some(pattern) = learned_pattern {
        prompt.push_str(&format!(
            "\n\n## Learned Strategy (confidence: {:.0}%)\n{}\n",
            pattern.confidence * 100.0,
            pattern.strategy,
        ));
    }
    
    prompt
}
```

호출처에서:
```rust
let learned_pattern = session_ctx.sona_manager()
    .and_then(|s| s.engine().adapt(&seed.goal).ok().flatten());
system_prompt = build_system_prompt(seed, persona_prompt, ..., learned_pattern.as_ref());
```

#### 2.3.5 config: `sona_enabled` 대신 `LearningConfig` 활용

이미 `config.rs`에 `LearningConfig`가 존재한다:

```rust
pub struct LearningConfig {
    pub enabled: bool,           // ✅ 이것이 sona_enabled 역할
    pub sona_mode: String,      // ✅ 이미 존재
    pub distill_interval_hours: u64,
    pub auto_promote_quality: f32,
    pub auto_promote_min_usage: u32,
}
```

별도의 `sona_enabled` 필드 추가 불필요. `learning.enabled = false`이면 모든 Sona 로직 스킵.

### 2.4 기능 플래그 요약

```toml
# config.toml
[memory.consolidation]
proactive_recall = true          # ✅ 기존 필드
proactive_recall_limit = 5      # ✅ 기존 필드
proactive_recall_threshold = 0.6 # ✅ 기존 필드

[memory.learning]
enabled = true                   # ✅ 기존 필드 (sona_enabled 대체)
sona_mode = "balanced"          # ✅ 기존 필드
```

---

## 3. 마이그레이션 계획

### Phase 0: 실험 (1일)

| 작업 | 산출물 | 판정 기준 |
|------|--------|----------|
| proactive recall 추가 비용 측정 | 벤치마크 결과 | < 30ms 추가 |
| oxi-sdk AgentEvent 필드 확인 | field availability note | 트래킹 가능/불가능 |
| sona distill 품질 평가 | 평가 보고서 | 의미 있는 패턴 ≥ 60% |
| distill 알고리즘 개선 (개선안 D) | 수정된 sona.rs | Phase 0 실패 시만 |
| SonaEngine vs auto_promote_patterns 관계 분석 | 기술 메모 | Phase 2 초기 |

**Phase 0 실패 시**: 모듈 삭제 여부를 별도 논의. git history에 보존.

### Phase 1: Proactive Recall (1일) — Phase 0 통과 시

| 작업 | 파일 |
|------|------|
| `MemoryManager::recall_with_proactive()` 구현 | `memory/store.rs` |
| `RecallTiming`을 `AgentRuntime`이 아닌 **세션 컨텍스트**에 배치 | `orchestrator.rs` 또는 새 파일 |
| `AgentRuntime::execute()`에 `session_ctx` 인자 추가 | `agent_runtime.rs` |
| `current_context` 중복 방지 로직 수정 (`&[]` → 실제 항목) | `memory/proactive.rs` |
| `ConsolidationConfig`로 초기화 반영 | kernel.rs |
| 통합 테스트: 세션 시작 시 proactive recall 트리거 | 신규 테스트 |
| 통합 테스트: 토큰 예산 초과하지 않음 | 신규 테스트 |

### Phase 2: Sona (1-2일) — Phase 1 완료 + oxi-sdk 필드 확인 후

| 작업 | 파일 | 비고 |
|------|------|------|
| oxi-sdk AgentEvent 필드 PR (필요 시) | upstream | Phase 0에서 확인 후 |
| `SonaManager` 구조체 정의 | `memory/mod.rs` 또는 `kernel_handle/` | 이중 Arc 회피 |
| `SonaManager` 초기화 + KernelHandle 연결 | kernel.rs | `LearningConfig` 사용 |
| `TrajectoryCollector` 구조체 추가 | `agent_runtime.rs` | `run_streaming` 콜백 내부 |
| tool call 루프에서 trajectory 기록 | `agent_runtime.rs` | oxi-sdk 이벤트 활용 |
| Dream Phase 9에 `sona.distill()` + `persist_to_sqlite()` 통합 | `memory/dream.rs` | 기존 auto_promote와 중복 제거 |
| `build_system_prompt()`에 학습 패턴 주입 파라미터 추가 | `agent_runtime.rs` | `LearnedPattern` 인자 |
| Sona 패턴 로깅/메트릭 추가 | `observability.rs` | patterns_distilled, adapt_hits 등 |
| 통합 테스트: trajectory 기록 → distill → adapt 전체 흐름 | 신규 테스트 |

### Phase 3: 검증 (0.5일)

| 작업 | 비고 |
|------|------|
| end-to-end 시나리오: 동일 작업 2회 → 2회차에서 패턴 주입 확인 | 핵심 테스트 |
| 성능 회귀: 기존 벤치마크 대비 응답 시간 증가 < 10% | 벤치마크 비교 |
| `cargo test --workspace` | 전체 테스트 |
| oxi-sdk AgentEvent 필드 의존성 문서화 | upstream 호환성 |

---

## 4. 고려사항

### 4.1 Proactive recall vs 기존 recall() — 중복 회피

`recall()`이 이미 전체 tier 검색을 수행하므로, `proactive.recall()`은 **최종 결과에 대해 추가 항목만** 반환해야 한다. 수정된 `recall_with_proactive()`에서:

1. `recall()` 먼저 실행 → `combined`에 저장
2. `current_context`에 `combined`을 전달하여 `seen_ids` 초기화
3. `proactive.recall()`는 중복되지 않은 추가 항목만 반환
4. `dedup_by_id()`로 최종 정리

### 4.2 SQLite backend에서의 동작

`recall()`이 SQLite backend일 때 `sqlite.recall()` → `sqlite.search()` → BM25+벡터 hybrid가 이미 수행된다.
`recall_with_proactive()`에서 `proactive.recall()`이 또 `search()`를 호출하면 **이중 검색**이 발생한다.

**해결**: SQLite backend에서는 `proactive.recall()`의 search를スキ핑하고, `list_by_tier(Warm, limit)`만 수행한다:

```rust
// store.rs — recall_with_proactive 수정
if should_recall && combined.len() < self.max_recall {
    // SQLite: 이미 검색 완료되었으므로 tier list만
    #[cfg(feature = "sqlite-memory")]
    if self.sqlite_store.is_some() {
        let warm = self.list_by_tier(MemoryTier::Warm, self.max_recall - combined.len()).await?;
        for entry in warm {
            if !seen_ids.contains(&entry.id) {
                combined.push(entry);
            }
        }
    } else {
        // JSON backend: proactive.search() 수행
        let proactive = proactive::ProactiveRecall::new(5, 0.6);
        let extra = proactive.recall(self, query, &combined).await?;
        combined.extend(extra);
    }
    dedup_by_id(&mut combined);
    combined.truncate(self.max_recall);
}
```

### 4.3 Sona distill 알고리즘 개선 (개선안 D)

현재 "첫 3 스텝을 →로 연결"은 너무 단순하다. 개선안:

```rust
// sona.rs — distill() 개선
pub async fn distill(&self) -> Result<Vec<LearnedPattern>> {
    let trajs = self.trajectories.read();
    
    let mut domain_groups: HashMap<String, Vec<&Trajectory>> = HashMap::new();
    for traj in trajs.iter() {
        if traj.verdict == Verdict::Success {
            domain_groups.entry(traj.domain.clone()).or_default().push(traj);
        }
    }

    for (domain, group) in domain_groups {
        if group.len() < 3 { continue; } // ✅ 임계값 2 → 3으로 증가
        
        // embedding 기반 클러스터링
        let clusters = cluster_by_embedding(&group, 0.7);  // TODO: 구현
        for cluster in clusters {
            if cluster.len() < 2 { continue; }
            
            // 공통 스텝 subsequence 추출 (LCS)
            let common_steps = extract_common_subsequence(&cluster);
            let strategy = format_strategy(&common_steps);
            let confidence = (cluster.len() as f32 * 0.2).min(1.0);
            
            patterns.push(LearnedPattern { strategy, confidence, ... });
        }
    }
    Ok(patterns)
}
```

### 4.4 oxi-sdk 이벤트 필드 의존성

`SonaEngine`의 trajectory 추적은 oxi-sdk의 `AgentEvent::ToolExecutionEnd`에 의존한다.
현재 코드는 `steps_completed`만 카운트할 뿐, tool name, duration, result를 추출하지 못한다.
**Phase 0에서 반드시 확인**하고, 필드가 없으면 upstream에 요청하거나 대체 구현을検討한다.

### 4.5 Dream의 patterns_persisted — 기존 로직 정리

현재 `dream_prune_and_index()` Phase 9에서 `sqlite.auto_promote_patterns()`를 호출한다.
이것이 SonaEngine의 패턴 persist와 어떤 관계인지 명확히 해야 한다:
- `auto_promote_patterns`가 독립적인 패턴 관리라면 → SonaEngine과 병행
- `auto_promote_patterns`가 SonaEngine의 subset이라면 → 제거하고 SonaEngine으로 통일

**Phase 0에서 분석 완료** 후 Phase 2에서 정리.

---

## 5. 성공 기준

### Phase 0

- [ ] proactive recall 추가 시간: < 30ms
- [ ] oxi-sdk AgentEvent 필드 가용성 확인: 트래킹 가능/불가능 판정
- [ ] sona distill 품질: 의미 있는 패턴 ≥ 60%
- [ ] SonaEngine vs auto_promote_patterns 관계: 기술 메모 작성
- [ ] distill 클러스터링 개선 후 품질 재평가 (Phase 0 실패 시)

### Phase 1

- [ ] `recall_with_proactive()`가 세션 첫 메시지에서 proactive 항목 포함
- [ ] 토픽 변경 감지 시 warm tier에서 관련 기억 회상
- [ ] 토큰 예산 초과하지 않음 (기존 대비 증가 < 30%)
- [ ] RecallTiming이 세션 레벨에서 올바르게 관리됨

### Phase 2

- [ ] tool execution trajectory가 기록됨 (oxi-sdk 필드 가용 시)
- [ ] Dream에서 Sona 패턴이 distill + persist됨
- [ ] 동일 도메인 재실행 시 학습 패턴이 프롬프트에 주입됨
- [ ] `learning.enabled = false` 시 성능 영향 없음
- [ ] `auto_promote_patterns` vs SonaEngine 정리됨

### Phase 3

- [ ] `cargo test --workspace` 통과
- [ ] 기존 벤치마크 대비 응답 시간 증가 < 10%
- [ ] end-to-end: 동일 작업 2회 → 2회차에서 learned pattern 주입 확인

---

## 6. 변경 로그 (v1 → v2)

| 항목 | 원본 | 수정 |
|------|------|------|
| `build_full_context()` 위치 | "HnswMemoryIndex의 메서드" | `MemoryManager`의 메서드 (정정) |
| 메모리 주입 경로 | `build_full_context()` | `recall()` → `blend_into_prompt()` (실제 경로) |
| `build_full_context()` 존재 여부 | "유지" | "호출되지 않음 — 새 메서드 불필요" |
| `RecallTiming` 배치 | `AgentRuntime` 구조체 | 세션 컨텍스트 (Orchestrator 레벨) |
| `AgentRuntime::execute()` 시그니처 | 변경 없음 | `session_ctx` 인자 추가 |
| `current_context` 중복 방지 | `&[]` 전달 (버그) | 실제 주입 항목 전달 |
| SonaEngine 배치 | `MemoryManager` 내부 `Arc<SonaEngine>` | `SonaManager` (KernelHandle 레벨) |
| 이중 Arc 문제 | 고려 안함 | SonaManager로 회피 |
| oxi-sdk 의존성 | 언급 없음 | Phase 0 선행 조건으로 추가 |
| `sona_enabled` 필드 | "신규 추가" | `LearningConfig.enabled` 활용 (이미 존재) |
| Dream patterns_persisted | 중복 언급 안함 | 기존 로직과 관계 분석 추가 |
| distill 임계값 | ≥ 2 | ≥ 3 (개선안 D 반영) |
| distill 클러스터링 | 없음 | embedding 기반 클러스터링 추가 (개선안 D) |
| SQLite 중복 검색 | 고려 안함 | backend 감지 후 proactive search 스킵 (개선안 B) |