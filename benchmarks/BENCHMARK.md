# Oxios Agent Experience Benchmark

> pi-agent가 일반 사용자처럼 oxios를 사용하는 경험을 평가하는 벤치마크 시스템.

## 1. 개요

### 목적
- pi-agent가 **oxios CLI를 통해** 실제 작업을 수행하는 능력 평가
- 일반 인간 사용자가 겪을 **사용자 경험(UX) 품질** 측정
- 오류 처리, 복구, 다중 턴 대화 등 **현실적 시나리오**에서의 견고성 검증

### 평가 대상
```
pi-agent → oxios CLI (run, chat, config, pkg, agent, daemon, audit, git, budget)
```

### 평가자
- **pi-agent**가 직접 시나리오를 수행
- 결과는 자동으로 수집되어 보고서 생성

---

## 2. 벤치마크 러너 아키텍처

```
benchmarks/
├── BENCHMARK.md           # 이 문서 (시나리오 정의)
├── runner.sh              # 메인 러너 스크립트
├── scenarios/             # 개별 시나리오 스크립트
│   ├── s01-first-run.sh
│   ├── s02-single-prompt.sh
│   ├── ...
│   └── s20-error-recovery.sh
├── evaluator/             # 평가 로직
│   ├── evaluate.py        # 결과 채점
│   └── criteria.py        # 평가 기준 정의
├── reports/               # 생성된 보고서 (gitignored)
│   └── benchmark-YYYY-MM-DD-HHMMSS/
│       ├── summary.md     # 요약 보고서
│       ├── s01-first-run.log
│       ├── s01-first-run.result.json
│       └── ...
└── README.md              # 실행 방법
```

### 실행 흐름

```
1. runner.sh 시작
   ├── 타임스탬프로 보고서 디렉토리 생성
   ├── oxios 상태 초기화 (백업 → 클린업)
   │
2. 시나리오 순차 실행
   ├── 각 시나리오 스크립트 실행
   ├── stdout/stderr + exit code 캡처
   ├── JSON 결과 파일 생성
   │
3. 평가
   ├── 각 시나리오 결과를 criteria에 대조
   ├── 점수 산출 (0-10)
   │
4. 보고서 생성
   ├── 종합 summary.md
   ├── 시나리오별 상세 로그
   └── JSON 결과 아카이브
```

---

## 3. 평가 기준 (Scoring Criteria)

### 3.1 기본 채점 (시나리오별 0-10점)

| 지표 | 가중치 | 설명 |
|------|--------|------|
| **Task Completion** | 40% | 시나리오의 핵심 목표를 달성했는가 |
| **Correctness** | 20% | 출력/결과가 정확한가 (JSON 파싱, 명령어 성공 등) |
| **Error Handling** | 15% | 오류 상황에서 적절히 대처했는가 |
| **Efficiency** | 10% | 불필요한 재시도 없이 효율적으로 수행했는가 |
| **Output Quality** | 15% | 응답의 가독성, 구조화, 유용성 |

### 3.2 보너스/페널티

| 항목 | 점수 |
|------|------|
| 자가 복구 성공 (오류 후 정상 복귀) | +1 |
| 불필요한 권한 요청 | -1 |
| 도움말/문서를 스스로 찾아 해결 | +1 |
| 타임아웃 발생 | -2 |
| 시스템 손상 (워크스페이스 오염 등) | -3 |

### 3.3 총점 등급

| 점수 | 등급 | 의미 |
|------|------|------|
| 90-100 | S | 프로덕션 레디 |
| 80-89 | A | 우수 |
| 70-79 | B | 양호 |
| 60-69 | C | 개선 필요 |
| <60 | D | 재설계 필요 |

---

## 4. 벤치마크 시나리오 (20개)

### Tier 1: 기본 사용법 (Basic Usage)
> 첫 설치 직후 일반 사용자가 겪는 경험.

---

#### S01: First Run & Setup
**목표:** oxios를 처음 실행하고 기본 상태를 확인한다.

```bash
# 사용자 관점에서의 행동:
oxios                  # 데몬 시작 (또는 포그라운드)
oxios status           # 상태 확인
oxios config show      # 기본 설정 확인
```

