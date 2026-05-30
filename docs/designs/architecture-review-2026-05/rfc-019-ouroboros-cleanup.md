# RFC-019: Ouroboros Evolution 루프 활성화

> **상태:** ✅ 구현 완료
> **날짜:** 2026-05-27 (v2 개정)
> **우선순위:** P2
> **범위:** `crates/oxios-ouroboros/`, `crates/oxios-kernel/src/orchestrator.rs`, `crates/oxios-kernel/src/config.rs`, `crates/oxios-kernel/src/event_bus.rs`
> **선행:** 없음
> **후행:** 없음
> **리뷰:** v1 리뷰에서 컴파일 불가 코드, 필드명 충돌, 누락된 연결 로직 등 8건 지적 반영

---

## 1. 동기

### 1.1 현재 상태: 설계는 5단계, 구현은 2.5단계

Ouroboros 프로토콜은 **Interview → Seed → Execute → Evaluate → Evolve** 5단계로 설계되었다.
하지만 Orchestrator의 `handle_message()`는 인라인 단순 평가로 끝난다.

```
설계된 흐름:
  Interview → Seed → Execute → Evaluate(3단계) → Evolve(필요시) → 재실행
                                             ↑                        ↓
                                             └──── evolution loop ────┘

현재 흐름:
  Interview → Seed → Execute(Orchestrator 직접) → results.iter().all(|r| r.success) → 끝
```

### 1.2 핵심 발견: evaluate()와 evolve()는 이미 구현되어 있다

코드 검증 결과:

| 메서드 | 구현 상태 | 연결 상태 | 비고 |
|--------|----------|----------|------|
| `OuroborosEngine::interview()` | ✅ 완전 구현 | ✅ 연결됨 | Orchestrator에서 호출 |
| `OuroborosEngine::generate_seed()` | ✅ 완전 구현 | ✅ 연결됨 | Orchestrator에서 호출 |
| `OuroborosEngine::execute()` | ⚠️ 껍데기 | ❌ 미연결 | `success: false` 반환, Orchestrator가 직접 실행 |
| `OuroborosEngine::evaluate()` | ✅ 완전 구현 | ❌ 미연결 | 3단계 평가 + 캐시까지 구현됨 |
| `OuroborosEngine::evolve()` | ✅ 완전 구현 | ❌ 미연결 | LLM 기반 Seed 개선 |

**코드 내 TODO 주석:**

```rust
// ouroboros_engine.rs — execute():
// Execution is delegated to the kernel's AgentRuntime via the Supervisor.
// The Orchestrator calls Supervisor::run_with_seed() directly.

// ouroboros_engine.rs — evaluate():
#[allow(dead_code)]

// ouroboros_engine.rs — evolve():
#[allow(dead_code)]
```

### 1.3 삭제된 모듈

```rust
// crates/oxios-ouroboros/src/lib.rs:23-24
// pub mod lateral;    // Removed: evolve() is not called by any caller
// pub mod regression; // Removed: evolve() is not called by any caller
```

**주의:** 이 모듈들은 단순히 주석 처리된 것이 아니라 **파일이 삭제**되었다.
git history (`527420c`)에서만 존재하므로 복원 + 현재 API 마이그레이션이 필요하다.

### 1.4 설정 불일치

`default-config.toml`에는 이미 `[orchestrator]` 섹션이 있지만, `OrchestratorConfig` struct가 비어 있어 **파싱 결과가 무시**된다.

```toml
# share/default-config.toml — 현재 파일에 이미 존재
[orchestrator]
max_evolution_iterations = 3
min_evaluation_score = 0.8
```

```rust
// config.rs — 빈 struct, 필드가 없어 위 설정이 파싱되지 않음
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OrchestratorConfig {}
```

`with_config()`에서도 `_config` prefix로 무시:

```rust
pub fn with_config(
    ...,
    _config: crate::config::OrchestratorConfig,  // ← 무시됨
) -> Self { ... }
```

---

## 2. 설계 원칙

