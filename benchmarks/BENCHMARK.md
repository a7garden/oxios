# Oxios Agent Experience Benchmark

> pi-agent가 일반 사용자처럼 oxios를 사용하는 경험을 평가하는 벤치마크.
> SWE-bench, WebArena, AgentBench 등 2025-2026 현대적 에이전트 벤치마크 방법론 적용.

## 1. 설계 철학

### 이전 버전의 문제
- bash 스크립트가 명령어를 나열 → 에이전트의 의사결정 과정이 없음
- `grep`으로 품질 평가 → "fn main이 아닙니다"에도 통과
- 스크립트가 실행하고 사용자는 관찰만 → "사용자 경험"이 아님

### 이번 설계 원칙

| 원칙 | 구현 |
|------|------|
| **목표만 주고 방법은 자율** | 시나리오는 "하고 싶은 것"만 정의. 명령어는 지정하지 않음 |
| **LLM-as-Judge** | 평가는 LLM이 의미적으로 판단. grep 없음 |
| **Trajectory 전체 기록** | 명령, 응답, 시간, 에러, 판단 전 과정을 궤적(trajectory)으로 기록 |
| **Rubric 기반 다차원 평가** | Completion·Quality·Efficiency·Recovery 4차원 + 증거 기반 |

---

## 2. 아키텍처

```
┌──────────────────────────────────────────────────────────┐
│                    run.py (Orchestrator)                  │
│                                                          │
│  ┌─────────────┐    goal(한국어)    ┌─────────────────┐  │
│  │  Scenario   │ ────────────────→ │  oxios run       │  │
│  │  (TOML)     │   "코드 리뷰해줘"  │  --json "goal"   │  │
│  └─────────────┘                   └────────┬────────┘  │
│                                              │            │
│                                    JSON response          │
│                                              ▼            │
│                                    ┌─────────────────┐   │
│                                    │  Recorder        │   │
│                                    │  (Trajectory)    │   │
│                                    │  명령·응답·시간   │   │
│                                    │  에러·판단 기록   │   │
│                                    └────────┬────────┘   │
│                                              │            │
│                                    trajectory + rubric    │
│                                              ▼            │
│                                    ┌─────────────────┐   │
│                                    │  LLM-as-Judge    │   │
│                                    │  4차원 평가       │   │
│                                    │  증거 기반 채점   │   │
│                                    └────────┬────────┘   │
│                                              │            │
│                                              ▼            │
│                                    ┌─────────────────┐   │
│                                    │  Reporter        │   │
│                                    │  Markdown 보고서  │   │
│                                    └─────────────────┘   │
└──────────────────────────────────────────────────────────┘
```

### 파일 구조

```
benchmarks/
├── BENCHMARK.md              # 이 문서
├── README.md                 # 실행 방법
├── run.py                    # 메인 오케스트레이터
├── recorder.py               # Trajectory 기록기
├── judge.py                  # LLM-as-Judge 평가기
├── report.py                 # 보고서 생성기
├── scenarios/                # 시나리오 정의 (TOML)
│   ├── s01.toml              # Tier 1: 기본
│   ├── s02.toml
│   ├── ...
│   └── s15.toml              # Tier 3: 오류 복구
├── judge/
│   └── rubrics.toml          # 평가 루브릭 정의
├── templates/
│   └── judge-prompt.md       # Judge에게 줄 프롬프트 템플릿
└── reports/                  # 생성된 보고서 (gitignore)
```

---

## 3. 시나리오 정의 형식 (TOML)