**평가 포인트:**
- [ ] 데몬이 정상 시작되는가
- [ ] status 명령이 응답하는가
- [ ] config show가 TOML을 정상 출력하는가
- [ ] 에러 메시지가 이해하기 쉬운가

**성공 기준:** 모든 명령이 exit code 0으로 완료.

---

#### S02: Single Prompt Execution
**목표:** 간단한 프롬프트를 실행하고 응답을 받는다.

```bash
oxios run "Rust에서 Hello World를 출력하는 코드를 작성해줘"
```

**평가 포인트:**
- [ ] 응답이 생성되는가
- [ ] 응답 내용이 질문과 관련 있는가
- [ ] 합리적인 시간 내에 완료되는가 (60초 이내)

**성공 기준:** 유효한 Rust Hello World 코드 포함 응답.

---

#### S03: JSON Output
**목표:** --json 플래그로 구조화된 출력을 얻는다.

```bash
RESULT=$(oxios run --json "1부터 10까지의 합을 구해줘")
echo $RESULT | jq .response
echo $RESULT | jq .session_id
echo $RESULT | jq .evaluation_passed
```

**평가 포인트:**
- [ ] 출력이 유효한 JSON인가
- [ ] 필수 필드(response, session_id, evaluation_passed)가 존재하는가
- [ ] 수학적 정답(55)이 응답에 포함되는가

**성공 기준:** `jq` 파싱 성공 + 정답 포함.

---

#### S04: Context File Input
**목표:** 파일을 컨텍스트로 전달하여 분석을 수행한다.

```bash
# 준비: 테스트 파일 생성
echo 'fn fibonacci(n: u32) -> u64 {
    if n <= 1 { return n as u64; }
    fibonacci(n - 1) + fibonacci(n - 2)
}' > /tmp/fib.rs

oxios run --context-file /tmp/fib.rs "이 코드의 시간 복잡도를 분석하고 최적화된 버전을 제안해줘"
```

**평가 포인트:**
- [ ] 파일 내용이 컨텍스트로 전달되는가
- [ ] 응답이 파일 내용을 기반으로 작성되었는가
- [ ] 시간복잡도 분석(O(2^n))이 정확한가

**성공 기준:** 정확한 복잡도 분석 + 최적화 제안 포함.

---

#### S05: Multi-Turn Conversation
**목표:** 세션을 이어서 대화를 진행한다.

```bash
# 첫 번째 턴
RESP1=$(oxios run --json "간단한 TODO 리스트 Rust 구조체를 만들어줘")
SID=$(echo $RESP1 | jq -r '.session_id')

# 두 번째 턴 (이어서)
RESP2=$(oxios run --json --session "$SID" "여기에 완료 상태를 토글하는 메서드를 추가해줘")

# 세 번째 턴 (이어서)
RESP3=$(oxios run --json --session "$SID" "이 구조체에 대한 간단한 테스트 코드도 작성해줘")
```

**평가 포인트:**
- [ ] session_id가 일관되게 반환되는가
- [ ] 후속 턴이 이전 대화 맥락을 유지하는가
- [ ] 각 턴의 응답이 누적 컨텍스트를 반영하는가

**성공 기준:** 3턴 모두 성공 + 컨텍스트 연속성 확인.

---

### Tier 2: 프로그램 & 도구 (Programs & Tools)
> oxios의 핵심 기능인 프로그램 시스템을 활용하는 경험.

---

#### S06: Program Installation
**목표:** 내장 프로그램을 설치하고 확인한다.

```bash
# 프로그램 설치
oxios pkg install .programs/code-review

# 설치 확인
oxios pkg list

# 프로그램 상세 보기
oxios program code-review
```

**평가 포인트:**
- [ ] 설치가 성공하는가
- [ ] pkg list에 표시되는가
- [ ] program 명령으로 SKILL.md 내용을 볼 수 있는가

**성공 기준:** 설치 → 목록 표시 → 상세 조회 모두 성공.

---

#### S07: Code Review Program Usage
**목표:** code-review 프로그램을 통해 코드 리뷰를 수행한다.