| 원칙 | 의미 |
|------|------|
| **기존 필드명 존중** | `default-config.toml`에 이미 있는 필드명(`min_evaluation_score`)을 struct에 그대로 매핑 |
| **Simple task 영향 제로** | acceptance criteria가 없는 simple task는 LLM 호출 수·지연 변화 없음 |
| **Best-result 보장** | evolution이 점수를 악화시키면 이전 최고 결과를 반환 |
| **KernelEvent 확장** | 새 이벤트 enum을 만들지 않고 기존 `KernelEvent`에 variant 추가 |
| **Seed에 complexity 불추가** | complexity는 `InterviewResult`에만 존재. Seed는 판단 기준으로 acceptance_criteria 수를 사용 |
| **콜백 기반 execute() 지양** | trait 시그니처 변경 대신 Orchestrator가 실행 경로를 소유 |

---

## 3. 설계

### 3.1 목표 아키텍처

```
handle_message()
  │
  ├─ Interview (LLM: task vs chat + 복잡도 + 모호성)
  │   ├─ chat → 응답 반환
  │   └─ task ↓
  │
  ├─ Seed
  │   ├─ simple → Seed::from_message()
  │   └─ complex → generate_seed() (LLM)
  │
  ├─ Execute (Orchestrator가 lifecycle.spawn_and_run 직접 호출 — 변경 없음)
  │
  ├─ Evaluate (신규 연결: 기존 OuroborosEngine::evaluate 활성화)
  │   ├─ Stage 1: 기계적 평가 (acceptance criteria vs output)
  │   ├─ Stage 2: 의미 평가 (LLM, 기계적 통과 시 스킵)
  │   └─ Stage 3: 합의 (선택적)
  │
  ├─ score >= threshold? → 반환
  │
  └─ Evolve Loop (최대 N회)
      ├─ Evolve: LLM이 Seed 개선
      ├─ 재실행
      └─ 재평가 → score >= threshold? → 반환 / 다시 evolve
                 └─ 점수 하락 시 이전 best 결과 반환
```

### 3.2 execute() — 변경하지 않음

v1에서는 `OuroborosEngine::execute()`를 콜백 기반으로 리팩토링하려 했으나, 이는 `OuroborosProtocol` trait의 시그니처 변경을 유발하고 복잡도만 증가시킨다. **실행 책임은 Orchestrator가 그대로 가진다.**

`OuroborosEngine::execute()`는 현재 껍데기(`success: false`)이며, 이 RFC에서는 이 메서드를 **그대로 둔다**. 대신 Orchestrator가 직접 `lifecycle.spawn_and_run()`을 호출한 후 그 결과를 `evaluate()`에 전달하는 구조를 유지한다.

이유:
- trait 시그니처 변경이 파급 효과를 가짐
- 실행 경로(Supervisor, AgentRuntime)는 이미 Orchestrator에 깊이 결합되어 있음
- `OuroborosEngine`은 LLM 호출(interview/seed/evaluate/evolve)에 집중

### 3.3 OrchestratorConfig에 필드 추가 — 기존 TOML 필드명 존중

```rust
// crates/oxios-kernel/src/config.rs

/// Orchestrator configuration (Ouroboros protocol execution).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrchestratorConfig {
    /// 최대 evolution 반복 횟수.
    /// 0 = evolution 비활성화 (evaluate만 수행).
    /// 기본값: 3 (default-config.toml과 일치).
    #[serde(default = "default_max_evolution_iterations")]
    pub max_evolution_iterations: u32,

    /// 평가 통과 점수 임계값 (0.0 ~ 1.0).
    /// 이 점수 이상이면 evolution 없이 통과.
    /// 기본값: 0.8 (default-config.toml과 일치).
    #[serde(default = "default_min_evaluation_score")]
    pub min_evaluation_score: f64,

    /// 평가 결과 캐시 활성화.
    #[serde(default = "default_true")]
    pub eval_cache_enabled: bool,
}

fn default_max_evolution_iterations() -> u32 { 3 }
fn default_min_evaluation_score() -> f64 { 0.8 }

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_evolution_iterations: default_max_evolution_iterations(),
            min_evaluation_score: default_min_evaluation_score(),
            eval_cache_enabled: true,
        }
    }
}
```

**주의:** `default-config.toml`의 기존 필드명 `max_evolution_iterations`, `min_evaluation_score`를 그대로 사용한다. v1이 제안한 `eval_score_threshold`는 사용하지 않는다.

`default-config.toml`에 캐시 설정만 추가:

```toml
# share/default-config.toml
[orchestrator]
max_evolution_iterations = 3
min_evaluation_score = 0.8
eval_cache_enabled = true
```

### 3.4 with_config()에서 설정 연결

