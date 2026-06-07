# Chat UI 재설계 — 인터랙티브 인터뷰 + UX 개선

> **상태:** Draft (1차 검토 완료 — 수정사항 반영)  
> **날짜:** 2026-06-07  
> **영역:** oxios-web (frontend), oxios-gateway (WS protocol), oxios-kernel (orchestrator)

---

## 0. 검토 이력

| 날짜 | 검토자 | 결과 |
|------|--------|------|
| 2026-06-07 | 1차 | Gateway 통과 경로 누락, interview_response 수신 처리 누락, backward compat 불완전 지적 → §3.4, §3.5 전면 재작성 |

---

## 1. 문제 분석

### 1.1 현재 상태

현재 채팅 UI는 다음과 같은 구조:

```
┌─────────────────────────────────────────────────────────────┐
│ [새 대화] [새로고침]                    Connected 🟢       │  ← 불필요
├──────────┬──────────────────────────────────────────────────┤
│ Projects │                                                  │
│ · oxios  │   (빈 화면: "메시지를 입력하세요")              │
│ Sessions │                                                  │
│ · 오늘   │                                                  │
│ · 어제   │                                                  │
│          │                                                  │
│          │                                                  │
│          │  ┌──────────────────────────────────┐            │
│          │  │ 메시지를 입력하세요...       [➤] │            │
│          │  └──────────────────────────────────┘            │
└──────────┴──────────────────────────────────────────────────┘
```

### 1.2 식별된 문제점

| # | 문제 | 심각도 | 상세 |
|---|------|--------|------|
| **P1** | 인터뷰 질문이 일반 텍스트로만 표시 | 🔴 Critical | 우로보로스의 핵심 인터랙션인 "질문 → 답변"이 Claude 웹, pi questionnaire 같은 구조화된 UI가 아닌 plain text로 렌더됨. 사용자가 질문을 읽고 직접 타이핑해야 함 |
| **P2** | "Connected" 배지가 불필요 | 🟡 Minor | WebSocket 연결 상태는 사용자에게 무의미. 연결 끊김 시에만 표시 |
| **P3** | 빈 상태(Empty State)가 빈약 | 🟠 Medium | "메시지를 입력하세요" 텍스트 하나. 첫 인상이 밋밋함 |
| **P4** | 입력 영역이 단순 텍스트필드 | 🟠 Medium | 파일 첨부, 멀티라인 프리뷰, 커맨드 단축키 등이 없음 |
| **P5** | 사이드바가 항상 고정 | 🟡 Minor | 좁은 화면에서 채팅 공간을 잡아먹음. 토글/접기 필요 |
| **P6** | 인터뷰 진행 상태가 불명확 | 🟠 Medium | 인터뷰가 몇 라운드 진행 중인지, 언제 끝나는지 알 수 없음 |
| **P7** | 에러/재시도 UX 부재 | 🟡 Minor | 스트리밍 실패 시 사용자가 할 수 있는 것이 없음 |

### 1.3 벤치마크: Claude 웹 인터페이스

Claude.ai 채팅 UI의 핵심 패턴:
1. **빈 상태**: 제안 프롬프트 카드 (클릭하면 바로 전송)
2. **인터랙티브 응답**: 질문이 올 때 선택지가 버튼/칩으로 제시됨
3. **최소 헤더**: 접속 상태 표시 없음. 필요시에만 배너
4. **어댑티브 입력**: 큰 텍스트 영역 + 파일 드롭 + 음성

---

## 2. 설계 목표

1. **인터랙티브 인터뷰** — 에이전트의 질문이 구조화된 UI 컴포넌트로 렌더
2. **클린 레이아웃** — 불필요한 UI 요소 제거, 채팅에 최대 공간 할당
3. **적응형 입력** — 상황에 따라 입력 UI가 변화 (인터뷰 모드 vs 자유 입력)
4. **명확한 상태** — 인터뷰 진행 상태, 페이즈, 완료 여부를 직관적으로 표시

---

## 3. 핵심 설계: 인터랙티브 인터뷰 UI

### 3.1 현재 흐름 (문제)

```
사용자: "해커뉴스 트렌드 요약해줘"
에이전트: "I'd like to understand your request better. Could you help clarify:
          1. "트렌드" 기준이 무엇인가요 — 포인트가 높은 순, 댓글이 많은 순...?
          2. 요약은 한국어로 작성해야 하나요?
          3. 요약의 형식에 선호가 있나요?"
사용자: (직접 타이핑하여 3개 질문에 모두 답변)
```

**문제:** 
- 질문이 긴 텍스트 블록으로 표시되어 읽기 어려움
- 사용자가 어떤 형식으로 답변해야 할지 불명확
- 한 질문에만 답하고 싶어도 전체를 읽어야 함
- 모바일에서 특히 가독성이 떨어짐

### 3.2 제안 흐름

```
사용자: "해커뉴스 트렌드 요약해줘"
                    ┌─────────────────────────────────────────┐
                    │ 🤔 요청을 더 정확히 이해하고 싶어요    │
                    ├─────────────────────────────────────────┤
                    │                                         │
                    │ "트렌드" 기준이 무엇인가요?            │
                    │ ┌──────────┐ ┌──────────┐ ┌──────────┐  │
                    │ │ 포인트순 │ │ 댓글순   │ │ 프론트   │  │
                    │ └──────────┘ └──────────┘ └──────────┘  │
                    │                                         │
                    │ 요약 언어는 어떻게 할까요?             │
                    │ ┌──────┐ ┌────────────┐                 │
                    │ │ 한국어│ │ 원문 유지  │                 │
                    │ └──────┘ └────────────┘                 │
                    │                                         │
                    │ 요약 형식의 선호가 있나요?             │
                    │ ┌──────┐ ┌──────────┐ ┌──────┐          │
                    │ │한 줄 │ │불릿포인트│ │단락  │          │
                    │ └──────┘ └──────────┘ └──────┘          │
                    │                                         │
                    │ ┌────────────────────────────────────┐  │
                    │ │ 추가 의견이 있으면 입력하세요...   │  │
                    │ └────────────────────────────────────┘  │
                    │                                         │
                    │          [ 답변 제출 ]                  │
                    └─────────────────────────────────────────┘
```

