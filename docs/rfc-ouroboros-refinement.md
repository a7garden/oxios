# RFC: Ouroboros 다듬기 (원본 대비 검증 완료)

## 검증 방법

1. 원본 Q00/ouroboros (Python, 171K줄) 핵심 모듈 분석
2. Oxios ouroboros (Rust, 1.7K줄)과의 구조적 비교
3. 실제 LLM 시나리오 테스트 (11개 케이스)
4. 실시드 데이터 분석 (~80개 Seed JSON)
5. 코드 흐름 전체 추적 (orchestrator → gateway → channel)

---

## 핵심 발견: Oxios의 Evolve는 원본의 Evolve가 아니다

### 원본의 Evolve (4,611줄)

```
Gen 1: Seed → Execute → Evaluate
Gen 2: Wonder("뭘 모르지?") → Reflect("어떻게 고치지?") → Seed₂ → Execute → Evaluate
Gen 3: Wonder → Reflect → Seed₃ → Execute → Evaluate
...수렴(convergence)하거나 max_generations(기본 30)까지

구성:
  wonder.py     (422줄): "아직 모르는 게 뭐야?" → 질문 생성
  reflect.py    (467줄): Wonder + 실행결과 → 온톨로지 돌연변이 + 개선된 AC
  convergence.py(369줄): 온톨로지 안정성, 정체, 발진, 수렴 판정
  watchdog.py   (610줄): 진행 감시, 타임아웃, graceful shutdown
  loop.py      (1925줄): 세대 관리, SIGINT, 이벤트 스토어
```

### Oxios의 Evolve (ouroboros_engine.rs 내 ~30줄)

```rust
// LLM 한 번에 "개선된 Seed 만들어줘"
let raw = self.llm_complete(EVOLVE_SYSTEM_PROMPT, &user_message).await?;
let parsed: SeedResponse = Self::parse_json(&raw)...
```

**비교**: 원본은 Wonder → Reflect → SeedGenerator의 3단계 파이프라인.
Oxios은 LLM 1회 호출로 전부 처리. Wonder(인식론적 질문)도, 
온톨로지 진화도, 수렴 판정도 없다.

### 실제 결과 (Seed 데이터로 확인)

```
"Create benchmark_test.txt" → gen 1→2→3 → goal 동일 → 의미 없는 반복
"Clarify file location"     → gen 1→2→3 → goal 동일 → 의미 없는 반복
"Task from user input"      → gen 0→1   → 깨진 Seed이 계속 깨짐
```

**결론: Oxios의 Evolve는 원본의 이론적 기반(Wonder+Reflect)이 빠진, 
LLM 재시도일 뿐이다. 실제로 개선을 만들어내지 못한다.**

---

## 최종 수정안

### ✅ 1. Interview 최대 라운드 (보험)

원본에서도 interview는 Gen 1에만 해당. Gen 2+는 Wonder+Reflect로 자율 진화.
Oxios도 Interview → Seed까지만 사용자 대화. 이후는 자동.

**수정**:
- `InterviewSession`에 `round: u32` 추가
- `OrchestratorConfig`에 `max_interview_rounds: u32` 추가 (기본 3)
- 3라운드 초과 시 "이해한 내용으로 진행" + 강제 ready

### ✅ 2. Seed에 사용자 원문 보존

원본 Seed는 `frozen=True` Pydantic 모델. 한 번 생성되면 절대 수정 불가.
Oxios Seed도 불변이지만, **원문을 보존하지 않는다**.

원본은 interview 전체 대화를 SeedMetadata에 `interview_id`로 추적.
Oxios은 interview_id도 없고, 원문도 Seed에 없다.

**수정**:
- `Seed`에 `original_request: String` 추가 (`#[serde(default)]`)
- `generate_seed()`에서 `interview.original_message` 저장
- `build_system_prompt()`에 원문 주입

### ✅ 3. Evolve 루프 제거

**이것이 가장 중요한 변경이다.**

원본의 Evolve는 Wonder(422줄) + Reflect(467줄) + Convergence(369줄) + 
Watchdog(610줄) + Loop(1925줄) = **3,793줄의 정교한 시스템**이다.

Oxios의 Evolve는 ~30줄의 LLM 호출이다. 이것은 원본의 Evolve가 아니다.
원본의 이론적 기반 없이 Evolve만 흉내 내면, 
비용만 추가하고 개선은 만들어내지 못한다.

