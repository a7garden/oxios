# RFC: Ouroboros 다듬기

## 동기

시나리오 테스트 결과 Ouroboros Interview의 **핵심 분류 능력은 잘 동작**한다.

```
"이거 좀 고쳐줘"        → ambiguity 0.790 → BLOCK ✅
"앱이 느려"              → ambiguity 0.790 → BLOCK ✅
"로그인 페이지 수정해줘"  → ambiguity 0.520 → BLOCK ✅
"cargo test 실행해줘"     → ambiguity 0.170 → PASS  ✅
"안녕"                   → CHAT            ✅
```

하지만 실제 동작에서 5가지 문제가 발견되었다. 각각의 심각도와 수정 범위가 다르다.

---

## 문제 1: 명확한 요청에도 항상 질문 3개 생성

### 현상

```
입력: "src/main.rs에서 greet 함수를 hello로 이름 변경해줘"
결과: ambiguity = 0.110, ready = true  ← 통과는 함
      질문 3개도 같이 생성됨            ← 불필요
      → LLM 토큰 낭비 + 레이턴시 증가
```

### 원인

프롬프트가 "Up to 3 Socratic clarifying questions"를 항상 요청한다.
LLM은 ambiguity가 낮아도 성실하게 3개를 만든다.

### 수정

**파일**: `ouroboros_engine.rs` — interview 프롬프트

```
BEFORE:
  "questions": Up to 3 Socratic clarifying questions. Empty array when is_task=false.

AFTER:
  "questions": Socratic clarifying questions for ambiguities ONLY.
               Return EMPTY ARRAY [] if all three scores are 0.8+.
               Only ask about dimensions where clarity is below 0.7.
               Maximum 3 questions.
```

orchestrator 쪽은 수정 불필요 — 이미 `ready_for_seed` 체크 후 질문을 버림.

### 영향

- LLM 토큰 절약 (명확한 요청에서 질문 생성 안 함)
- 레이턴시 감소 (생성할 텍스트가 줄어듦)
- 기능적 변화 없음 (ready=true면 질문이 사용자에게 안 보임)

---

## 문제 2: Interview 최대 라운드 제한 없음

### 현상

```rust
// orchestrator.rs — interview follow-up 루프에 상한이 없음
// session이 존재하면 계속 interview를 반복
self.ouroboros.interview(&multi_turn_context).await?
```

사용자가 계속 모호하게 대답하면 interview가 무한히 반복된다.

### 수정

**파일**: `interview.rs` — InterviewResult에 round 추가

```rust
pub struct InterviewResult {
    // 기존 필드 ...
    
    /// 현재 인터뷰 라운드 (1부터 시작).
    #[serde(default)]
    pub round: u32,
}
```

**파일**: `orchestrator.rs` — 세션에 max_rounds 추가

```rust
struct InterviewSession {
    // 기존 필드 ...
    round: u32,
}

// handle_message 내부 follow-up 분기:
if session.round >= 3 {
    // 3라운드 넘으면 정리 제안 후 강제 진행
    return Ok(OrchestrationResult {
        response: format!(
            "지금까지 들은 내용을 정리하면:\n\
             • 목표: {}\n\
             이대로 진행할까요? 더 자세히 알려주셔도 됩니다.",
            session.interview.original_message
        ),
        phase_reached: Phase::Interview,
        // 특별 플래그 또는 ready_for_seed = true 처리
        ..
    });
}
```

**파일**: `OrchestratorConfig`에 설정 추가

```rust
pub struct OrchestratorConfig {
    // 기존 ...
    
    /// Interview 최대 라운드 (기본 3).
    #[serde(default = "default_max_interview_rounds")]
    pub max_interview_rounds: u32,
}
```

### 영향

- 무한 대화 방지
- 사용자 경험 개선 (3번 질문 후에는 정리해서 제안)
- 새 설정값은 config.toml에서 조정 가능

---

## 문제 3: Seed가 사용자 원문의 디테일을 유실할 위험

### 현상

```
사용자: "components/auth/LoginButton.tsx에서 handleSubmit 안에 
         네트워크 에러 핸들링 추가해줘. 토스트 메시지로 보여주고,
         retry 버튼도 넣어줘"

LLM Seed: { 
  goal: "Add error handling to LoginButton component",
  acceptance_criteria: ["Error handling added", "Toast messages shown"]
}
// → 파일 경로, 함수명, retry 버튼 요구사항이 사라짐
```

### 수정

**파일**: `seed.rs` — Seed에 원문 보존 필드 추가

```rust
pub struct Seed {
    // 기존 필드 ...
    
    /// 인터뷰에서 수집된 사용자 원문 (그대로 보존).
    /// Seed 생성 시 반드시 채워져야 함.
    #[serde(default)]
    pub original_request: String,
}
```

**파일**: `ouroboros_engine.rs` — generate_seed에서 원문 저장

```rust
let seed = Seed {
    // 기존 ...
    original_request: interview.original_message.clone(),
};
```

**파일**: `agent_runtime.rs` — system prompt에 원문 주입

현재 agent_runtime은 seed.goal을 기반으로 에이전트를 실행한다.
seed.original_request가 있으면 이것도 system prompt에 포함:

```rust
// 에이전트 실행 시 system prompt 구성
if !seed.original_request.is_empty() {
    context_block.push_str(&format!(
        "\n\n## 사용자 원본 요청 (이 내용을 그대로 반영하세요)\n{}",
        seed.original_request
    ));
}
```

### 검증 (선택)

generate_seed 후 간단한 디테일 보존 체크를 추가할 수 있다:

