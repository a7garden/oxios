# RFC-019: Ouroboros Dead Code 정리

> **상태:** 📝 설계
> **날짜:** 2026-05-26
> **우선순위:** P2
> **범위:** `crates/oxios-ouroboros/`, `crates/oxios-kernel/src/orchestrator.rs`
> **선행:** 없음
> **후행:** 없음

---

## 1. 동기

Ouroboros 프로토콜은 5단계(Interview → Seed → Execute → Evaluate → Evolve)로 설계되었으나, 실제로는 2.5단계만 연결되어 있다:

### 설계 vs 구현

```
프로토콜 trait (ouroboros_engine.rs):
  interview()    → ✅ 연결됨
  generate_seed() → ✅ 연결됨
  execute()       → ❌ #[allow(dead_code)]
  evaluate()      → ❌ #[allow(dead_code)]
  evolve()        → ❌ #[allow(dead_code)]

실제 흐름 (orchestrator.rs):
  handle_message()
    → interview LLM 호출 (✅)
    → Seed::from_message() 또는 generate_seed() LLM 호출 (✅)
    → AgentLifecycleManager::spawn_and_run() (✅ — 하지만 trait 아님)
    → 간단한 인라인 평가 (⚠️ — trait의 3단계 평가 아님)
    → evolve 루프 (❌ — config에 max_evolution_iterations=3 있지만 미사용)
```

**구체적 dead code:**

| 파일 | 대상 | `#[allow(dead_code)]` | 설명 |
|------|------|----------------------|------|
| `ouroboros_engine.rs` | `OuroborosEngine::execute()` | ✅ | AgentRuntime 실행 래퍼 |
| `ouroboros_engine.rs` | `OuroborosEngine::evaluate()` | ✅ | 3단계 평가 (mechanical → semantic → consensus) |
| `ouroboros_engine.rs` | `OuroborosEngine::evolve()` | ✅ | Seed 개선 + 재실행 루프 |
| `orchestrator.rs` | `DelegationConfig::timeout_ms` | ✅ | 위임 타임아웃 설정값 |
| `orchestrator.rs` | `InterviewSession` 필드 전체 | ✅ (unused) | 인터뷰 세션 상태 |

**주석처리된 파일 (존재하지 않음):**
- `ouroboros/src/lateral.rs` — 주석에서 참조됨
- `ouroboros/src/regression.rs` — 주석에서 참조됨

---

## 2. 설계

### 옵션 A: Dead Code 제거 + Trait 축소

프로토콜을 현재 실제 구현에 맞게 축소:

```rust
// 변경 전: ouroboros protocol trait
#[async_trait]
pub trait OuroborosProtocol: Send + Sync {
    async fn interview(&self, prompt: &str) -> Result<InterviewResult>;
    async fn generate_seed(&self, prompt: &str, interview: &InterviewResult) -> Result<Seed>;
    async fn execute(&self, seed: &Seed) -> Result<ExecutionResult>;       // dead
    async fn evaluate(&self, seed: &Seed, result: &ExecutionResult) -> Result<EvaluationResult>; // dead
    async fn evolve(&self, seed: &Seed, eval: &EvaluationResult) -> Result<Seed>; // dead
}

// 변경 후: 실제 구현에 맞춘 축소 trait
#[async_trait]
pub trait OuroborosProtocol: Send + Sync {
    /// 사용자 프롬프트를 분류 (task vs chat) + 복잡도/모호성 평가
    async fn interview(&self, prompt: &str) -> Result<InterviewResult>;

    /// 인터뷰 결과를 구조적 Seed 스펙으로 변환
    async fn generate_seed(&self, prompt: &str, interview: &InterviewResult) -> Result<Seed>;
}
```

**execute/evaluate/evolve는 Orchestrator가 직접 관리** (trait 밖으로 이동):