**두 가지 선택지**:

#### 선택지 A: Evolve 완전 제거 (권장)

```
Interview → Seed → Execute → Evaluate → 결과 보고 (끝)
```

- 코드 ~60줄 감소 (orchestrator evolve 루프)
- LLM 호출 50% 절감
- 사용자가 재시도 원하면 더 명확한 메시지로 새 요청

#### 선택지 B: Wonder+Reflect를 제대로 구현

원본의 핵심을 제대로 가져오려면:
- `WonderEngine`: 실행 결과를 보고 "아직 모르는 것" 질문
- `ReflectEngine`: Wonder + 평가 결과 → 개선된 AC + 온톨로지
- `ConvergenceCriteria`: 수렴/정체/발진 판정
- 최소 ~500줄 추가
- 하지만 이건 Oxios의 "가볍게 툭툭" 사용 패턴에 over-engineering

**선택지 A가 맞다.** 원본의 Evolve는 무거운 소프트웨어 프로젝트용이다.
Oxios의 사용자는 "이거 고쳐줘" 수준의 가벼운 요청을 한다.
여기에 Wonder→Reflect→Convergence 파이프라인은 과하다.

### ✅ 4. Evolve 대신 "결과 보고 + 사용자 판단"

Evolve 루프를 제거하고, evaluate 결과를 그대로 사용자에게 보여준다:

```rust
// Evaluate 완료 후
Ok(OrchestrationResult {
    response: if evaluation.all_passed() {
        format!("✅ '{}'\n{}", seed.goal, format_notes(&evaluation))
    } else {
        format!(
            "⚠️ '{}'을(를) 시도했지만 완전히 성공하지 못했습니다.\n\
             평가: {:.0}%\n\
             \n\
             더 자세히 알려주시면 다시 시도할 수 있습니다.",
            seed.goal,
            evaluation.score * 100.0,
        )
    },
    evaluation_passed: evaluation.all_passed(),
    ..
})
```

사용자가 "다시 해줘, 이번엔 OO도 포함해줘"라고 하면 
새 Interview → Seed → Execute 사이클이 시작된다.
**이게 사용자 관점에서는 evolve보다 낫다.** 
사용자가 뭘 고쳐야 하는지 알고, 직접 방향을 결정한다.

---

## 수정하지 않는 것

| 항목 | 이유 |
|------|------|
| Interview 분류 | 11/11 정확 |
| AmbiguityScore 가중치 | 원본과 동일 (40/30/30) |
| "Be generous" 프롬프트 | 시나리오 테스트에서 잘 동작 |
| Mechanical evaluation | LLM 호출 스킵 최적화 |
| Semantic evaluation | Evolve 제거 후 정보 제공 목적으로 적합 |
| OuroborosProtocol 트레이트 | evolve() 메서드는 유지 (호출만 안 함) |
| 5-phase Phase enum | Evolve는 enum에 남김. 나중에 제대로 구현할 수도 있음 |

## 원본에서 나중에 고려할 것

| 원본 기능 | 언제 필요한가 | 현재 필요성 |
|-----------|-------------|------------|
| Wonder Engine | 복잡한 멀티세대 프로젝트 | 낮음 |
| Reflect Engine | 온톨로지 진화가 필요한 경우 | 낮음 |
| OntologyDelta | 세대간 지식 변화 추적 | 낮음 |
| 3-Stage Consensus | 고신뢰 평가 필요 시 | 중간 |
| Brownfield 분석 | 기존 코드 기반 작업 | 중간 |
| Watchdog | 장시간 실행 작업 | 중간 |

## 요약

```
원본: 171K줄, Wonder→Reflect→Convergence, max 30세대
Oxios: 1.7K줄, LLM 1회 호출, max 3세대

Oxios의 Evolve는 원본의 Evolve가 아니다.
원본의 이론적 핵심(Wonder+Reflect)이 빠진 LLM 재시도일 뿐.
실제 데이터에서도 개선을 만들어내지 못한다.

→ Evolve 루프를 제거하고, Evaluate 결과를 사용자에게 보고.
→ 사용자가 재시도를 원하면 새 Interview로 명확한 방향 제공.
→ Interview 최대 라운드와 Seed 원문 보존은 추가.
```