```bash
# 준비: 리뷰할 코드가 있는 가상 프로젝트
cat > /tmp/review_target.rs << 'EOF'
fn main() {
    let password = "admin123";
    let sql = format!("SELECT * FROM users WHERE name = '{}'", std::env::args().nth(1).unwrap());
    println!("{}", sql);
    unsafe {
        let ptr = std::ptr::null::<u8>();
        println!("{}", *ptr);
    }
}
EOF

oxios run --context-file /tmp/review_target.rs "이 코드를 리뷰해줘. 보안 문제와 버그를 찾아줘."
```

**평가 포인트:**
- [ ] 하드코딩된 비밀번호를 감지하는가
- [ ] SQL 인젝션 취약점을 지적하는가
- [ ] null 포인터 역참조를 발견하는가
- [ ] 개선 제안이 실용적인가

**성공 기준:** 3개 이상의 보안/버그 이슈 식별.

---

#### S08: Multiple Program Install
**목표:** 여러 프로그램을 설치하고 관리한다.

```bash
# 여러 프로그램 순차 설치
oxios pkg install .programs/debug
oxios pkg install .programs/deploy
oxios pkg install .programs/refactor
oxios pkg install .programs/guardian

# 전체 목록 확인
oxios pkg list

# 검색
oxios pkg search
```

**평가 포인트:**
- [ ] 모든 프로그램이 정상 설치되는가
- [ ] list에 전부 표시되는가
- [ ] search가 설치된 프로그램의 정보를 보여주는가

**성공 기준:** 4개 프로그램 모두 설치 → 목록 확인.

---

#### S09: Program Uninstall
**목표:** 프로그램을 제거하고 상태를 확인한다.

```bash
# 제거
oxios pkg uninstall debug

# 확인
oxios pkg list
```

**평가 포인트:**
- [ ] 제거가 성공하는가
- [ ] list에서 사라졌는가
- [ ] 다른 프로그램은 영향을 받지 않는가

**성공 기준:** debug만 제거되고 나머지는 유지.

---

#### S10: Program Creator — Make a Custom Program
**목표:** program-creator를 사용해 새 프로그램을 만든다.

```bash
oxios pkg install .programs/program-creator

RESP=$(oxios run --json "이름이 'weather-check'인 프로그램을 만들어줘. 날씨 API를 호출해서 현재 날씨를 알려주는 기능이야. program.toml과 SKILL.md를 포함해줘.")

# 결과 확인 — 프로그램이 생성되었는지 확인
oxios pkg list
```

**평가 포인트:**
- [ ] program.toml 형식이 올바른가
- [ ] SKILL.md가 작성되었는가
- [ ] 프로그램이 설치 가능한 형태인가

**성공 기준:** 유효한 프로그램 구조 생성.

---

### Tier 3: 설정 & 관리 (Configuration & Management)
> 설정 변경, 에이전트 관리, 감사 로그 등 관리자 수준 작업.

---

#### S11: Configuration Change
**목표:** 설정을 조회하고 변경한다.

```bash
# 현재 설정 확인
oxios config get gateway.port
oxios config get engine.default_model

# 설정 변경
oxios config set gateway.port 8080

# 변경 확인
oxios config get gateway.port

# 전체 설정 조회
oxios config show
```

**평가 포인트:**
- [ ] get이 올바른 값을 반환하는가
- [ ] set이 변경을 영속화하는가
- [ ] show가 전체 설정을 보여주는가
- [ ] 존재하지 않는 키에 대한 에러 처리가 적절한가

**성공 기준:** get → set → get 변경 확인.

---

#### S12: Agent Lifecycle
**목표:** 에이전트를 실행하고 관리한다.

```bash
# 에이전트 목록 (초기)
oxios agent list

# 프롬프트 실행 (에이전트 생성)
oxios run --json "현재 시간을 기준으로 간단한 일기 형식의 메시지를 작성해줘" &

# 에이전트 목록 (실행 중)
sleep 2
oxios agent list

# 데몬 재시작
oxios restart

# 상태 확인
oxios status
```