```rust
// orchestrator.rs

pub struct Orchestrator {
    ouroboros: Arc<dyn OuroborosProtocol>,
    // ... 기존 필드 ...
    /// Evolution 설정 (config.rs에서 로드).
    evolution_config: EvolutionConfig,
}

/// Evolution 루프 설정. OrchestratorConfig에서 추출.
#[derive(Debug, Clone)]
struct EvolutionConfig {
    max_iterations: u32,
    score_threshold: f64,
    eval_cache_enabled: bool,
}

impl From<crate::config::OrchestratorConfig> for EvolutionConfig {
    fn from(c: crate::config::OrchestratorConfig) -> Self {
        Self {
            max_iterations: c.max_evolution_iterations,
            score_threshold: c.min_evaluation_score,
            eval_cache_enabled: c.eval_cache_enabled,
        }
    }
}

impl Orchestrator {
    pub fn with_config(
        ouroboros: Arc<dyn OuroborosProtocol>,
        event_bus: EventBus,
        state_store: Arc<StateStore>,
        lifecycle: AgentLifecycleManager,
        config: crate::config::OrchestratorConfig,  // ← _ prefix 제거
    ) -> Self {
        let evolution_config = EvolutionConfig::from(config);
        Self {
            ouroboros,
            event_bus,
            state_store,
            git_layer: None,
            sessions: RwLock::new(std::collections::HashMap::new()),
            lifecycle,
            a2a: None,
            space_manager: RwLock::new(None),
            conversation_buffer: RwLock::new(ConversationBuffer::default()),
            delegation_config: DelegationConfig::default(),
            a2a_breaker: Arc::new(crate::a2a_circuit_breaker::A2ACircuitBreaker::new(5, 30)),
            evolution_config,  // ← 저장
        }
    }
}
```

### 3.5 Evolution 루프 연결

```rust
// orchestrator.rs — handle_message() 확장

// ── 기존 실행 코드 이후 ──

// Evaluate + Evolve
let (final_result, evaluation, final_seed) = if self.should_evaluate(&seed) {
    self.run_evolution_loop(&session_id, seed, exec_result).await?
} else {
    // Simple task: 기계적 평가만 (LLM 호출 없음)
    let passed = exec_result.success;
    let eval = EvaluationResult::mechanical_only(passed, if passed { 1.0 } else { 0.0 });
    (exec_result, eval, seed)
};

// session 정리, 결과 반환 ...
```

```rust
impl Orchestrator {
    /// Seed가 evaluate + evolution 대상인지 판단.
    ///
    /// 판단 기준:
    /// - acceptance_criteria가 1개 이상 있어야 함 (없으면 기계적 평가 불가)
    /// - output_schema가 없어야 함 (schema 검증은 별도 경로)
    ///
    /// 주의: Seed에는 complexity 필드가 없다.
    /// InterviewResult.complexity는 이미 routing 시점에서 소비됨.
    fn should_evaluate(&self, seed: &Seed) -> bool {
        !seed.acceptance_criteria.is_empty() && seed.output_schema.is_none()
    }

    /// Evaluate → (optional) Evolve → re-execute loop.
    ///
    /// best_result / best_seed / best_score를 추적하여
    /// evolution이 점수를 악화시키면 이전 최고 결과를 반환.
    async fn run_evolution_loop(
        &self,
        session_id: &str,
        seed: Seed,
        initial_result: ExecutionResult,
    ) -> Result<(ExecutionResult, EvaluationResult, Seed)> {
        let max_iterations = self.evolution_config.max_iterations;
        let threshold = self.evolution_config.score_threshold;

        let mut current_seed = seed.clone();
        let mut current_result = initial_result;

        // Best-result 추적
        let mut best_result = current_result.clone();
        let mut best_seed = current_seed.clone();
        let mut best_eval: Option<EvaluationResult> = None;

        for iteration in 0..=max_iterations {
            // Evaluate
            let evaluation = self.ouroboros.evaluate(&current_seed, &current_result).await?;

            tracing::info!(
                iteration,
                score = evaluation.score,
                passed = evaluation.all_passed(),
                "Evaluation complete"
            );

            self.event_bus.publish(KernelEvent::EvaluationComplete {
                seed_id: current_seed.id,
                passed: evaluation.all_passed(),
            })?;

            // best 갱신
            if best_eval.as_ref().map_or(true, |b| evaluation.score >= b.score) {
                best_result = current_result.clone();
                best_seed = current_seed.clone();
                best_eval = Some(evaluation.clone());
            }

            // 통과 or 마지막 iteration
            if evaluation.score >= threshold || iteration == max_iterations {
                return Ok((best_result, best_eval.unwrap(), best_seed));
            }

            // max_iterations == 0이면 evolution 없이 평가만
            if max_iterations == 0 {
                return Ok((best_result, best_eval.unwrap(), best_seed));
            }

            // Evolve: 개선된 Seed 생성
            let evolved = self.ouroboros.evolve(&current_seed, &evaluation).await?;
            match evolved {
                Some(new_seed) => {
                    tracing::info!(
                        old_seed_id = %current_seed.id,
                        new_seed_id = %new_seed.id,
                        iteration,
                        "Seed evolved, re-executing"
                    );

                    self.event_bus.publish(KernelEvent::EvolutionStarted {
                        seed_id: current_seed.id,
                        new_seed_id: new_seed.id,
                        iteration,
                    })?;

                    current_seed = new_seed;
                    current_result = self.execute_seed(&current_seed).await?;
                }
                None => {
                    // evolve가 None이면 개선 불가
                    tracing::info!(seed_id = %current_seed.id, "Evolve returned None, stopping");
                    return Ok((best_result, best_eval.unwrap(), best_seed));
                }
            }
        }

        // 여기에 도달하지 않음 (마지막 iteration에서 반환)
        unreachable!()
    }

    /// Seed 실행 (Orchestrator의 기존 경로).
    async fn execute_seed(&self, seed: &Seed) -> Result<ExecutionResult> {
        self.lifecycle.spawn_and_run(seed, Priority::Normal).await
    }
}
```