```rust
// orchestrator.rs — 명시적 흐름

impl Orchestrator {
    pub async fn handle_message(&self, channel: &str, content: &str, meta: HashMap<String, String>) -> Result<OrchestrationResult> {
        // Phase 1: Interview
        let interview = self.protocol.interview(content).await?;

        if interview.is_chat {
            return Ok(OrchestrationResult::chat(interview.response));
        }

        // Phase 2: Seed
        let seed = if interview.is_simple() {
            Seed::from_message(content)?
        } else {
            self.protocol.generate_seed(content, &interview).await?
        };

        // Phase 3: Execute (Orchestrator가 직접 — Ouroboros trait 아님)
        let result = self.lifecycle.spawn_and_run(seed).await?;

        // Phase 4: Evaluate (간단한 인라인 평가 — 나중에 확장 가능)
        let evaluation = self.evaluate_result(&seed, &result).await?;

        // Phase 5: Evolve — TODO: 향후 구현
        // if !evaluation.passed && evolution_count < max_iterations {
        //     let improved_seed = self.evolve_seed(&seed, &evaluation).await?;
        //     ... loop back to Phase 3
        // }

        Ok(OrchestrationResult::from(seed, result, evaluation))
    }
}
```

### 옵션 B: Full Protocol 활성화

5단계를 모두 연결:

```rust
impl Orchestrator {
    pub async fn handle_message(&self, ...) -> Result<OrchestrationResult> {
        let interview = self.protocol.interview(content).await?;
        // ...
        let seed = self.protocol.generate_seed(content, &interview).await?;

        // Evolution loop
        let mut current_seed = seed;
        for iteration in 0..self.config.max_evolution_iterations {
            let result = self.protocol.execute(&current_seed).await?;
            let eval = self.protocol.evaluate(&current_seed, &result).await?;

            if eval.passed() {
                return Ok(OrchestrationResult::from(current_seed, result, eval));
            }

            if iteration < self.config.max_evolution_iterations - 1 {
                current_seed = self.protocol.evolve(&current_seed, &eval).await?;
            }
        }
        // 최대 반복 도달, 마지막 결과 반환
    }
}
```

**옵션 B의 문제:** 단순 작업에도 4-12회 LLM 호출 (비용/지연 급증).

### 옵션 C: 하이브리드 (권장)

- **심플 패스:** Interview → Seed::from_message() → Execute → inline evaluate → 완료 (2-3 LLM 호출)
- **풀 패스:** 복잡한 작업에만 evolution loop 활성화

```rust
match interview.complexity {
    Complexity::Simple => {
        // 빠른 경로: Interview + ad-hoc Seed + Execute + 기계적 평가
        let seed = Seed::from_message(content)?;
        let result = self.lifecycle.spawn_and_run(seed).await?;
        let eval = self.mechanical_evaluate(&seed, &result);
        Ok(OrchestrationResult::from(seed, result, eval))
    }
    Complexity::Complex => {
        // 전체 경로: Interview + LLM Seed + Execute + 3단계 평가 + 선택적 evolve
        let seed = self.protocol.generate_seed(content, &interview).await?;
        self.run_evolution_loop(seed).await
    }
}
```

---

## 3. 권장안: 옵션 A + C 하이브리드

### 3.1 단계별 작업

#### Step 1: Dead Code 정리

| 작업 | 파일 |
|------|------|
| `OuroborosEngine::execute()` 제거 | `ouroboros_engine.rs` |
| `OuroborosEngine::evaluate()` 제거 | `ouroboros_engine.rs` |
| `OuroborosEngine::evolve()` 제거 | `ouroboros_engine.rs` |
| `OuroborosProtocol` trait에서 execute/evaluate/evolve 제거 | `protocol.rs` |
| `DelegationConfig::timeout_ms` dead code 정리 | `orchestrator.rs` |
| `InterviewSession` unused 필드 정리 | `orchestrator.rs` |

#### Step 2: Evolution 인프라 보존

dead code를 삭제하되, **evolution을 위한 설계 문서는 별도로 보존**:

```
docs/
└── rfc-ouroboros-evolution.md   ← execute/evaluate/evolve 설계 의도 문서화
                                      (코드가 아닌 마크다운으로)
```

이 문서에는:
- 3단계 평가 설계 (mechanical → semantic → consensus)
- Evolve 전략 (seed refinement, lateral thinking, regression)
- 활성화 조건 (복잡도 ≥ threshold, evaluation score < 0.8)
- 비용/지연 추정치

#### Step 3: Config 정리