### 3.3 백엔드 프로토콜 변경

#### 3.3.1 새 WS chunk type: `interview`

현재: 인터뷰 질문이 `response` 문자열로 plain text 전송됨.

**제안:** 새 chunk type `interview`를 추가하여 구조화된 데이터 전송.

```json
{
  "type": "interview",
  "session_id": "abc...",
  "questions": [
    {
      "id": "q1",
      "text": "\"트렌드\" 기준이 무엇인가요?",
      "kind": "single_choice",
      "options": [
        { "value": "points", "label": "포인트가 높은 순" },
        { "value": "comments", "label": "댓글이 많은 순" },
        { "value": "frontpage", "label": "현재 프론트페이지 기준" }
      ]
    },
    {
      "id": "q2",
      "text": "요약은 어떤 언어로 작성할까요?",
      "kind": "single_choice",
      "options": [
        { "value": "ko", "label": "한국어" },
        { "value": "original", "label": "원문 언어 유지" }
      ]
    },
    {
      "id": "q3",
      "text": "요약 형식의 선호가 있나요?",
      "kind": "single_choice",
      "options": [
        { "value": "one_line", "label": "한 줄 요약" },
        { "value": "bullets", "label": "불릿 포인트" },
        { "value": "paragraph", "label": "단락 형식" }
      ]
    }
  ],
  "round": 1,
  "ambiguity": 0.6
}
```

#### 3.3.2 InterviewQuestion 스키마

```typescript
interface InterviewQuestion {
  id: string
  text: string
  kind: 'single_choice' | 'multi_choice' | 'free_text' | 'yes_no'
  options?: InterviewOption[]
  required?: boolean
}

interface InterviewOption {
  value: string
  label: string
  description?: string
}

interface InterviewChunk {
  type: 'interview'
  session_id: string
  questions: InterviewQuestion[]
  round: number
  ambiguity: number
}
```

**`kind` 분류:**

| kind | UI | 예시 |
|------|----|----|
| `single_choice` | 칩/버튼 중 1개 선택 | "트렌드 기준이 무엇인가요?" |
| `multi_choice` | 칩 여러 개 선택 | "어떤 정보를 포함할까요?" |
| `free_text` | 텍스트 입력 필드 | "추가로 원하는 것이 있나요?" |
| `yes_no` | 예/아니오 버튼 | "기존 파일을 덮어쓸까요?" |

#### 3.3.3 사용자 응답 프로토콜

사용자가 인터뷰에 답변할 때:

```json
{
  "type": "interview_response",
  "session_id": "abc...",
  "answers": [
    { "question_id": "q1", "value": "points" },
    { "question_id": "q2", "value": "ko" },
    { "question_id": "q3", "value": "bullets" },
    { "question_id": "free_text", "value": "상위 10개만" }
  ]
}
```

### 3.4 백엔드 변경: 전체 파이프라인

> **⚠️ 1차 검토에서 식별:** 설계 초안은 Orchestrator가 직접 WS chunk를 보낸다고
> 가정했으나, 실제로는 `Orchestrator → OrchestrationResult → Gateway → OutgoingMessage → WS handler`
> 경로를 거침. 이 섹션은 실제 코드 경로에 맞게 전면 재작성.

#### 3.4.1 실제 데이터 흐름 (현재)

```
Orchestrator
  → OrchestrationResult {
      response: format_questions(&questions),  // "I'd like to understand..."
      phase_reached: Phase::Interview
    }
  → Gateway::dispatch()
  → OutgoingMessage { content: response, meta: ResponseMeta { phase: "Interview" } }
  → WS recv_task
  → token chunk { content: "I'd like to understand..." }  ← plain text
  → done chunk { phase: "Interview" }
```

#### 3.4.2 변경 후 데이터 흐름

```
Orchestrator
  → OrchestrationResult {
      response: format_questions(&questions),          // fallback용 텍스트
      interview_questions: Some(Vec<InterviewQuestion>),  // NEW
      phase_reached: Phase::Interview
    }
  → Gateway::dispatch()
  → OutgoingMessage {
      content: response,
      meta: ResponseMeta { phase: "Interview", interview_questions: Some(...) }
    }
  → WS recv_task
  → interview chunk { questions: [...], round, ambiguity }  ← NEW
  → done chunk { phase: "Interview" }
```

#### 3.4.3 변경점 1: `OrchestrationResult`에 필드 추가

```rust
// orchestrator.rs

pub struct OrchestrationResult {
    // ... 기존 필드 ...
    
    /// 구조화된 인터뷰 질문. 인터뷰 페이즈에서 모호도가 높아
    /// 추가 질문이 필요할 때 설정. 프론트엔드는 이 필드가 있으면
    /// 인터랙티브 UI를 렌더하고, 없으면 `response`를 마크다운으로 렌더.
    pub interview_questions: Option<Vec<InterviewQuestion>>,
    
    /// 인터뷰 라운드 번호 (1부터 시작).
    pub interview_round: Option<u32>,
    
    /// 현재 모호도 점수.
    pub interview_ambiguity: Option<f64>,
}
```