**평가 포인트:**
- [ ] agent list가 동작하는가
- [ ] 에이전트가 목록에 나타나는가
- [ ] restart가 정상 동작하는가
- [ ] status가 정확한 정보를 보여주는가

**성공 기준:** agent list → restart → status 모두 성공.

---

#### S13: Audit Trail Verification
**목표:** 감사 로그를 확인하고 무결성을 검증한다.

```bash
# 여러 작업 수행 (감사 엔트리 생성)
oxios run --json "안녕하세요" > /dev/null
oxios config show > /dev/null
oxios pkg list > /dev/null

# 감사 로그 확인
oxios audit

# 감사 무결성 검증
oxios audit  # (무결성 검증이 포함된 경우)
```

**평가 포인트:**
- [ ] audit 명령이 로그를 출력하는가
- [ ] 수행한 작업이 로그에 기록되어 있는가
- [ ] 타임스탬프가 유효한가

**성공 기준:** 수행한 작업 3개 이상이 감사 로그에 기록됨.

---

#### S14: Git Operations
**목표:** 상태 저장소의 Git 작업을 수행한다.

```bash
# 변경사항 만들기
oxios config set gateway.port 4201

# Git 로그 확인
oxios git log

# 태그 생성
oxios git tag "bench-test-v1" --message "벤치마크 테스트 태그"

# 로그에서 태그 확인
oxios git log
```

**평가 포인트:**
- [ ] git log가 커밋 기록을 보여주는가
- [ ] 태그가 생성되는가
- [ ] 로그에 태그가 반영되는가

**성공 기준:** log → tag → log에 태그 표시.

---

#### S15: Budget Check
**목표:** 에이전트 예산 정보를 확인한다.

```bash
# 전체 예산
oxios budget

# 특정 에이전트 예산 (ID가 있는 경우)
# oxios budget <agent_id>
```

**평가 포인트:**
- [ ] budget 명령이 응답하는가
- [ ] 토큰 사용량 정보가 표시되는가
- [ ] 에이전트 ID를 지정했을 때 상세 정보가 나오는가

**성공 기준:** 예산 정보 출력 성공.

---

### Tier 4: 오류 복구 (Error Recovery)
> 잘못된 입력, 존재하지 않는 리소스, 권한 문제 등에 대한 대응.

---

#### S16: Invalid Commands & Arguments
**목표:** 잘못된 명령어에 대한 에러 메시지 품질을 평가한다.

```bash
# 존재하지 않는 명령어
oxios nonexistent

# 필수 인자 누락
oxios run

# 잘못된 설정 키
oxios config get totally.invalid.key

# 존재하지 않는 프로그램 제거
oxios pkg uninstall no-such-program

# 잘못된 세션 ID
oxios run --session "invalid-uuid" "test"
```

**평가 포인트:**
- [ ] 각각에 대해 이해 가능한 에러 메시지가 출력되는가
- [ ] exit code가 0이 아닌가
- [ ] 크래시 없이 정상 종료되는가
- [ ] 에러 메시지가 해결 방법을 제시하는가

**성공 기준:** 모든 케이스에서 크래시 없이 의미 있는 에러 메시지.

---

#### S17: Missing Credentials
**목표:** API 키 없이 실행할 때의 경험.

```bash
# 백업
cp ~/.oxios/config.toml /tmp/oxios-config-backup.toml

# API 키 제거 시뮬레이션
# (환경변수를 임시로 제거하고 실행)
ANTHROPIC_API_KEY="" OPENAI_API_KEY="" oxios run --json "테스트"

# 복구
cp /tmp/oxios-config-backup.toml ~/.oxios/config.toml
```

**평가 포인트:**
- [ ] 명확한 에러 메시지가 출력되는가
- [ ] 어떤 API 키가 필요한지 안내하는가
- [ ] 설정 방법을 제시하는가

**성공 기준:** "API 키가 필요합니다" 또는 동등한 안내 메시지.

---

#### S18: Workspace Corruption Recovery
**목표:** 워크스페이스가 손상된 상태에서 복구한다.

