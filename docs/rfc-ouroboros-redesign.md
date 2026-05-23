# RFC: Ouroboros 재설계 — Oxios에 맞는 프로토콜

## 전제

Oxios는 **모든 사람을 위한 범용 AI OS**다. 개발자 도구가 아니다.
사용자는 한국어로 "오늘 날씨 알려줘", "블로그에 여행기 올려줘", "이거 고쳐줘"라고 말한다.
Ouroboros의 아이디어(spec-first: 확실해지기 전까지 실행하지 않는다)는 차용하되,
구현은 Oxios의 사용 패턴에 맞게 새로 작성한다.

## 현재의 문제

### 문제 1: 모든 요청이 무거운 파이프라인을 통과한다

```
"오늘 날씨 알려줘" (명확한 간단 요청):
  Interview LLM → Seed LLM → AgentRuntime → Evaluate LLM
  = 오버헤드 LLM 3회 + AgentRuntime
  
  하지만 AgentRuntime에 "오늘 날씨 알려줘"를 그대로 줘도 된다.
  Seed LLM은 "Check today's weather"로 재포장할 뿐이다.
  Evaluate LLM은 "Weather info provided"라고 점수를 매길 뿐이다.
```

### 문제 2: Seed가 원본의 개발 도구용 설계를 답습했다

```rust
// 현재 Seed — 원본의 software engineering용 구조
pub struct Seed {
    pub id: Uuid,
    pub goal: String,
    pub constraints: Vec<String>,        // ← 범용 OS에서 거의 비어있음
    pub acceptance_criteria: Vec<String>, // ← LLM이 대충 만듦
    pub ontology: Vec<Entity>,            // ← 거의 항상 비어있음
    pub created_at: DateTime<Utc>,
    pub generation: u32,                  // ← evolve 없으면 의미 없음
    pub parent_seed_id: Option<Uuid>,     // ← evolve 없으면 의미 없음
    pub cspace_hint: Option<String>,      // ← 내부 구현 디테일
}
```

실제 Seed 데이터:
```json
{
  "goal": "Execute the shell command 'echo hello world'",
  "constraints": [],
  "acceptance_criteria": ["The string 'hello world' appears in output"],
  "ontology": []
}
```
= 사용자의 "echo hello world 실행해줘"를 영어로 재포장. constraints, ontology는 비어있음.

### 문제 3: Interview가 분류와 모호성 측정을 한 번에 한다

시나리오 테스트에서 잘 동작하지만, **task/chat 분류**와 **모호성 측정**은 
성격이 다른 작업이다. 분류는 가볍게, 모호성은 깊게 판단해야 한다.

지금은 한 LLM 호출에 둘 다 넣고, 결과적으로 "generous" 점수를 줘야 
채팅이 안 걸리는 상황이 된다.

---

## 새 설계

### 핵심 원칙

1. **요청의 무게에 비례해서 프로세스의 무게를 결정한다**
2. **LLM 호출은 꼭 필요한 만큼만**
3. **사용자의 원문을 그대로 존중한다** (번역/재포장 금지)

### 3-Tier 라우팅

```
사용자 메시지
    │
    ▼
┌─────────────────────────┐
│  Classify (LLM 1회)      │
│  task/chat + complexity  │
└────┬────────────────────┘
     │
     ├── CHAT ──────────────→ 응답 반환 (끝)
     │
     ├── SIMPLE_TASK ───────→ AgentRuntime 직접 실행
     │   "오늘 날씨 알려줘"     (Seed 없이, 원문 그대로)
     │   "알람 7시로 설정해줘"
     │                          LLM 0회 추가
     │
     └── COMPLEX_TASK ──────→ Interview (필요시) → Seed → Execute → Report
         "이거 좀 고쳐줘"     
         "블로그에 여행기 올려줘"  LLM 1-3회 추가 (interview 라운드 수)
```

### Classify: 1회 LLM 호출로 3-way 분류

```rust
#[derive(Deserialize)]
struct ClassifyResponse {
    /// chat: 인사, 질문, 잡담, 의견
    /// simple: 명확한 1회성 요청 (날씨, 알람, 검색, 계산)
    /// complex: 모호하거나 다단계 작업 (수정, 작성, 배포, 분석)
    category: String,  // "chat" | "simple" | "complex"
    
    /// chat일 때의 응답
    response: Option<String>,
    
    /// complex일 때의 초기 모호성 점수
    ambiguity: Option<f64>,  // 0.0-1.0
    
    /// complex + 모호할 때의 질문
    questions: Option<Vec<String>>,
}
```

### SIMPLE_TASK: Seed 없이 직접 실행

```rust
// Classify가 simple이라고 판단하면
// Seed 생성 LLM 호출 없이 바로 AgentRuntime으로

let result = self.lifecycle
    .spawn_and_run_with_message(&user_message, Priority::Normal)
    .await?;
```