**주의:** `response` 필드는 유지. 구조화된 질문 생성에 실패하거나
구버전 프론트엔드의 fallback으로 사용.

#### 3.4.4 변경점 2: `ResponseMeta`에 필드 추가

```rust
// gateway/message.rs

pub struct ResponseMeta {
    // ... 기존 필드 ...
    
    /// 인터뷰 질문 (구조화). 프론트엔드가 interview chunk로 변환.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interview_questions: Option<Vec<InterviewQuestion>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interview_round: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interview_ambiguity: Option<f64>,
}
```

**의존성:** `oxios-gateway`가 `oxios-ouroboros::InterviewQuestion`을 참조.
현재 gateway는 ouroboros에 의존하지 않으므로, 둘 중 하나:

- **(A)** `InterviewQuestion`을 공유 타입으로 추출 (`oxios-ouroboros`에 두고 gateway가 참조)
- **(B)** gateway에 미러 타입을 만들고 Gateway::dispatch에서 변환

**(A) 권장.** `oxios-ouroboros`는 이미 kernel의 의존성이고, gateway도 kernel을 통해 간접 접근 가능.
하지만 `Cargo.toml`에 직접 의존성을 추가해야 함.

#### 3.4.5 변경점 3: WS recv_task에서 `interview` chunk 전송

```rust
// routes/chat.rs: recv_task 내

// 기존: token chunk + done chunk 전송
// 변경: meta.interview_questions가 있으면 interview chunk 먼저 전송

if let Some(ref questions) = msg.meta.as_ref().and_then(|m| m.interview_questions.clone()) {
    let interview_chunk = serde_json::json!({
        "type": "interview",
        "session_id": session_id,
        "project_id": project_id,
        "questions": questions,
        "round": msg.meta.as_ref().and_then(|m| m.interview_round),
        "ambiguity": msg.meta.as_ref().and_then(|m| m.interview_ambiguity),
    });
    let json = serde_json::to_string(&interview_chunk)?;
    ws_tx.send(Message::Text(json.into())).await?;
    // interview chunk 전송 후 token chunk는 생략 (질문 텍스트는 interview에 이미 포함)
} else {
    // 기존: token chunk 전송
    let token_chunk = serde_json::json!({
        "type": "token",
        "content": msg.content,
        ...
    });
    ws_tx.send(Message::Text(json.into())).await?;
}
// done chunk는 항상 전송
```

#### 3.4.6 변경점 4: WS send_task에서 `interview_response` 수신

> **⚠️ 1차 검토에서 식별:** 기존 설계에 누락됨.

```rust
// routes/chat.rs: send_task 내

match msg_type.as_str() {
    "message" => {
        // 기존 로직: content를 IncomingMessage로 만들어서 incoming_tx에 전송
    }
    "interview_response" => {
        // 인터뷰 답변 처리
        let answers = parsed.get("answers")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        
        // 답변을 자연어 문자열로 변환하여 일반 메시지로 전송
        // (Orchestrator는 interview_response 타입을 모름)
        let answer_text = answers.iter().filter_map(|a| {
            let q_id = a.get("question_id")?.as_str()?;
            let value = a.get("value")?.as_str()?;
            Some(format!("{}: {}", q_id, value))
        }).collect::<Vec<_>>().join("\n");
        
        if !answer_text.is_empty() {
            let mut incoming = IncomingMessage::new("web", "default", answer_text);
            if let Some(ref sid) = incoming_session_id {
                incoming.metadata.insert("session_id".into(), sid.clone());
            }
            incoming_tx.send(incoming).await?;
        }
    }
    _ => continue,
}
```

**핵심 설계 결정:** 인터뷰 답변은 백엔드에 **자연어 문자열**로 전달.
Orchestrator는 interview_response를 특별히 처리하지 않고, 기존 멀티턴 인터뷰
경로(follow-up message)를 그대로 탄다. 이유:

1. Orchestrator의 멀티턴 로직이 이미 `conversation_history`를 유지하고 있음
2. 새 메시지 타입을 Orchestrator까지 전파하려면 KernelHandle, Gateway 인터페이스 변경 필요
3. 답변이 자연어면 LLM이 문맥을 이해하는 데에도 유리

### 3.5 백엔드 변경: LLM 프롬프트 수정

#### 3.5.1 `InterviewResponse` 스키마 변경

현재:

```rust
#[derive(Debug, Deserialize)]
struct InterviewResponse {
    is_task: bool,
    chat_response: String,
    questions: Vec<String>,  // ← 단순 문자열 배열
    scores: Option<AmbiguityScores>,
    complexity: String,
}
```

**전략: `questions` 타입 변경하지 않음.** 대신 **새 필드 `structured_questions`** 추가.

이유:
- `questions: Vec<String>`은 Orchestrator에서 `join("\n")`, `last()`, `filter()` 등으로 직접 사용
- `InterviewResult.questions`도 `Vec<String>`이고 세션 직렬화에 사용됨
- 타입을 바꾸면 6개 이상 파일 연쇄 수정 → 위험도 높음

```rust
#[derive(Debug, Deserialize)]
struct InterviewResponse {
    is_task: bool,
    chat_response: String,
    questions: Vec<String>,  // 유지: Orchestrator 로직 + fallback용
    /// NEW: 구조화된 질문. LLM이 생성하면 채움, 실패하면 None.
    #[serde(default)]
    structured_questions: Option<Vec<InterviewQuestionOutput>>,
    scores: Option<AmbiguityScores>,
    complexity: String,
}

/// LLM 출력용 질문 스키마.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct InterviewQuestionOutput {
    id: String,
    text: String,
    #[serde(default = "default_free_text")]
    kind: String,
    #[serde(default)]
    options: Vec<InterviewOptionOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InterviewOptionOutput {
    value: String,
    label: String,
    #[serde(default)]
    description: String,
}
``n```

