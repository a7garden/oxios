# RFC: Ouroboros 다듬기 — 최종 검증 완료

## 검증 결과: 설계에서 수정한 것

| 원래 제안 | 검증 후 변경 | 이유 |
|-----------|-------------|------|
| Seed → TaskSpec rename | **Seed 유지, 생성 경로만 추가** | rename은 19곳 변경, 위험만 증가 |
| Classify 모듈 추가 | **Interview에 complexity 필드 추가** | 같은 LLM 호출에 필드 1개 추가로 끝 |
| ontology 제거 | **유지하되 새로 생성 안 함** | 기존 62개 Seed에 데이터 있음 |
| 3-tier 분리 (Classify + Interview) | **Interview에 통합** | 프롬프트/스키마 1개 관리, 기존 테스트 활용 |

## 실제 변경 (4개)

### 1. Interview에 complexity 판단 추가

**파일**: `ouroboros_engine.rs` — InterviewResponse

```rust
#[derive(Debug, Deserialize)]
struct InterviewResponse {
    is_task: bool,
    chat_response: String,
    questions: Vec<String>,
    scores: Option<AmbiguityScores>,

    // 새로 추가
    /// "simple": 명확한 1회성 요청 (날씨, 알람, 검색, 계산, 간단한 명령)
    /// "complex": 모호하거나 다단계 작업 (수정, 작성, 배포, 분석)
    #[serde(default = "default_complexity")]
    complexity: String,
}

fn default_complexity() -> String {
    "complex".to_string()  // 모르면 complex (안전)
}
```

**프롬프트에 추가**:
```
- "complexity": "simple" for clear single-action requests (check weather,
  set alarm, search, calculate, simple file operations). "complex" for 
  ambiguous or multi-step tasks. Default to "complex" when unsure.
```

### 2. Orchestrator에 complexity 기반 라우팅

**파일**: `orchestrator.rs` — handle_message

```rust
// Interview 결과 받은 후
if result.is_task {
    match result.complexity.as_str() {
        "simple" => {
            // Seed LLM 호출 없이, 코드로 ad-hoc Seed 생성
            let seed = Seed::from_message(&user_input);
            // 바로 AgentRuntime 실행
            let exec_result = self.lifecycle.spawn_and_run(&seed, Priority::Normal).await?;
            // 결과 반환 (Evaluate LLM 호출 없이)
            return Ok(OrchestrationResult { ... });
        }
        _ => {
            // 기존 complex 경로: Seed LLM → AgentRuntime → (Evaluate 제거)
            if !result.ready_for_seed {
                // 질문 반환
            }
            let seed = self.ouroboros.generate_seed(&result).await?;
            let exec_result = self.lifecycle.spawn_and_run(&seed, Priority::Normal).await?;
            // 결과 반환
            return Ok(OrchestrationResult { ... });
        }
    }
}
```

### 3. Seed에 코드 생성 경로 추가

**파일**: `seed.rs`

```rust
impl Seed {
    /// 명확한 요청을 위한 ad-hoc Seed 생성 (LLM 호출 없음).
    /// 사용자 원문을 goal에 그대로 넣고, constraints/criteria는 비워둔다.
    pub fn from_message(message: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            goal: message.to_string(),
            original_request: message.to_string(),
            constraints: Vec::new(),
            acceptance_criteria: Vec::new(),
            ontology: Vec::new(),
            created_at: Utc::now(),
            generation: 0,
            parent_seed_id: None,
            cspace_hint: None,
        }
    }
}
```

### 4. Evolve 루프 + Evaluate LLM 제거

**파일**: `orchestrator.rs`

- Phase 5 (Evolve) while 루프 제거 (~60줄)
- Evaluate LLM 호출 제거
- AgentRuntime 실행 결과를 그대로 OrchestrationResult에 반환
- `OrchestratorConfig`에서 `max_evolution_iterations`, `min_evaluation_score` 제거

**파일**: `ouroboros_engine.rs`

- `evaluate()` 메서드: 트레이트 구현 유지, 내부 단순화
- `evolve()` 메서드: 트레이트 구현 유지, orchestrator에서 호출 안 함
- `EVALUATE_SYSTEM_PROMPT`, `EVOLVE_SYSTEM_PROMPT`: 제거

## 변경하지 않는 것

| 항목 | 이유 |
|------|------|
| Seed 구조체 이름 | rename 위험 > 이득 |
| Seed 필드 (ontology 등) | 기존 데이터 호환성 |
| OuroborosProtocol 트레이트 | API 안정성 |
| Interview 분류 로직 | 11/11 정확 |
| AmbiguityScore 가중치 | 시나리오 테스트 통과 |
| Interview "Be generous" | 실제로 잘 동작 |
| Phase enum (5개) | evolve, evaluate phase는 남김 |
| evaluation.rs, eval_cache.rs | 파일 유지, 내부 단순화 |

## 새로 추가하지 않는 것

| 제안했던 것 | 보류 이유 |
|------------|----------|
| classify.rs 모듈 | Interview에 통합이 더 단순 |
| spawn_and_run_with_message() | Seed::from_message()로 해결 |
| TaskSpec 타입 | Seed에 original_request 필드만 추가 |
| Interview 최대 라운드 | 기존 설계에서 유지하던 것, 이번 변경과 무관하게 나중에 추가 |

## LLM 호출 절약

| 시나리오 | Before | After | 절약 |
|----------|--------|-------|------|
| chat ("안녕") | 1회 | 1회 | 0% |
| simple task ("날씨 알려줘") | 3회 | 1회 | **67%** |
| clear complex ("foo.rs 수정") | 3회 | 2회 | 33% |
| ambiguous 2라운드 ("이거 고쳐줘") | 5회 | 4회 | 20% |

## Seed에 original_request 추가

**파일**: `seed.rs`

```rust
pub struct Seed {
    // 기존 필드 ...
    
    /// 사용자 원본 메시지 (번역/재포장 없이 그대로 보존).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub original_request: String,
}
```

**파일**: `agent_runtime.rs` — build_system_prompt

```rust
// goal 직후에 추가
if !seed.original_request.is_empty() && seed.original_request != seed.goal {
    prompt.push_str(&format!("\n## 사용자 원본 요청\n{}\n", seed.original_request));
}
```

`#[serde(default)]`이므로 기존 Seed JSON과 호환.