AgentRuntime에 `spawn_and_run_with_message()` 추가:
- Seed 대신 사용자 원문을 직접 system prompt에 주입
- constraints, acceptance_criteria 없이 goal만으로 실행
- 간단한 요청은 agent가 알아서 판단

### COMPLEX_TASK: Interview → Seed → Execute → Report

Interview는 기존처럼 동작. 하지만 Seed 구조를 단순화:

```rust
/// Oxios Task Specification.
/// 사용자의 명확해진 요청을 실행 가능한 형태로 정리.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    /// 고유 식별자
    pub id: Uuid,
    
    /// 명확화된 목표 (사용자 언어 그대로)
    pub goal: String,
    
    /// 사용자 원본 메시지 (수정 없이 보존)
    pub original_request: String,
    
    /// 인터뷰에서 밝혀진 제약사항 (있으면)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<String>,
    
    /// 인터뷰에서 밝혀진 성공 기준 (있으면)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_criteria: Vec<String>,
    
    /// capability 시스템 힌트
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cspace_hint: Option<String>,
    
    /// 생성 시각
    pub created_at: DateTime<Utc>,
}
```

**Seed에서 제거한 것**:
- `ontology: Vec<Entity>` — 범용 OS에서 사용 안 함
- `generation: u32` — evolve 제거
- `parent_seed_id` — evolve 제거
- `created_at`은 유지 (state store용)

### Report: Evaluate를 간단한 결과 보고로 대체

```rust
// 현재: LLM이 점수 매기는 formal evaluation
// 새: AgentRuntime의 실행 결과를 그대로 보고

pub struct ExecutionReport {
    /// 실행 성공 여부 (AgentRuntime이 판단)
    pub success: bool,
    /// 에이전트가 생성한 출력
    pub output: String,
    /// 실행 중 발생한 에러 (있으면)
    pub error: Option<String>,
}
```

LLM evaluation 제거. 대신:
- AgentRuntime이 도구 실행 결과로 스스로 성공/실패를 판단 (이미 하고 있음)
- 결과를 사용자에게 그대로 보여줌
- 사용자가 불만족하면 새 메시지로 재시도

---

## LLM 호출 비교

### Before (현재)

```
"오늘 날씨 알려줘":
  Interview LLM → Seed LLM → AgentRuntime → Evaluate LLM
  = 오버헤드 3회

"이거 좀 고쳐줘" (2라운드 인터뷰):
  Interview LLM → Interview LLM → Interview LLM → Seed LLM → AgentRuntime → Evaluate LLM
  = 오버헤드 5회
```

### After (새 설계)

```
"오늘 날씨 알려줘":
  Classify LLM → AgentRuntime
  = 오버헤드 1회 (67% 절감)

"이거 좀 고쳐줘" (2라운드 인터뷰):
  Classify LLM → Interview LLM → Interview LLM → AgentRuntime
  = 오버헤드 3회 (40% 절감)
```

---

## 모듈 구조

```
crates/oxios-ouroboros/src/
├── lib.rs              # 공개 API
├── classify.rs         # 3-way 분류 (chat/simple/complex) ← 새로
├── interview.rs        # 복잡한 요청의 명확화 인터뷰 (기존 유지)
├── task_spec.rs        # TaskSpec 정의 (Seed 대체) ← rename
├── protocol.rs         # OuroborosProtocol 트레이트 (간소화)
└── ouroboros_engine.rs # 엔진 구현 (Classify + Interview)
```

**제거**:
- `evaluation.rs` — LLM evaluation 제거
- `eval_cache.rs` — evaluation cache 제거
- `seed.rs` → `task_spec.rs` 로 rename + 간소화

**추가**:
- `classify.rs` — 3-way 분류 로직

---

## 마이그레이션

### Seed → TaskSpec

- 필드 대부분 호환 (goal, constraints, id, cspace_hint)
- `ontology` 제거 → `#[serde(default)]`로 기존 JSON 호환
- `generation`, `parent_seed_id` 제거 → `#[serde(default)]`로 기존 JSON 호환
- `original_request` 추가 → `#[serde(default)]`로 기존 JSON 호환
- 기존 `~/.oxios/workspace/seeds/` 파일들 읽을 수 있음

### AgentRuntime 영향

- `execute(&self, agent_id, seed: &Seed)` → `execute(&self, agent_id, spec: &TaskSpec)`
- `spawn_and_run_with_message(&self, message: &str)` 추가 (simple task용)
- `build_system_prompt()`은 TaskSpec에서 goal/constraints/acceptance_criteria 사용

### Orchestrator 영향

- `handle_message()`에 3-way 라우팅 추가
- Evolve 루프 제거
- Evaluate 단계 제거 (AgentRuntime 결과를 직접 보고)
- InterviewSession에 round 카운터 추가