#### 3.5.2 프롬프트 수정

기존 `INTERVIEW_SYSTEM_PROMPT`에 추가:

```
## Structured Questions
When asking questions, also provide a "structured_questions" array.
Each entry has:
- "id": short identifier ("q1", "q2", ...)
- "text": the question text (same as the "questions" array entry)
- "kind": "single_choice" | "free_text" | "yes_no"
- "options": array of { "value", "label" } (required for single_choice, empty for free_text)

Example:
"structured_questions": [
  {
    "id": "q1",
    "text": "정렬 기준이 무엇인가요?",
    "kind": "single_choice",
    "options": [
      { "value": "points", "label": "포인트 높은 순" },
      { "value": "comments", "label": "댓글 많은 순" }
    ]
  },
  {
    "id": "q2",
    "text": "추가로 원하는 것이 있나요?",
    "kind": "free_text",
    "options": []
  }
]

If you cannot determine good options, omit structured_questions or set to null.
```

#### 3.5.3 Fallback 전략: Graceful Degradation

> **⚠️ 1차 검토에서 변경:** `free_text` 강등 대신 **완전 생략** 전략.

| LLM 응답 | 프론트엔드 동작 |
|----------|----------------|
| `structured_questions: Some([...])` | 인터랙티브 인터뷰 UI |
| `structured_questions: None` | 기존 마크다운 응답 (token chunk로 전송) |
| JSON 파싱 실패 | degraded fallback → 마크다운 응답 |

**왜 `free_text` 강등이 아닌 완전 생략인가:**
- `free_text` 질문은 칩/버튼 없이 텍스트 입력 필드만 보임 → 현재 경험과 차이 없음
- 옵션이 없는 질문을 굳이 구조화할 이유가 없음
- LLM이 옵션을 못 만들면 아예 일반 텍스트로 응답하는 게 더 자연스러움

### 3.6 Orchestrator 변경 요약

```rust
// orchestrator.rs: 인터뷰 결과 반환 부분

if !interview.ready_for_seed {
    let questions = interview.questions.iter()
        .filter(|q| !q.is_empty())
        .cloned()
        .collect::<Vec<_>>();

    return Ok(OrchestrationResult {
        session_id: Some(session_id.clone()),
        response: format_questions(&questions),
        interview_questions: structured.clone(),  // NEW
        interview_round: Some(current_round),     // NEW
        interview_ambiguity: Some(interview.ambiguity.ambiguity()),  // NEW
        // ... 기존 필드 ...
    });
}
```

`structured`는 `OuroborosEngine::interview()`의 반환값에 포함시키는 대신,
별도 메서드로 분리하는 것도 고려:

```rust
// ouroboros_engine.rs

/// Interview 결과에 구조화된 질문 포함
pub struct InterviewOutput {
    pub result: InterviewResult,       // 기존 (변경 없음)
    pub structured_questions: Option<Vec<InterviewQuestionOutput>>,
}
```

이렇게 하면 `InterviewResult`는 건드리지 않고 확장 가능.

---

## 4. UI/UX 개선 사항

### 4.1 "Connected" 배지 제거

**현재:** 헤더에 항상 `Connected 🟢` 표시  
**변경:** 제거. 연결 끊김 시에만 경고 배너 표시

```tsx
// Before
<ConnectionStatus connected={connected} />

// After
{!connected && (
  <div className="flex items-center gap-2 px-3 py-1.5 bg-warning/10 text-warning text-xs">
    <span className="h-2 w-2 rounded-full bg-warning animate-pulse" />
    {t('chat.reconnecting')}
  </div>
)}
```

### 4.2 빈 상태 개선

현재: "메시지를 입력하세요" 텍스트만  
변경: 제안 프롬프트 카드 + 빠른 액션