### 3.6 Multi-agent 경로와의 상호작용

현재 `handle_message()`에는 두 개의 실행 경로가 있다:

1. **Multi-agent 경로** (acceptance_criteria ≥ 5): `delegate_subtasks()` → 각 subtask 독립 실행 → `results.iter().all(|r| r.success)`
2. **Single-agent 경로** (acceptance_criteria < 5): `lifecycle.spawn_and_run()` → 인라인 평가

Evolution 루프는 **single-agent 경로에만** 적용한다. 이유:

- Multi-agent 경로에서는 각 subtask가 독립 seed를 가짐. 전체 결과를 하나의 Seed에 대해 evaluate하는 것은 의미가 없음.
- Multi-agent의 품질 관리는 개별 subtask 수준에서 이루어져야 함 (별도 RFC).

```rust
// handle_message() 내에서의 분기:

if should_split_seed(&seed) {
    // Multi-agent 경로: evolution 없음 (기존 로직 유지)
    let results = self.delegate_subtasks(subtasks, &seed).await?;
    // ... 기존 코드 ...
} else {
    // Single-agent 경로
    let exec_result = self.execute_seed(&seed).await?;

    // Evolution 루프 (신규)
    let (final_result, evaluation, final_seed) = if self.should_evaluate(&seed) {
        self.run_evolution_loop(&session_id, seed, exec_result).await?
    } else {
        let passed = exec_result.success;
        let eval = EvaluationResult::mechanical_only(passed, if passed { 1.0 } else { 0.0 });
        (exec_result, eval, seed)
    };

    // ... 결과 반환 ...
}
```

### 3.7 KernelEvent에 Evolution variant 추가

새 `OrchestratorEvent` enum을 만들지 않고 기존 `KernelEvent`에 추가:

```rust
// event_bus.rs — KernelEvent 확장

pub enum KernelEvent {
    // ... 기존 variant ...

    /// Evaluation has completed (기존 — 그대로 사용).
    EvaluationComplete {
        seed_id: uuid::Uuid,
        passed: bool,
    },

    /// Evolution이 시작됨 (신규).
    EvolutionStarted {
        /// 진화 전 seed ID.
        seed_id: uuid::Uuid,
        /// 진화 후 seed ID.
        new_seed_id: uuid::Uuid,
        /// 현재 iteration (0부터).
        iteration: u32,
    },

    /// Evolution이 최대 반복에 도달함 (신규).
    EvolutionMaxReached {
        seed_id: uuid::Uuid,
        final_score: f64,
        iterations: u32,
    },
}
```

`kernel_event_to_audit_action()`에도 매핑 추가:

```rust
KernelEvent::EvolutionStarted { seed_id, new_seed_id, iteration } => AuditAction::Other {
    detail: format!("evolution:{}→{}:iter{}", seed_id, new_seed_id, iteration),
},
KernelEvent::EvolutionMaxReached { seed_id, final_score, iterations } => AuditAction::Other {
    detail: format!("evolution_max:{}:score={}:iters={}", seed_id, final_score, iterations),
},
```

### 3.8 lateral.rs / regression.rs 복원

파일이 삭제되어 있으므로 git history에서 복원 후 현재 API에 맞게 수정:

```bash
# 복원
git show 527420c:crates/oxios-ouroboros/src/lateral.rs > crates/oxios-ouroboros/src/lateral.rs
git show 527420c:crates/oxios-ouroboros/src/regression.rs > crates/oxios-ouroboros/src/regression.rs
```

복원 후 필요한 작업:

1. **`lateral.rs`**: 현재 API와 호환성 확인. `Seed` struct 변경분(新增 필드) 반영.
   - `rethink()` 함수가 `&Seed`와 `&EvaluationResult`를 받도록 시그니처 조정
   - 사용하지 않는 타입 import 정리

2. **`regression.rs`**: `RegressionDetector`를 `run_evolution_loop()`에 통합.
   - `GenerationRecord`가 현재 `Seed` + `EvaluationResult`에서 생성되도록 수정
   - 10세대 히스토리 유지 로직은 그대로

`evolve()` 확장 (lateral/regression 통합):

```rust
// ouroboros_engine.rs — evolve() 내부, LLM evolve 이후

// Lateral thinking: stagnation 감지 시 다른 관점에서 Seed 재구성
if seed.generation >= 2 {
    if let Some(lateral_seed) = crate::lateral::rethink(seed, evaluation) {
        candidates.push(lateral_seed);
    }
}

// Regression check: 이전에 통과한 criteria가 실패하기 시작하면 경고
let regressions = crate::regression::RegressionDetector::new()
    .detect_from_history(generation_history);
if !regressions.is_empty() {
    // regression 정보를 evolve 프롬프트에 주입
    // (다음 LLM 호출 시 회귀 방지)
}
```

`lib.rs`에서 주석 해제:

```rust
pub mod lateral;
pub mod regression;
```

### 3.9 #[allow(dead_code)] 정리

`ouroboros_engine.rs`에서 `evaluate()`, `evolve()`의 `#[allow(dead_code)]` 제거.
`set_persona_prompt()`은 유지 (trait의 default 메서드 오버라이드, 직접 호출 없음).

`delegation_config.timeout_ms`의 `#[allow(dead_code)]`는 **이 RFC와 무관하므로 건드리지 않는다.** 별도 정리 커밋으로 분리.

---

## 4. 마이그레이션 계획

### Phase 1: 설정 + 기반 구조 (1일)

| 작업 | 파일 | 비고 |
|------|------|------|
| `OrchestratorConfig`에 필드 추가 | `config.rs` | `max_evolution_iterations`, `min_evaluation_score`, `eval_cache_enabled` |
| `OrchestratorConfig::default()` 구현 | `config.rs` | `#[derive(Default)]` 제거, manual impl |
| `default-config.toml`에 `eval_cache_enabled` 추가 | `default-config.toml` | 기존 두 필드는 그대로 |
| `EvolutionConfig` struct 추가 | `orchestrator.rs` | `OrchestratorConfig` → `EvolutionConfig` 변환 |
| `with_config()`에서 `_config` → `config`로 변경, 필드 저장 | `orchestrator.rs` | `_` prefix 제거 |
| `should_evaluate()` 구현 | `orchestrator.rs` | acceptance_criteria 비어있지 않음 + output_schema 없음 |
| `execute_seed()` helper 추출 | `orchestrator.rs` | `lifecycle.spawn_and_run()` 래핑 |

### Phase 2: Evolution 루프 연결 (1-2일)

| 작업 | 파일 | 비고 |
|------|------|------|
| `run_evolution_loop()` 구현 | `orchestrator.rs` | best-result 추적 포함 |
| `handle_message()`에 루프 통합 | `orchestrator.rs` | single-agent 경로에만 적용 |
| `EvaluationResult` import | `orchestrator.rs` | `oxios_ouroboros::EvaluationResult` |
| `#[allow(dead_code)]` 제거 | `ouroboros_engine.rs` | evaluate, evolve |
| `KernelEvent` variant 2개 추가 | `event_bus.rs` | `EvolutionStarted`, `EvolutionMaxReached` |
| audit_action 매핑 추가 | `event_bus.rs` | 새 variant에 대한 매핑 |