```toml
# config.toml

[orchestrator]
# evolution_iterations = 3   ← 제거 또는 주석 처리
# eval_score_threshold = 0.8  ← 제거 또는 주석 처리

# 향후 evolution 활성화 시:
# [orchestrator.evolution]
# max_iterations = 3
# score_threshold = 0.8
# enabled = false  # true로 설정하면 복잡 작업에 evolution 활성화
```

#### Step 4: 평가 모듈 재설계 준비

현재 `evaluation.rs`의 3단계 평가는 좋은 설계다. dead code로 두지 말고 **평가만 독립 모듈로 추출**:

```rust
// tools/evaluation.rs (신규 — 나중에 구현)
pub struct ResultEvaluator {
    kernel: Arc<KernelHandle>,
}

impl ResultEvaluator {
    /// 기계적 평가: 완료 여부, 산출물 존재, 에러 여부
    pub fn mechanical(&self, seed: &Seed, result: &ExecutionResult) -> EvaluationScore {
        let mut score = 0.0;
        if result.success { score += 0.3; }
        if self.check_acceptance_criteria(seed, result) { score += 0.4; }
        if self.check_output_artifacts(seed, result) { score += 0.3; }
        EvaluationScore { score, details: vec![] }
    }

    /// 의미 평가: LLM으로 결과 품질 평가 (선택적)
    pub async fn semantic(&self, seed: &Seed, result: &ExecutionResult) -> Result<EvaluationScore> {
        // 향후 구현
        todo!("LLM 기반 결과 품질 평가")
    }
}
```

---

## 4. 마이그레이션 계획

### Phase 1: 정리 (1일)

| 작업 | 비고 |
|------|------|
| Dead code 제거 (execute/evaluate/evolve) | `ouroboros_engine.rs` |
| Trait 축소 | `protocol.rs` |
| `#[allow(dead_code)]` 제거 | `orchestrator.rs` |
| `InterviewSession` 정리 | `orchestrator.rs` |
| Evolution 설계 문서화 | `docs/rfc-ouroboros-evolution.md` |

### Phase 2: Config 정리 (0.5일)

| 작업 | 비고 |
|------|------|
| 사용하지 않는 config 필드 주석 처리 | `config.rs`, `default-config.toml` |
| `tool_names()` 카운트 수정 (24→26) | `kernel_bridge.rs` |
| Capability 추론 중복 제거 | `orchestrator.rs` + `agent_lifecycle.rs` → 공유 유틸 |

### Phase 3: 평가 모듈 기반 (선택, 1일)

| 작업 | 비고 |
|------|------|
| `ResultEvaluator` 구조 설계 | `tools/evaluation.rs` |
| 기계적 평가 구현 | Orchestrator 기존 인라인 로직 이관 |
| 의미 평가 TODO | 향후 구현 |

---

## 5. 영향 범위

| 파일 | 변경 |
|------|------|
| `ouroboros_engine.rs` | execute/evaluate/evolve 제거 |
| `protocol.rs` | trait 축소 |
| `orchestrator.rs` | dead code 정리, inline 평가 유지 |
| `agent_lifecycle.rs` | capability 추론 공유 유틸 사용 |
| `config.rs` | evolution 필드 정리 |
| `kernel_bridge.rs` | tool_names 카운트 수정 |
| `docs/` | evolution 설계 문서 신규 |

---

## 6. 위험 및 완화

| 위험 | 완화 |
|------|------|
| Evolution 기능이 필요해질 때 재구현 | 설계 문서(`rfc-ouroboros-evolution.md`)에 상세 기록 |
| 기존 evaluation.rs의 좋은 설계가 손실 | 독립 모듈(`ResultEvaluator`)로 추출 |
| 평가 품질 저하 | 기계적 평가는 기존과 동일, 의미 평가는 애초에 미구현 |

---

## 7. 성공 기준

- [ ] `#[allow(dead_code)]`가 ouroboros/orchestrator에서 제거됨
- [ ] `OuroborosProtocol` trait이 interview + generate_seed만 포함
- [ ] 모든 작업이 2-3회 LLM 호출 내에 완료 (simple 기준)
- [ ] Evolution 설계 의도가 마크다운 문서로 보존됨
- [ ] `tool_names()`가 실제 등록 도구 수(26)와 일치
- [ ] 기존 테스트 전체 통과