```
┌──────────────────────────────────────────────────────────────┐
│                                                              │
│              🤖 무엇을 도와드릴까요?                        │
│                                                              │
│  ┌──────────────────────┐  ┌──────────────────────────┐     │
│  │ 📝 코드 리뷰해줘     │  │ 🔍 최신 기술 트렌드      │     │
│  └──────────────────────┘  └──────────────────────────┘     │
│  ┌──────────────────────┐  ┌──────────────────────────┐     │
│  │ 📁 프로젝트 분석     │  │ 🗓️ 오늘 일정 확인        │     │
│  └──────────────────────┘  └──────────────────────────┘     │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

```tsx
function EmptyChatState({ onSuggestionClick }: { onSuggestionClick: (text: string) => void }) {
  const { t } = useTranslation()
  const suggestions = [
    { emoji: '📝', text: t('chat.suggestions.codeReview') },
    { emoji: '🔍', text: t('chat.suggestions.techTrend') },
    { emoji: '📁', text: t('chat.suggestions.projectAnalysis') },
    { emoji: '🗓️', text: t('chat.suggestions.todaySchedule') },
  ]

  return (
    <div className="flex flex-col items-center justify-center h-full gap-6 text-muted-foreground">
      <div className="text-center">
        <Bot className="h-12 w-12 mx-auto mb-3 text-primary/60" />
        <p className="text-lg font-medium">{t('chat.greeting')}</p>
      </div>
      <div className="grid grid-cols-2 gap-3 max-w-md">
        {suggestions.map((s) => (
          <button
            key={s.text}
            onClick={() => onSuggestionClick(`${s.emoji} ${s.text}`)}
            className="flex items-center gap-2 px-4 py-3 rounded-lg border bg-card hover:bg-accent/50 transition-colors text-sm text-left"
          >
            <span>{s.emoji}</span>
            <span>{s.text}</span>
          </button>
        ))}
      </div>
    </div>
  )
}
```

### 4.3 입력 영역 개선

**현재:** 단순 Textarea + Send 버튼  
**변경:**

1. **Shift+Enter** 줄바꿈, **Enter** 전송 (현재와 동일)
2. **인터뷰 모드 전환** — 인터뷰 응답이 활성 상태면 일반 입력이 비활성화되고 InterviewResponse 컴포넌트로 전환
3. **파일 드롭존** — 파일 드래그 시 시각적 피드백
4. **모델/페르소나 표시** — 현재 활성 모델을 입력 영역 하단에 작게 표시

### 4.4 사이드바 개선

**현재:** 항상 56px (w-56) 고정  
**변경:**
- 모바일/좁은 화면: 기본 숨김, 토글 버튼으로 열기
- 데스크톱: `Cmd+B`로 토글
- 축소 모드: 아이콘만 표시 (w-14)

### 4.5 인터뷰 진행 상태 표시

인터뷰 진행 중일 때 메시지 영역 상단에 프로그레스 바 표시:

```
┌──────────────────────────────────────────────┐
│  🔄 인터뷰 진행 중  Round 1/3  모호도 0.6  │
│  ████████░░░░░░░░░░░                         │
└──────────────────────────────────────────────┘
```

### 4.6 스트리밍 중 페이즈 인디케이터 개선

현재 RFC-015로 `ActivityTimeline`이 구현되어 있으나, 스트리밍 중 현재 페이즈가 헤더에 표시되지 않음.

**추가:** 스트리밍 중일 때 입력 영역 위에 현재 페이즈 표시:

```
┌──────────────────────────────────────────────┐
│  ⚙️ 에이전트 실행 중...                      │
│  🔧 bash · read_file (2/5 tools)            │
└──────────────────────────────────────────────┘
```

---

## 5. 컴포넌트 아키텍처

### 5.1 새/수정 컴포넌트

```
components/chat/
├── chat-input.tsx              ← 수정: 인터뷰 모드 인식
├── connection-status.tsx       ← 제거 (→ 인라인 경고 배너)
├── empty-chat-state.tsx        ← 신규: 제안 프롬프트 카드
├── interview-response.tsx      ← 신규: 인터뷰 응답 UI (핵심)
├── interview-question.tsx      ← 신규: 개별 질문 (kind별 렌더링)
├── interview-progress.tsx      ← 신규: 인터뷰 진행 상태
├── message-bubble.tsx          ← 수정: 인터뷰 메시지 렌더링
├── activity-timeline.tsx       ← 유지
├── activity-card.tsx           ← 유지
├── tool-call-card.tsx          ← 유지
├── chat-metadata.tsx           ← 유지
├── browse-context-badge.tsx    ← 유지
└── browse-context-detail.tsx   ← 유지
```

### 5.2 InterviewResponse 컴포넌트 (핵심)

```tsx
interface InterviewResponseProps {
  questions: InterviewQuestion[]
  round: number
  ambiguity: number
  onSubmit: (answers: InterviewAnswer[]) => void
  disabled?: boolean
}