```toml
[scenario]
id = "s01"
name = "Hello Oxios"
tier = 1          # 1=기본, 2=프로그램, 3=오류복구
difficulty = "easy"
time_limit_secs = 60

# 사용자가 oxios에게 말하는 것 (자연어 목표)
# pi-agent는 이것을 그대로 oxios에 전달
goal = "Rust로 Hello World 프로그램을 작성해줘"

# 필요한 사전 설정 (선택사항)
[setup]
# files = [{ path = "/tmp/test.rs", content = "..." }]
# env_vars = {}

# 평가 기준 — Judge가 참조
[evaluation]
# 최소 기대 결과 (LLM이 판단 참고용, grep 아님)
min_expected = "응답에 유효한 Rust 코드가 포함되어야 함"

# 질적 기대 (LLM이 판단)
quality_expected = "코드가 컴파일 가능해야 하고, 설명이 이해하기 쉬워야 함"

# 루브릭 오버라이드 (선택사항)
# rubric_override = { efficiency = { weight = 0.3 } }
```

---

## 4. 평가 루브릭 (4차원)

### 4.1 Completion (목표 달성) — 가중치 40%
> 사용자가 원하는 것을 실제로 얻었는가?

| 점수 | 기준 |
|------|------|
| 10 | 목표 완벽 달성. 추가 통찰까지 제공 |
| 8 | 목표 달성. 사소한 누락만 있음 |
| 5 | 부분 달성. 핵심은 있으나 불완전 |
| 2 | 거의 달성 못함. 관련은 있음 |
| 0 | 완전 실패 또는 무관련 응답 |

### 4.2 Quality (응답 품질) — 가중치 25%
> 응답이 정확하고, 구조화되어 있고, 이해하기 쉬운가?

| 점수 | 기준 |
|------|------|
| 10 | 정확·구조화·가독성 모두 우수. 전문가 수준 |
| 8 | 대부분 정확. 구조 양호 |
| 5 | 정확성은 있으나 구조/가독성 부족 |
| 2 | 부정확하거나 혼란스러움 |
| 0 | 완전히 틀리거나 이해 불가 |

### 4.3 Efficiency (효율성) — 가중치 15%
> 합리적인 시간과 단계 내에 완료되었는가?

| 점수 | 기준 |
|------|------|
| 10 | 예상 시간 내 완료. 불필요한 단계 없음 |
| 7 | 약간 느리거나 단계가 많으나 허용 범위 |
| 4 | 비효율적. 동일 목표에 절반 시간 가능 |
| 0 | 타임아웃 또는 극도로 비효율 |

### 4.4 Recovery (오류 대응) — 가중치 20%
> 문제 발생 시 적절히 대처하는가? (정상 시 N/A → 10점)

| 점수 | 기준 |
|------|------|
| 10 | 오류 없음, 또는 오류 후 완벽 자가 복구 |
| 7 | 오류 후 대부분 복구. 사용자 개입 약간 필요 |
| 4 | 오류 후 부분 복구만 |
| 0 | 오류 발생 후 복구 불가 또는 악화 |

---

## 5. LLM-as-Judge 구조

### 5.1 Judge 입력

Judge는 다음을 받는다:

```json
{
  "scenario": {
    "id": "s01",
    "name": "Hello Oxios",
    "goal": "Rust로 Hello World 프로그램을 작성해줘",
    "min_expected": "응답에 유효한 Rust 코드가 포함되어야 함",
    "quality_expected": "코드가 컴파일 가능해야 하고, 설명이 이해하기 쉬워야 함"
  },
  "trajectory": {
    "command": "oxios run --json 'Rust로 Hello World 프로그램을 작성해줘'",
    "exit_code": 0,
    "duration_ms": 8500,
    "stdout": "{ \"response\": \"...\", \"evaluation_passed\": true, ... }",
    "stderr": "",
    "parsed_response": "물론입니다! 다음은 Rust로 작성한 Hello World...",
    "phase_reached": "Execute",
    "evaluation_passed": true,
    "session_id": "abc-123"
  }
}
```

### 5.2 Judge 출력

```json
{
  "scores": {
    "completion": { "score": 9, "evidence": "유효한 Rust Hello World 코드를 포함하고 있으며..." },
    "quality": { "score": 8, "evidence": "코드는 정확하며 설명도 함께 제공됨..." },
    "efficiency": { "score": 10, "evidence": "8.5초 만에 완료. 단일 실행으로 목표 달성." },
    "recovery": { "score": 10, "evidence": "오류 없이 정상 완료됨." }
  },
  "weighted_total": 9.15,
  "overall_assessment": "목표를 완벽하게 달성했다. 컴파일 가능한 Rust 코드와 함께...",
  "issues": [],
  "highlights": ["단일 실행으로 목표 달성", "코드 + 설명 모두 제공"]
}
```