```bash
# 백업
oxios backup --output /tmp/oxios-backup.tar

# 워크스페이스 일부 손상 시뮬레이션
rm -rf ~/.oxios/workspace/sessions/*

# 복구 시도
oxios restore /tmp/oxios-backup.tar

# 상태 확인
oxios status
```

**평가 포인트:**
- [ ] backup이 정상 동작하는가
- [ ] 손상 상태에서 복구가 가능한가
- [ ] restore 후 정상 상태로 돌아오는가

**성공 기준:** backup → 손상 → restore → 정상 상태.

---

#### S19: Concurrent Execution Stress
**목표:** 여러 프롬프트를 동시에 실행한다.

```bash
# 3개 동시 실행
oxios run --json "첫 번째: 짧은 시를 써줘" > /tmp/bench-1.json &
oxios run --json "두 번째: 피보나치 수열 10개를 출력해줘" > /tmp/bench-2.json &
oxios run --json "세 번째: 'Hello World'를 5개 언어로 써줘" > /tmp/bench-3.json &

wait

# 결과 확인
cat /tmp/bench-1.json | jq .evaluation_passed
cat /tmp/bench-2.json | jq .evaluation_passed
cat /tmp/bench-3.json | jq .evaluation_passed
```

**평가 포인트:**
- [ ] 모든 실행이 완료되는가
- [ ] 스케줄러가 동시 실행을 처리하는가
- [ ] max_concurrent 제한이 적용되는가
- [ ] 응답이 섞이지 않고 독립적인가

**성공 기준:** 3개 모두 evaluation_passed: true.

---

#### S20: Exit Code Based Workflow
**목표:** --exit-code 플래그를 활용한 스크립트 통합.

```bash
# 성공 케이스
oxios run --exit-code --json "1+1은 얼마야?"
echo "Exit code: $?"

# 평가 실패 케이스 (의도적으로 애매한 프롬프트)
oxios run --exit-code --json "불가능한 것을 만들어줘: 영구 기관"
echo "Exit code: $?"

# 스크립트 내 활용
if oxios run --exit-code --json "Rust 프로젝트를 초기화하는 명령어를 알려줘"; then
    echo "평가 통과"
else
    echo "평가 실패 — 재시도 필요"
fi
```

**평가 포인트:**
- [ ] exit code가 올바르게 설정되는가
- [ ] --exit-code + --json 조합이 동작하는가
- [ ] 스크립트 내에서 if문과 함께 사용 가능한가

**성공 기준:** 성공 시 0, 실패 시 1 반환.

---

## 5. 시나리오별 가중치

| Tier | 시나리오 | 가중치 | 카테고리 |
|------|----------|--------|----------|
| 1 | S01 | 1.0x | 기본 |
| 1 | S02 | 1.0x | 기본 |
| 1 | S03 | 1.0x | 기본 |
| 1 | S04 | 1.0x | 기본 |
| 1 | S05 | 1.5x | 기본 (멀티턴 중요) |
| 2 | S06 | 1.0x | 프로그램 |
| 2 | S07 | 1.5x | 프로그램 (실사용) |
| 2 | S08 | 1.0x | 프로그램 |
| 2 | S09 | 0.5x | 프로그램 |
| 2 | S10 | 1.5x | 프로그램 (창작) |
| 3 | S11 | 1.0x | 관리 |
| 3 | S12 | 1.5x | 관리 (에이전트) |
| 3 | S13 | 1.0x | 관리 (감사) |
| 3 | S14 | 0.5x | 관리 (Git) |
| 3 | S15 | 0.5x | 관리 (예산) |
| 4 | S16 | 1.5x | 오류 복구 |
| 4 | S17 | 1.0x | 오류 복구 |
| 4 | S18 | 1.0x | 오류 복구 |
| 4 | S19 | 1.5x | 오류 복구 (스트레스) |
| 4 | S20 | 1.0x | 오류 복구 |

**총 만점:** 100점 (가중치 정규화 후)

---

## 6. 보고서 형식

### 6.1 summary.md 구조