function InterviewResponse({ questions, round, ambiguity, onSubmit, disabled }: InterviewResponseProps) {
  const [answers, setAnswers] = useState<Record<string, string | string[]>>({})
  const [freeText, setFreeText] = useState('')

  const handleSubmit = () => {
    const formatted = Object.entries(answers).map(([qId, value]) => ({
      question_id: qId,
      value: Array.isArray(value) ? value.join(', ') : value,
    }))
    if (freeText.trim()) {
      formatted.push({ question_id: 'free_text', value: freeText.trim() })
    }
    onSubmit(formatted)
  }

  const allRequiredAnswered = questions
    .filter(q => q.kind !== 'free_text')
    .every(q => answers[q.id] !== undefined)

  return (
    <div className="rounded-xl border bg-card shadow-sm">
      {/* 헤더 */}
      <div className="flex items-center justify-between px-4 py-3 border-b">
        <div className="flex items-center gap-2">
          <HelpCircle className="h-4 w-4 text-primary" />
          <span className="text-sm font-medium">{t('chat.interview.title')}</span>
        </div>
        <InterviewProgress round={round} ambiguity={ambiguity} />
      </div>

      {/* 질문들 */}
      <div className="p-4 space-y-5">
        {questions.map((q) => (
          <InterviewQuestionCard
            key={q.id}
            question={q}
            value={answers[q.id]}
            onChange={(v) => setAnswers(prev => ({ ...prev, [q.id]: v }))}
            disabled={disabled}
          />
        ))}

        {/* 추가 의견 */}
        <div>
          <p className="text-xs text-muted-foreground mb-1.5">
            {t('chat.interview.additionalThoughts')}
          </p>
          <Textarea
            value={freeText}
            onChange={(e) => setFreeText(e.target.value)}
            placeholder={t('chat.interview.optionalPlaceholder')}
            className="min-h-[60px] resize-none"
            disabled={disabled}
          />
        </div>
      </div>

      {/* 제출 */}
      <div className="flex justify-end px-4 py-3 border-t">
        <Button onClick={handleSubmit} disabled={!allRequiredAnswered || disabled}>
          {t('chat.interview.submit')} <ArrowRight className="h-4 w-4 ml-1" />
        </Button>
      </div>
    </div>
  )
}
```

### 5.3 InterviewQuestionCard 컴포넌트

```tsx
function InterviewQuestionCard({ question, value, onChange, disabled }: {
  question: InterviewQuestion
  value: string | string[] | undefined
  onChange: (value: string | string[]) => void
  disabled?: boolean
}) {
  switch (question.kind) {
    case 'single_choice':
      return (
        <div>
          <p className="text-sm font-medium mb-2">{question.text}</p>
          <div className="flex flex-wrap gap-2">
            {question.options?.map(opt => (
              <button
                key={opt.value}
                onClick={() => onChange(opt.value)}
                disabled={disabled}
                className={cn(
                  'px-3 py-1.5 rounded-full text-sm border transition-colors',
                  value === opt.value
                    ? 'bg-primary text-primary-foreground border-primary'
                    : 'bg-card hover:bg-accent/50 border-border'
                )}
              >
                {opt.label}
              </button>
            ))}
          </div>
        </div>
      )

    case 'multi_choice':
      return (
        <div>
          <p className="text-sm font-medium mb-2">{question.text}</p>
          <div className="flex flex-wrap gap-2">
            {question.options?.map(opt => {
              const selected = Array.isArray(value) && value.includes(opt.value)
              return (
                <button
                  key={opt.value}
                  onClick={() => {
                    const current = Array.isArray(value) ? value : []
                    onChange(selected
                      ? current.filter(v => v !== opt.value)
                      : [...current, opt.value]
                    )
                  }}
                  disabled={disabled}
                  className={cn(
                    'px-3 py-1.5 rounded-full text-sm border transition-colors',
                    selected
                      ? 'bg-primary text-primary-foreground border-primary'
                      : 'bg-card hover:bg-accent/50 border-border'
                  )}
                >
                  {opt.label}
                </button>
              )
            })}
          </div>
        </div>
      )

    case 'yes_no':
      return (
        <div>
          <p className="text-sm font-medium mb-2">{question.text}</p>
          <div className="flex gap-2">
            <button
              onClick={() => onChange('yes')}
              disabled={disabled}
              className={cn(
                'px-4 py-1.5 rounded-lg text-sm border transition-colors',
                value === 'yes'
                  ? 'bg-success/10 text-success border-success/30'
                  : 'bg-card hover:bg-accent/50 border-border'
              )}
            >
              ✅ 예
            </button>
            <button
              onClick={() => onChange('no')}
              disabled={disabled}
              className={cn(
                'px-4 py-1.5 rounded-lg text-sm border transition-colors',
                value === 'no'
                  ? 'bg-error/10 text-error border-error/30'
                  : 'bg-card hover:bg-accent/50 border-border'
              )}
            >
              ❌ 아니오
            </button>
          </div>
        </div>
      )

    case 'free_text':
    default:
      return (
        <div>
          <p className="text-sm font-medium mb-2">{question.text}</p>
          <Textarea
            value={(value as string) ?? ''}
            onChange={(e) => onChange(e.target.value)}
            disabled={disabled}
            className="min-h-[44px] resize-none"
            placeholder={t('chat.interview.typeAnswer')}
          />
        </div>
      )
  }
}
```

### 5.4 Chat Store 확장

```typescript
// stores/chat.ts 추가

interface ChatStore {
  // ... 기존 ...

  /** 활성 인터뷰 질문 (인터뷰 모드가 아닐 때 null). */
  activeInterview: InterviewState | null
  /** 인터뷰 답변 제출. */
  submitInterviewResponse: (answers: InterviewAnswer[]) => void
}

interface InterviewState {
  questions: InterviewQuestion[]
  round: number
  ambiguity: number
  messageIndex: number  // 어느 메시지에 해당하는 인터뷰인지
}