### Phase 3: lateral/regression 복원 (1-2일)

| 작업 | 파일 | 비고 |
|------|------|------|
| git history에서 파일 복원 | `ouroboros/src/lateral.rs`, `regression.rs` | `git show 527420c:...` |
| API 마이그레이션 | `lateral.rs`, `regression.rs` | 현재 Seed/EvaluationResult 타입에 맞춤 |
| `lib.rs`에서 모듈 활성화 | `ouroboros/src/lib.rs` | 주석 해제 |
| `evolve()`에 lateral/regression 통합 | `ouroboros_engine.rs` | stagnation 감지 + regression 주입 |
| 컴파일 확인 | — | `cargo build -p oxios-ouroboros` |

### Phase 4: 검증 + 정리 (1일)

| 작업 | 비고 |
|------|------|
| 단위 테스트: `should_evaluate()` 판단 로직 | acceptance_criteria 유무, output_schema 유무 |
| 단위 테스트: `run_evolution_loop()` 1차 통과 | evolution 진입 없이 반환 |
| 단위 테스트: `run_evolution_loop()` 1회 evolve | mock evaluate/evolve |
| 단위 테스트: best-result 하락 시 이전 결과 반환 | evolve 후 점수 하락 케이스 |
| 단위 테스트: `max_evolution_iterations = 0` | evaluate만, evolution 없음 |
| 통합 테스트: complex task가 evolution 루프 진입 | acceptance criteria 포함 |
| 통합 테스트: simple task는 evolution 스킵 | LLM 호출 수 변화 없음 |
| 성능 테스트: simple task 지연 변화 측정 | 추가 LLM 호출 0회 확인 |
| 기존 테스트 전체 통과 | `cargo test --workspace` |

---

## 5. 영향 범위

| 파일 | 변경 | 비고 |
|------|------|------|
| `ouroboros_engine.rs` | `#[allow(dead_code)]` 제거, evolve에 lateral/regression 통합 | 핵심 |
| `orchestrator.rs` | `run_evolution_loop()`, `should_evaluate()`, `execute_seed()`, `EvolutionConfig` 추가, `handle_message()` 확장 | 핵심 |
| `config.rs` | `OrchestratorConfig` 필드 3개 추가, manual Default impl | 설정 |
| `default-config.toml` | `eval_cache_enabled = true` 추가 (기존 필드 유지) | 설정 |
| `event_bus.rs` | `KernelEvent` variant 2개 추가 + audit 매핑 | 관찰성 |
| `ouroboros/src/lib.rs` | `lateral`/`regression` 모듈 주석 해제 | 확장 |
| `ouroboros/src/lateral.rs` | git history에서 복원 + API 마이그레이션 | 확장 |
| `ouroboros/src/regression.rs` | git history에서 복원 + API 마이그레이션 | 확장 |

**변경 없음:**
- `evaluation.rs` — 이미 완전 구현됨, 그대로 사용
- `eval_cache.rs` — 이미 구현됨, 그대로 사용
- `protocol.rs` — trait 시그니처 변경 없음
- `seed.rs` — struct 변경 없음
- `interview.rs` — 변경 없음
- `degraded.rs` — 변경 없음
- Frontend — Web은 이미 `evaluation_passed`를 표시

---

## 6. 비용 분석

### LLM 호출 수

| 작업 유형 | 현재 | 변경 후 | 증가 |
|-----------|------|---------|------|
| Simple task (from_message) | 1-2회 | 1-2회 (변화 없음) | 0 |
| Simple task + criteria 없음 | 2-3회 | 2-3회 (변화 없음) | 0 |
| Complex task (1차 통과) | 3-4회 | 3-4회 + 평가 0-1회 | +0~1 |
| Complex task (1회 evolve) | 3-4회 | 3-4회 + 평가 1회 + evolve 1회 + 재실행 1회 + 재평가 1회 | +3~4 |
| Complex task (2회 evolve) | 3-4회 | 위 + evolve 1회 + 재실행 1회 + 재평가 1회 | +6~7 |
| Multi-agent (criteria ≥ 5) | N×2-3회 | N×2-3회 (변화 없음) | 0 |