```markdown
# Oxios Benchmark Report

**일시:** 2026-05-16 14:30:00
**oxios 버전:** 0.3.0-alpha
**런타임:** macOS / Rust 1.xx
**총 소요 시간:** XX분 YY초

---

## 종합 결과

| Tier | 점수 | 등급 |
|------|------|------|
| Tier 1: 기본 사용법 | XX/100 | A |
| Tier 2: 프로그램 & 도구 | XX/100 | B |
| Tier 3: 설정 & 관리 | XX/100 | A |
| Tier 4: 오류 복구 | XX/100 | C |
| **총점** | **XX/100** | **B** |

---

## 시나리오별 결과

| ID | 시나리오 | 점수 | 소요시간 | exit_code | 비고 |
|----|----------|------|----------|-----------|------|
| S01 | First Run & Setup | 10/10 | 3s | 0 | 완벽 |
| S02 | Single Prompt | 8/10 | 12s | 0 | 응답 약간 느림 |
| ... | ... | ... | ... | ... | ... |

---

## 상세 이슈

### S16: Invalid Commands — 점수 6/10

**문제:**
- `oxios run` (인자 없음) 시 segfault 발생
- 에러 메시지에 해결 방법 미포함

**로그 발췌:**
\`\`\`
thread 'main' panicked at 'assertion failed: ...'
\`\`\`

**개선 제안:**
- clap의 arg_required_else_help 활용
- 에러 메시지에 usage 힌트 추가

---

## 추천 사항

1. **[Critical]** S16 segfault 수정 필요
2. **[Medium]** S05 세션 지속성 개선
3. **[Low]** S15 예산 출력 포맷 개선
```

### 6.2 시나리오별 result.json

```json
{
  "scenario_id": "S01",
  "scenario_name": "First Run & Setup",
  "timestamp": "2026-05-16T14:30:01Z",
  "duration_ms": 3200,
  "exit_code": 0,
  "steps": [
    {
      "command": "oxios",
      "exit_code": 0,
      "stdout": "...",
      "stderr": "",
      "duration_ms": 1200
    },
    {
      "command": "oxios status",
      "exit_code": 0,
      "stdout": "Daemon: running\n...",
      "stderr": "",
      "duration_ms": 800
    }
  ],
  "checks": {
    "daemon_started": true,
    "status_responds": true,
    "config_valid_toml": true,
    "error_messages_readable": true
  },
  "score": 10,
  "max_score": 10,
  "notes": ""
}
```

---

## 7. pi-agent 실행 지침

이 벤치마크는 pi-agent가 **직접** 수행한다. 다음 지침을 따른다:

### 실행 전
1. `oxios`가 설치되어 있는지 확인 (`which oxios`)
2. API 키가 설정되어 있는지 확인
3. 워크스페이스 초기 상태 확인 (`~/.oxios/` 백업)

### 실행 중
1. 각 시나리오를 순서대로 실행
2. 모든 stdout/stderr를 그대로 기록
3. exit code를 캡처
4. 예상치 못한 동작이나 크래시 발생 시 상황을 상세히 기록
5. **절대**: 시나리오를 수정하거나 건너뛰지 않음
6. **절대**: 실패한 명령을 재시도하여 성공한 것처럼 보고하지 않음

### 실행 후
1. 각 시나리오의 결과를 result.json으로 정리
2. 종합 summary.md 작성
3. 발견된 버그와 개선 사항을 이슈로 정리

---

## 8. 확장 시나리오 (향후 추가)

| ID | 시나리오 | 설명 |
|----|----------|------|
| E01 | Cron Job Scheduling | 시간 기반 작업 예약 |
| E02 | A2A Communication | 에이전트 간 통신 |
| E03 | Memory Persistence | 세션 간 메모리 유지 |
| E04 | Web Dashboard | 브라우저 기반 UI 사용 |
| E05 | Custom Model Switch | 모델 전환 (GPT-4, Claude 등) |
| E06 | Structured Exec | allowlist 기반 명령 실행 |
| E07 | Rate Limiting | 스케줄러 레이트 리밋 동작 |
| E08 | Guardian Program | 백그라운드 감시 프로그램 |
| E09 | Telegram Channel | 텔레그램 채널 연동 |
| E10 | Large Context | 대용량 파일 분석 (1000줄+) |
