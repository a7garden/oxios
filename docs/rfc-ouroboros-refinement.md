# RFC: Ouroboros 다듬기 (검증 완료)

## 검증 방법

- 시나리오 테스트: 실제 LLM (zai/glm-5-turbo)로 11개 시나리오 인터뷰 실행
- 실시드 분석: `~/.oxios/workspace/seeds/`의 실제 Seed JSON 검사
- 코드 추적: orchestrator → gateway → channel 전체 흐름 분석

---

## 최종 수정안 (5개 → 3개로 축소)

### ❌ DROP: 명확한 요청 질문 생성 억제 (원 1순위)

**이유**: ready=true인 질문은 orchestrator에서 사용자에게 절대 전달되지 않음.
추가 토큰 ~50개로 무시 가능. 프롬프트 조건부 지시는 LLM에서 불확실.
코드에서 `questions.clear()`해도 이미 생성된 토큰 비용은 절약 안 됨.
**실제 영향 없는 비문제(non-issue).**

---

### ❌ DROP: Evaluation "관대함" 수정 (원 5순위)

**이유**: Evolve 루프를 제거하면 evaluation은 정보 제공 목적으로만 사용됨.
"관대한" 평가가 사용자에게 "~80% 달성"으로 표시되는 건 합리적.
엄격하게 만들면 false negative 증가 → 사용자가 불필요하게 재시도.
**Evolve 제거 후에는 현재 방식이 더 적합.**

---

### ✅ KEEP 1: Interview 최대 라운드 (보험)

**검증 결과**: 
- 대부분의 인터뷰는 1-2라운드에 해결
- LLM이 conversation_history를 보고 점점 더 높은 점수를 줌
- 무한 루프 실제 위험은 낮음

**하지만 추가하는 이유**: 
- 비용이 매우 낮음 (설정 1개, 체크 3줄)
- pathological user에 대한 방어
- config.toml에서 조정 가능

**수정 범위**:

```
InterviewSession에 round: u32 추가
OrchestratorConfig에 max_interview_rounds: u32 추가 (기본 3)
orchestrator follow-up 분기에서 round 체크
초과 시: "지금까지 이해한 내용으로 진행할게요" + ready_for_seed = true 처리
```

---

### ✅ KEEP 2: Seed에 사용자 원문 보존

**검증 결과**:
- 실제 Seed 데이터: 구체적 요청은 디테일이 잘 보존됨 ✅
- 하지만 한국어 원문이 영어로 번역되어 Seed에 저장됨 ⚠️
- Parse 실패 시 "Task from user input"으로 디테일 완전 상실 ⚠️
- Seed는 저장만 되고 다시 로드되지 않으므로 `#[serde(default)]`로 호환성 OK

**수정 범위**:

```
seed.rs: original_request: String 필드 추가 (#[serde(default)])
ouroboros_engine.rs: generate_seed에서 interview.original_message 저장
agent_runtime.rs: build_system_prompt에 original_request 주입
```

agent_runtime 수정:
```rust
// build_system_prompt 함수 내, goal 직후에 추가:
if !seed.original_request.is_empty() && seed.original_request != seed.goal {
    prompt.push_str(&format!(
        "\n## 사용자 원본 요청\n{}\n",
        seed.original_request
    ));
}
```

---

### ✅ KEEP 3: Evolve 루프 제거

**검증 결과** (실제 데이터 기반):

| Seed | Generation | 결과 |
|------|-----------|------|
| "Clarify file location" | gen 1→2→3 | 3회 진화 후에도 동일한 모호한 goal |
| "Create benchmark_test.txt" | gen 1→2→3 | 명확한 task인데 3회 진화 → 불필요 |
| "Task from user input" | gen 0→1 | parse 실패 seed → 진화해도 여전히 깨짐 |

**추가 발견 — 임계값 버그**:
```
Evolve 진입: score < 0.8 AND !all_passed
최종 통과:   score >= 0.7 OR  all_passed

score 0.75 → evolve 3회 실행 → 여전히 0.75 → 통과(score >= 0.7)
= 9회 LLM 호출을 낭비하고 결과는 같음
```

**수정 범위**:

```
orchestrator.rs: 
  - Phase 5 (Evolve) while 루프 전체 제거 (~60줄)
  - evaluate 결과를 사용자에게 직접 보고
  - Evolve 관련 코드 제거 (evolve 호출, re-execute, re-evaluate)
  
ouroboros_engine.rs:
  - evolve() 메서드 유지 (트레이트 구현체이므로) 
  - 하지만 orchestrator에서 호출하지 않음
  
OrchestratorConfig:
  - max_evolution_iterations 제거
  - min_evaluation_score 제거
```

최종 결과 반환 로직:
```rust
// Evaluate 완료 후 바로 결과 반환
let passed = current_evaluation.all_passed();

Ok(OrchestrationResult {
    response: if passed {
        format!("✅ {}\n\n{}", seed.goal, format_evaluation_notes(&evaluation))
    } else {
        format!(
            "⚠️ '{}'을(를) 시도했지만 완전히 성공하지 못했습니다.\n점수: {:.0}%\n\n{}",
            seed.goal,
            current_evaluation.score * 100.0,
            format_evaluation_notes(&current_evaluation)
        )
    },
    evaluation_passed: passed,
    ..
})
```

---

## 수정하지 않는 것

| 항목 | 이유 |
|------|------|
| 5-phase 구조 | Interview → Seed → Execute → Evaluate는 유지. Evolve만 제거 |
| AmbiguityScore 가중치 | 시나리오 테스트에서 완벽 동작 |
| task/chat 분류 | 11/11 정확 |
| Interview "Be generous" | goal만 generous, constraint/criteria는 엄격 → overall 잘 차단 |
| Evaluation "Be generous" | Evolve 제거 후 정보 제공 목적으로 적합 |
| MechanicalEvalResult | LLM 호출 스킵 최적화로 기여 |

## 수정 전후 비용 비교

```
Before (모호한 요청, 1회 인터뷰, 1회 진화 실패):
  Interview: 2회 LLM  (초기 + follow-up)
  Seed:      1회 LLM
  Execute:   1회 AgentRuntime (N회 tool calls)
  Evaluate:  1-2회 LLM (mechanical 실패시)
  Evolve:    1회 LLM
  Re-exec:   1회 AgentRuntime (N회 tool calls)
  Re-eval:   1-2회 LLM
  Total:     8-10회 LLM + 2회 AgentRuntime

After:
  Interview: 2회 LLM
  Seed:      1회 LLM
  Execute:   1회 AgentRuntime (N회 tool calls)
  Evaluate:  1-2회 LLM
  Total:     4-5회 LLM + 1회 AgentRuntime
  
절약: 4-5회 LLM 호출 + 1회 AgentRuntime (50% 절감)
```