interface InterviewAnswer {
  question_id: string
  value: string
}
```

`handleChunk`에 `interview` case 추가:

```typescript
case 'interview': {
  set({
    activeInterview: {
      questions: chunk.questions,
      round: chunk.round,
      ambiguity: chunk.ambiguity,
      messageIndex: s.messages.length - 1,
    },
    isStreaming: false,
  })
  break
}
```

`submitInterviewResponse`:

```typescript
submitInterviewResponse(answers: InterviewAnswer[]) {
  const { activeInterview } = get()
  if (!activeInterview) return

  // WS로 전송
  wsInstance?.send(JSON.stringify({
    type: 'interview_response',
    session_id: get().activeSessionId ?? '',
    answers,
  }))

  // 답변을 사용자 메시지로 추가
  const answerText = answers
    .map(a => a.value)
    .filter(v => v)
    .join(', ')

  const userMsg: ChatMessage = {
    id: crypto.randomUUID(),
    role: 'user',
    content: answerText,
    timestamp: new Date().toISOString(),
  }

  set(s => ({
    messages: [...s.messages, userMsg],
    activeInterview: null,
    isStreaming: true,
  }))
}
```

### 5.5 ChatPage 수정

```tsx
function ChatPage() {
  const { activeInterview, ... } = useChatStore()
  // ...

  return (
    <div className="flex h-[calc(100vh-8rem)]">
      <CollapsibleSidebar ... />

      <div className="flex flex-1 flex-col min-w-0">
        {/* 헤더 — Connected 제거, 타이틀만 */}
        <div className="flex items-center justify-between px-4 py-3 border-b">
          <h2 className="text-sm font-semibold">
            {activeSessionId ? t('chat.activeConversation') : t('chat.newConversation')}
          </h2>
          <div className="flex items-center gap-1">
            {/* 새로고침, 새 대화 버튼 */}
          </div>
        </div>

        {/* 연결 끊김 경고 (선택적) */}
        {!connected && <ReconnectBanner />}

        {/* 메시지 */}
        <Card className="flex-1 flex flex-col min-h-0 mx-4 my-3 border-t-0">
          <ScrollArea ...>
            {messages.length === 0 && !activeInterview ? (
              <EmptyChatState onSuggestionClick={handleSuggestionClick} />
            ) : (
              <div className="space-y-4">
                {messages.map((msg) => (
                  <MessageBubble key={msg.id} message={msg} />
                ))}

                {/* 인터뷰 응답 UI */}
                {activeInterview && (
                  <InterviewResponse
                    questions={activeInterview.questions}
                    round={activeInterview.round}
                    ambiguity={activeInterview.ambiguity}
                    onSubmit={(answers) => submitInterviewResponse(answers)}
                  />
                )}

                <div ref={bottomRef} />
              </div>
            )}
          </ScrollArea>

          {/* 입력 — 인터뷰 활성 시 숨김 */}
          {!activeInterview && (
            <ChatInput ... />
          )}
        </Card>
      </div>
    </div>
  )
}
```

### 5.6 MessageBubble 수정 — 인터뷰 chunk 렌더링

`interview` chunk가 오면 기존 MessageBubble이 아니라 `InterviewResponse` 컴포넌트가
렌더됨 (ChatPage에서 분기). MessageBubble 자체는 수정 필요 없음.

과거 인터뷰 메시지는 현재 plain text로 저장되므로, 일반 마크다운으로 렌더.
구조화 복원은 세션 포맷 변경 후 별도 구현 (§9.7 참조).

---

## 6. 백엔드 프로토콜 변경 요약

### 6.1 데이터 흐름 전체도

```
┌─────────────────────────────────────────────────────────────────────┐
│ LLM (interview)                                                     │
│   Input: user message                                               │
│   Output: { questions: [...], structured_questions: [...] | null }  │
└───────────────────────────┬─────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────┐
│ OuroborosEngine::interview()                                        │
│   → InterviewOutput { result, structured_questions }               │
└───────────────────────────┬─────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────┐
│ Orchestrator                                                        │
│   → OrchestrationResult {                                          │
│       response: "I'd like to understand...",  // fallback 텍스트   │
│       interview_questions: Some([...]),       // 구조화 (선택)     │
│       interview_round: Some(1),                                      │
│       interview_ambiguity: Some(0.6),                                │
│     }                                                                │
└───────────────────────────┬─────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────┐
│ Gateway::dispatch()                                                 │
│   → OutgoingMessage {                                               │
│       content: response,                                            │
│       meta: ResponseMeta { interview_questions, ... }               │
│     }                                                                │
└───────────────────────────┬─────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────┐
│ WS recv_task (routes/chat.rs)                                       │
│                                                                     │
│   if meta.interview_questions.is_some():                            │
│     → interview chunk  { type, questions, round, ambiguity }        │
│   else:                                                             │
│     → token chunk    { type, content }                              │
│   항상:                                                             │
│     → done chunk     { type, session_id, phase, ... }               │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│ WS send_task (routes/chat.rs) ← 프론트엔드 → 백엔드                │
│                                                                     │
│   type: "message":                                                  │
│     → IncomingMessage { content } → incoming_tx                     │
│   type: "interview_response":  ← NEW                                │
│     → answers를 자연어로 변환 → IncomingMessage → incoming_tx       │
│     (Orchestrator는 멀티턴 follow-up으로 처리)                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 6.2 WS Chunk Types

| Chunk | 방향 | 설명 |
|-------|------|------|
| `interview` | Server → Client | 구조화된 인터뷰 질문 전송 |
| `interview_response` | Client → Server | 사용자의 인터뷰 답변 (자연어로 변환되어 전달) |

### 6.3 변경 파일 목록

| 파일 | 변경 |
|------|------|
| `ouroboros_engine.rs` | `InterviewQuestionOutput` 타입 정의, `structured_questions` 필드 추가, 프롬프트 수정 |
| `ouroboros_engine.rs` | `interview()` → `InterviewOutput` 반환으로 변경 (또는 별도 메서드) |
| `protocol.rs` | 트레이트 시그니처 변경 가능성 (InterviewOutput 반환) |
| `interview.rs` | **변경 없음** (`InterviewResult.questions`는 `Vec<String>` 유지) |
| `orchestrator.rs` | `OrchestrationResult`에 `interview_questions`, `interview_round`, `interview_ambiguity` 필드 추가 |
| `orchestrator.rs` | 인터뷰 결과 반환 시 구조화 질문 필드 채움 |
| `gateway/message.rs` | `ResponseMeta`에 동일 필드 추가 |
| `gateway/gateway.rs` | `dispatch()`에서 `OrchestrationResult` → `ResponseMeta` 매핑 시 새 필드 복사 |
| `routes/chat.rs` | WS recv_task: `interview` chunk 전송 분기 |
| `routes/chat.rs` | WS send_task: `interview_response` 타입 처리 |

---

## 7. 구현 순서

### Phase 1: 프론트엔드 정리 (Backend 변경 없이 가능)
1. `ConnectionStatus` 제거 → reconnect 배너로 교체
2. `EmptyChatState` 컴포넌트 구현
3. 사이드바 축소/확장 토글

### Phase 2: 백엔드 파이프라인 (위에서 아래로)
4. `InterviewQuestionOutput`/`InterviewOptionOutput` 타입 정의 (`ouroboros_engine.rs`)
5. `InterviewResponse`에 `structured_questions` 필드 추가
6. Interview 프롬프트에 구조화 질문 생성 지시 추가
7. `OuroborosEngine::interview()` → `InterviewOutput` 반환
8. `OuroborosProtocol` 트레이트 변경 (반환 타입)
9. `OrchestrationResult`에 `interview_questions`, `interview_round`, `interview_ambiguity` 필드 추가
10. `orchestrator.rs`에서 새 필드 채우는 로직
11. `ResponseMeta`에 동일 필드 추가 (`gateway/message.rs`)
12. `Gateway::dispatch()` 매핑 수정
13. `routes/chat.rs` WS recv_task: `interview` chunk 전송 분기
14. `routes/chat.rs` WS send_task: `interview_response` 타입 처리