### 5.3 신뢰성 확보

| 방법 | 설명 |
|------|------|
| temperature=0 | 결정론적 판정 |
| 증거 강제 | 점수 없이 증거만 먼저 작성 후 점수 부여 |
| 구조화 출력 | JSON 스키마로 일관성 보장 |
| 동일 모델 사용 | 판정 편향 최소화 |

---

## 6. 벤치마크 시나리오 (15개)

### Tier 1: 일상 사용 (Basic) — 5개

| ID | 이름 | 목표 (사용자가 말하는 것) |
|----|------|--------------------------|
| S01 | Hello Oxios | "Rust로 Hello World 프로그램을 작성해줘" |
| S02 | 수학 문제 | "1부터 100까지의 소수를 구하는 코드를 작성하고, 개수도 알려줘" |
| S03 | 파일 분석 | (fibonacci 코드 제공) "이 코드의 시간복잡도를 분석하고 최적화해줘" |
| S04 | 멀티턴 대화 | "간단한 할일 관리 구조체를 만들어줘" → "여기에 완료 토글을 추가해줘" → "테스트도 써줘" |
| S05 | JSON 출력 | "JSON 형태로 한국의 수도, 인구, 면적 정보를 알려줘" |

### Tier 2: oxios 고유 기능 (Features) — 5개

| ID | 이름 | 목표 |
|----|------|------|
| S06 | 프로그램 설치 | "oxios에 코드 리뷰 프로그램을 설치해줘" |
| S07 | 코드 리뷰 | (취약한 코드 제공) "이 코드에 있는 보안 문제를 찾아줘" |
| S08 | 설정 변경 | "포트를 8080으로 바꿔줘" |
| S09 | 커스텀 프로그램 | "'translator'라는 프로그램을 만들어줘. 한국어를 영어로 번역하는 기능이야" |
| S10 | 프로그램 제거 | "방금 설치한 translator 프로그램을 지워줘" |

### Tier 3: 오류 상황 (Resilience) — 5개

| ID | 이름 | 상황 |
|----|------|------|
| S11 | 없는 명령어 | "oxios에 'deploy' 명령어를 실행해줘" (미설치 상태) |
| S12 | 빈 입력 | "oxios를 아무 인자 없이 실행해줘" |
| S13 | 잘못된 설정 | "존재하지 않는 설정 키 'magic.wand'를 조회해줘" |
| S14 | 컨텍스트 없는 질문 | "이 코드에서 버그를 찾아줘" (아무 파일도 안 줌) |
| S15 | 동시 요청 | "동시에 3개의 다른 질문을 oxios에 보내줘" |

---

## 7. 보고서 형식

실행 완료 후 생성:

```
reports/benchmark-2026-05-16-143000/
├── summary.md              # 종합 보고서 (사람용)
├── trajectory-s01.json     # 시나리오별 전체 궤적
├── judge-s01.json          # Judge 판정 결과
├── ...
└── raw/
    ├── s01-stdout.json     # oxios run --json 원본 출력
    └── ...
```

### summary.md 구조

```markdown
# Oxios Benchmark Report

## 메타데이터
- 일시, 버전, 런타임, 총 소요시간

## 종합 결과
- 총점 (가중 평균)
- 등급 (S/A/B/C/D)
- 차원별 평균 (Completion/Quality/Efficiency/Recovery)

## 시나리오별 결과표
| ID | 이름 | Completion | Quality | Efficiency | Recovery | 총점 |
|----|------|-----------|---------|------------|----------|------|

## 발견된 이슈 (Judge가 지적)
- [Critical] ...
- [Medium] ...
- [Low] ...

## 강점 (Judge가 언급)
- ...

## 추천 개선사항
- ...
```