```rust
// Seed 생성 후, 원문의 핵심 명사구가 goal에 포함되어 있는지 확인
// 파일 경로, 함수명 등이 goal에 없으면 warn 로그
fn check_detail_preservation(original: &str, seed_goal: &str) {
    // 파일 경로 패턴 (xxx.yyy) 추출
    // 함수명 패턴 (snake_case) 추출
    // 각각이 seed_goal에 존재하는지 확인
    // 누락 시 tracing::warn
}
```

이건 선택사항 — 실제 문제가 발생하면 추가.

### 영향

- 사용자가 말한 구체적 디테일이 실행까지 보존됨
- 기존 데이터 호환 (original_request 기본값 = "")

---

## 문제 4: Evolve가 사용자 확인 없이 자동 재실행

### 현상

```rust
// orchestrator.rs — evolve 루프
while !current_evaluation.all_passed()
   && current_evaluation.score < 0.8
   && iterations < 3  // 최대 3회 자동 재실행
{
    // LLM이 새 seed 생성 → 자동 재실행 → 자동 재평가
}
```

3회까지 **사용자 모르게** 재실행된다.
비용(토큰), 시간, 부작용(파일 수정)이 모두 발생한다.

### 수정

2가지 옵션:

#### 옵션 A: Evolve 라운드를 interview처럼 사용자에게 돌려보내기

```rust
// 첫 실행 실패 시
if !evaluation.all_passed() {
    return Ok(OrchestrationResult {
        response: format!(
            "작업을 시도했지만 완전히 성공하지 못했습니다.\n\
             평가 점수: {:.0}%\n\n\
             실패 이유:\n{}\n\n\
             다시 시도할까요? 아니면 요청을 수정해주시겠어요?",
            evaluation.score * 100.0,
            evaluation.notes.join("\n")
        ),
        phase_reached: Phase::Evaluate,
        evaluation_passed: false,
        // session 유지 → 사용자가 "다시 시도해줘" 하면 evolve 진행
    });
}
```

#### 옵션 B: 자동 1회 + 이후 사용자 확인

```rust
// 첫 evolve는 자동 (빠른 복구 케이스)
// 두 번째부터는 사용자에게 물어봄
if iterations >= 1 {
    return OrchestrationResult {
        response: format!(
            "재시도했지만 여전히 완전하지 않습니다 (점수: {:.0}%).\n\
             계속 시도할까요?",
            current_evaluation.score * 100.0
        ),
        ..
    };
}
```

**권장**: 옵션 A. 사용자가 "툭" 던지는 패턴이므로,
실패한 사실을 명확히 알려주고 사용자가 결정하게 하는 것이 낫다.
빠른 복구는 AgentRuntime 레벨에서 이미 tool retry로 처리된다.

### 영향

- 예측 불가능한 자동 재실행 제거
- 토큰 비용 절감 (최악의 경우 3회 × LLM 3회 = 9회 호출 절약)
- 사용자 신뢰도 향상

---

## 문제 5: Evaluation 프롬프트가 너무 관대함

### 현상

```
EVALUATE_SYSTEM_PROMPT:
  "Score 0.8+ if the task was clearly accomplished"
  "Be GENEROUS — if the output plausibly shows the task was done, mark it as passed."
```

Interview의 "generous"는 constraint/criteria가 낮아서 문제가 없었지만,
Evaluation의 "generous"는 직접적으로 평가 결과에 영향을 미친다.
Evolve 루프의 진입 조건이 `score < 0.8`이므로, generous하게 0.8을 주면
**실패한 실행도 통과**시킨다.

### 수정

**파일**: `ouroboros_engine.rs` — EVALUATE_SYSTEM_PROMPT

```
BEFORE:
  Score 0.8+ if the task was clearly accomplished even if minor details differ.
  Be GENEROUS — if the output plausibly shows the task was done, mark it as passed.

AFTER:
  SCORING:
  - 1.0: All acceptance criteria explicitly satisfied in the output
  - 0.8-0.9: Core goal achieved, minor gaps in secondary criteria
  - 0.5-0.7: Partially done, significant gaps remain
  - Below 0.5: Task failed or no useful output
  
  Score based on EVIDENCE, not plausibility.
  If the output doesn't explicitly show a criterion was met, score it as not met.
```

### 영향

- 평가가 더 정확해짐
- Evolve 루프 진입이 더 정확하게 트리거됨
- 하지만 문제 4를 적용하면(자동 evolve 제거) 이것의 영향은 줄어듦

---

## 수정 우선순위

| # | 문제 | 난이도 | 영향 | 순서 |
|---|------|--------|------|------|
| 1 | 명확한 요청 질문 생성 억제 | 쉬움 — 프롬프트만 수정 | 토큰 절약 | **1순위** |
| 2 | Interview 최대 라운드 | 중간 — 설정+로직 | 안정성 | **2순위** |
| 3 | Seed 원문 보존 | 중간 — 필드+로직 | 정확도 | **3순위** |
| 4 | Evolve 사용자 확인 | 중간 — 흐름 변경 | 비용+신뢰 | **4순위** |
| 5 | Evaluation 관대함 수정 | 쉬움 — 프롬프트만 수정 | 정확도 | **5순위** |

## 수정하지 않는 것

- **AmbiguityScore 가중치** (goal 40% / constraint 30% / criteria 30%):
  시나리오 테스트에서 잘 동작함. 변경 불필요.

- **MechanicalEvalResult의 키워드 매칭**:
  의미 있는 기여를 하지 않지만, mechanical 평가가 통과하면 LLM 호출을 건너뛰는
  최적화 역할을 한다. 제거하면 LLM 평가 호출이 늘어남.

- **5-phase 구조 자체**:
  Interview → Seed → Execute → Evaluate → Evolve는 이 사용자 패턴에 적합.
  구조 변경 없이 각 phase의 동작만 다듬는다.

- **task/chat 분류 로직**:
  11/11 시나리오에서 정확. 건드리지 않음.