**완화:**
- `should_evaluate()`로 criteria 없는 task는 전혀 영향 없음
- Multi-agent 경로는 전혀 영향 없음
- `max_evolution_iterations = 3`으로 상한 제한 (기본값)
- 기계적 평가가 완벽 통과하면 LLM 평가 스킵 (기존 evaluate 구현에 이미 있음)
- `eval_cache`로 동일 seed/result 재평가 방지

### 지연

| 작업 유형 | 현재 | 변경 후 |
|-----------|------|---------|
| Simple task | ~5초 | ~5초 (변화 없음) |
| Complex (1차 통과) | ~15초 | ~18초 (평가 1회) |
| Complex (1회 evolve) | ~15초 | ~40초 (평가+evolve+재실행+재평가) |
| Multi-agent | ~N×10초 | ~N×10초 (변화 없음) |

---

## 7. 위험 및 완화

| 위험 | 확률 | 영향 | 완화 |
|------|------|------|------|
| Evolution 루프가 비용 폭증 | 중간 | 높음 | `max_iterations = 3` 상한 + simple task 스킵 + multi-agent 스킵 |
| Evolve가 악화시킴 (점수 하락) | 중간 | 중간 | **best-result 추적**: 하락 시 이전 최고 결과 반환 |
| 기계적 평가가 너무 관대해 evolution 스킵 | 낮음 | 낮음 | `min_evaluation_score` 조정 가능 |
| lateral/regression 모듈 미완성 | 중간 | 낮음 | Phase 3에서 컴파일 + 테스트 검증, 필요시 disabled |
| 설정 필드명 불일치 | 제거됨 | — | v2에서 기존 TOML 필드명 존중 |
| OuroborosProtocol trait 시그니처 변경 | 제거됨 | — | v2에서 execute() 변경하지 않음 |

---

## 8. v1 → v2 변경 요약

| # | v1 문제 | v2 해결 |
|---|---------|---------|
| 1 | `should_evaluate()`가 존재하지 않는 `Complexity` enum 사용 | Seed의 `acceptance_criteria.is_empty()` + `output_schema`로 판단 |
| 2 | `execute()`를 콜백 기반으로 리팩토링 → trait 시그니처 변경 | execute() 변경하지 않음. Orchestrator가 실행 소유 |
| 3 | config 필드명이 기존 TOML과 충돌 (`eval_score_threshold`) | 기존 필드명 `min_evaluation_score` 존중 |
| 4 | `with_config()`에서 `_config` 무시 문제 미해결 | `_` prefix 제거, `EvolutionConfig`로 변환하여 저장 |
| 5 | lateral/regression을 "주석 해제"로 서술 (실제로는 파일 삭제됨) | git history 복원 + API 마이그레이션으로 정정 |
| 6 | best-result 보관이 위험 테이블에만 언급, 설계에 없음 | `run_evolution_loop()`에 best-result 추적 로직 명시 |
| 7 | 새 `OrchestratorEvent` enum 제안 → 이벤트 시스템 중복 | 기존 `KernelEvent`에 variant 추가로 통일 |
| 8 | Multi-agent 경로와의 상호작용 미정의 | single-agent 경로에만 적용 명시, multi-agent는 불변 |

---

## 9. 성공 기준

- [ ] `OrchestratorConfig`에 3개 필드가 추가되고 `default-config.toml`과 일치
- [ ] `with_config()`가 설정을 무시하지 않고 `EvolutionConfig`로 저장
- [ ] `#[allow(dead_code)]`가 ouroboros_engine.rs의 evaluate/evolve에서 제거됨
- [ ] Complex task가 3단계 평가(mechanical → semantic → consensus)를 통과
- [ ] 평가 미달 시 자동으로 Seed 개선 + 재실행 수행
- [ ] Evolution이 점수를 악화시키면 이전 best 결과 반환
- [ ] Simple task는 기존과 동일한 성능 (LLM 호출 수·지연 변화 없음)
- [ ] Multi-agent 경로는 기존과 동일하게 동작 (변화 없음)
- [ ] `max_evolution_iterations = 0` 설정 시 evolution 비활성화 (evaluate만 수행)
- [ ] `lateral.rs`/`regression.rs`가 git history에서 복원되어 컴파일됨
- [ ] `KernelEvent`에 `EvolutionStarted`, `EvolutionMaxReached` 추가됨
- [ ] 기존 테스트 전체 통과 (`cargo test --workspace`)