### Phase 3: 프론트엔드 인터뷰 UI
15. `InterviewQuestion`/`InterviewOption` 타입 정의 (`types/index.ts`)
16. `StreamChunk` type 유니온에 `interview` 추가
17. `ChatStore`에 `activeInterview`, `submitInterviewResponse` 추가
18. `handleChunk`에 `interview` case 추가
19. `InterviewResponse` + `InterviewQuestionCard` 컴포넌트
20. `ChatPage` 통합 (인터뷰 활성 시 입력 영역 숨김)

### Phase 4: 폴리시
21. 인터뷰 진행 프로그레스 바
22. 스트리밍 페이즈 인디케이터
23. 입력 영역 개선 (파일 드롭, 모델 표시)
24. i18n 키 추가
25. 모바일 반응형

### 세션 복원은 보류

과거 인터뷰의 구조화 데이터는 현재 세션 저장 포맷(`agent_responses[n].content`)에
plain text로 저장되어 있어, 복원 시 다시 파싱해야 함. 이 기능은 Phase 4 이후에
별도 작업으로 진행. (세션에 `interview_questions` JSON을 메타데이터로 저장하는
방식 필요)

---

## 8. 타입 정의 요약

### Backend (Rust)

```rust
// crates/oxios-ouroboros/src/interview.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterviewQuestion {
    pub id: String,
    pub text: String,
    #[serde(default = "default_free_text")]
    pub kind: String,
    #[serde(default)]
    pub options: Vec<InterviewOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterviewOption {
    pub value: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
}
```

### Frontend (TypeScript)

```typescript
// types/index.ts 추가

export interface InterviewQuestion {
  id: string
  text: string
  kind: 'single_choice' | 'multi_choice' | 'free_text' | 'yes_no'
  options?: InterviewOption[]
  required?: boolean
}

export interface InterviewOption {
  value: string
  label: string
  description?: string
}

export interface InterviewAnswer {
  question_id: string
  value: string
}

// StreamChunk type 유니온에 추가
// | 'interview' | 'interview_response'

// ChatMessage metadata에 추가
// interview_questions?: InterviewQuestion[]
```

---

## 9. 고려사항

### 9.1 LLM 옵션 생성 품질

LLM이 항상 적절한 옵션을 생성하지 않을 수 있음. 대응:
- **Graceful Degradation:** `structured_questions`가 `None`이면 마크다운 응답 (현재와 동일)
- 프롬프트에 2-3개의 예시 포함
- `serde(default)` + `parse_json` 실패 시 `None` 처리
- JSON retry 로직이 이미 있으므로 파싱 실패 시 한 번 재시도

### 9.2 인터뷰가 없는 직접 실행

`complexity: "simple"` + `ready_for_seed: true`인 경우 인터뷰 없이 바로 실행.  
이 경우 기존 경로 그대로 토큰 스트리밍 → `ActivityTimeline` 표시.

### 9.3 모바일

인터뷰 칩/버튼이 터치 친화적이어야 함. 최소 타겟 44px.

### 9.4 성능

인터뷰 UI는 정적 렌더링이므로 성능 영향 없음.  
`activeInterview` 상태는 메시지당 1개, 크기 1KB 미만.

### 9.5 `OuroborosProtocol` 트레이트 변경 영향

현재 트레이트:
```rust
async fn interview(&self, user_input: &str) -> Result<InterviewResult>;
```

변경 후:
```rust
async fn interview(&self, user_input: &str) -> Result<InterviewOutput>;
```

`InterviewOutput`은 `InterviewResult`를 래핑하므로, 기존 `InterviewResult`를 사용하는
모든 코드는 `.result` 필드로 접근. 테스트 수정 필요.

### 9.6 Gateway 의존성 추가

`oxios-gateway`가 `InterviewQuestionOutput`을 직렬화/역직렬화해야 함.
두 가지 옵션:

**(A) `oxios-ouroboros`를 gateway의 의존성에 추가**
- `gateway/Cargo.toml`에 `oxios-ouroboros` 추가
- `ResponseMeta`에서 `oxios_ouroboros::interview::InterviewQuestionOutput` 직접 참조
- 깔끔하지만 의존성 그래프에 간선 하나 추가

**(B) 미러 타입**
- `gateway/message.rs`에 동일 구조의 `GatewayInterviewQuestion` 정의
- `Gateway::dispatch()`에서 변환 코드 작성
- 의존성은 없지만 중복 정의 유지보수 부담

권장: **(A)**. 이미 kernel이 ouroboros를 의존하고 있고 gateway도 kernel을 참조하므로
의존성 그래프에 사이클 없음.

### 9.7 세션 복원 (Future)

현재 세션 저장 포맷:
```rust
pub struct Session {
    user_messages: Vec<String>,
    agent_responses: Vec<AgentResponse { content: String, ... }>,
    // ...
}
```

인터뷰 질문은 `agent_responses[n].content`에 `format_questions()` 결과가 들어감.
구조화 복원을 위해서는:
1. 세션 저장 시 `agent_responses`에 `interview_questions` JSON을 메타데이터로 첨부
2. 또는 별도 `interview_snapshots` 필드를 `Session`에 추가
3. 프론트엔드 `loadSession` 시 이 데이터를 읽어서 과거 인터뷰 UI 렌더

이 기능은 Phase 4 이후 별도 작업으로 진행.
