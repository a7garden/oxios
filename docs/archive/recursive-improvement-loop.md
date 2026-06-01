# Oxios Recursive Improvement Loop

> AI가 AI OS를 개선하는 재귀적 시스템.
> pi-agent가 oxios를 직접 CLI로 실행하며 테스트하고, 발견된 문제를 코드로 수정하고, 다시 빌드/설치/테스트하는 무한 루프.

**생성일:** 2026-05-17
**버전:** v1
**상태:** ACTIVE

---

## 1. 철학

```
pi-agent → oxios CLI 테스트 → 버그 발견 → 코드 수정 → 빌드/설치 → 재테스트 → 반복
```

이것은 단순한 테스트가 아니다. **실제 사용자 경험**을 시뮬레이션하는 것이다.
pi-agent가 일반 사용자처럼 oxios를 조작하고, 문제를 발견하면 개발자처럼 수정한다.

원칙:
- **실제 CLI 실행이 곧 테스트다.** 단위 테스트 통과와 실제 동작은 다르다.
- **한 번에 하나씩.** 멀티태스킹하지 않는다. 하나 고치고, 확인하고, 다음으로.
- **커밋은 체크포인트다.** 수정할 때마다 커밋. 롤백 가능해야 한다.
- **문서화가 메모리다.** 다음 세션의 pi-agent가 이 문서를 읽고 바로 이어서 작업할 수 있어야 한다.

---

## 2. 현재 상태 (2026-05-17)

### 2.1 코드베이스

| 지표 | 값 |
|------|-----|
| 총 라인 | ~43,000줄 |
| 소스 파일 | ~90개 (.rs) |
| 테스트 | 501개 (코드상) |
| 워크스페이스 | 8 crate |
| 버전 | 0.1.2 |

### 2.2 빌드 상태

| 항목 | 상태 |
|------|------|
| `cargo build --release` | ✅ 통과 |
| `cargo test --workspace` | ❌ 컴파일 에러 (52개) |
| `cargo clippy` | ⚠️ 경고 있음 |
| `cargo fmt` | ⚠️ 미확인 |

### 2.3 CLI 실행 상태

| 항목 | 상태 | 비고 |
|------|------|------|
| `oxios --version` | ✅ | v0.1.2 |
| `oxios status` | ✅ | 데몬 정상 |
| `oxios doctor` | ✅ | 8/8 통과 |
| `oxios run --json "간단한 질문"` | ✅ | Interview 응답 (15-25초) |
| `oxios run --json "코드 작성"` | ✅ | Execute Phase 진입 성공 (~137초) |
| `oxios chat` | ⚠️ | 미테스트 |
| `oxios config show` | ✅ | 동작 |
| `oxios models` | ⚠️ | 미테스트 |

### 2.4 완료된 패치

| 패치 | 내용 | 상태 |
|------|------|------|
| PATCH-1 | ZAI 엔드포인트 `api.z.ai` + 환경변수 오버라이드 | ✅ 적용됨 |
| PATCH-2 | browser feature 전이 차단 | ✅ 적용됨 |
| PATCH-3 | oxi-ai 직접 의존성 | ✅ 적용됨 |
| PATCH-4 | GitLayer mutex deadlock 수정 | ✅ 적용됨 (commit f99d96b) |

---

## 3. 기능 인벤토리 & 테스트 매트릭스

각 기능을 3단계로 평가:
- **L0** = 컴파일/빌드만 됨 (코드 있음, 실행 안 됨)
- **L1** = CLI로 기본 동작 확인됨
- **L2** = 엣지 케이스까지 테스트됨, 신뢰 가능

### 3.1 코어 프로토콜

| # | 기능 | 모듈 | 레벨 | 블로커 |
|---|------|------|------|--------|
| C01 | Ouroboros Interview | `ouroboros/interview.rs` | **L1** | - |
| C02 | Ouroboros Seed 생성 | `ouroboros/seed.rs` | **L1** | - |
| C03 | Ouroboros Execute | `orchestrator.rs` | **L1** | - |
| C04 | Ouroboros Evaluate | `ouroboros/evaluation.rs` | **L1** | C03 해결 |
| C05 | Ouroboros Evolve | `ouroboros_engine.rs` | **L1** | C03 해결 |
| C06 | Multi-turn 세션 | `orchestrator.rs` | **L0** | C03 해결 |
| C07 | Persona 시스템 | `persona.rs`, `persona_manager.rs` | **L1** | - |

### 3.2 엔진 & 인증

| # | 기능 | 모듈 | 레벨 | 블로커 |
|---|------|------|------|--------|
| E01 | Engine Provider | `engine.rs` | **L1** | - |
| E02 | ZAI 연결 (코딩 플랜) | `engine.rs` | **L1** | - |
| E03 | Credential 해석 | `credential.rs` | **L1** | 환경변수만 지원 |
| E04 | Circuit Breaker | `circuit_breaker.rs` | **L0** | 미테스트 |
| E05 | 다중 Provider | `engine.rs` | **L0** | ZAI만 테스트됨 |

### 3.3 에이전트 시스템

| # | 기능 | 모듈 | 레벨 | 블로커 |
|---|------|------|------|--------|
| A01 | Supervisor (fork/exec/wait/kill) | `supervisor.rs` | **L0** | C03 블로킹 |
| A02 | AgentLifecycleManager | `agent_lifecycle.rs` | **L0** | C03 블로킹 |
| A03 | AgentRuntime (도구 호출 루프) | `agent_runtime.rs` | **L0** | C03 블로킹 |
| A04 | Scheduler (우선순위 큐) | `scheduler.rs` | **L0** | C03 블로킹 |
| A05 | Agent Groups | `agent_group.rs` | **L0** | C03 블로킹 |
| A06 | A2A 프로토콜 | `a2a.rs` | **L0** | C03 블로킹 |

### 3.4 도구 시스템

| # | 기능 | 모듈 | 레벨 | 블로커 |
|---|------|------|------|--------|
| T01 | ExecTool (shell 모드) | `tools/exec_tool.rs` | **L0** | C03 블로킹 |
| T02 | ExecTool (structured 모드) | `tools/exec_tool.rs` | **L0** | C03 블로킹 |
| T03 | BrowserTool | `tools/browser/` | **L0** | browser feature 이슈 |
| T04 | MCP Tool | `tools/mcp_tool.rs` | **L0** | MCP 서버 없음 |
| T05 | Memory Tools | `tools/memory_tools.rs` | **L0** | C03 블로킹 |
| T06 | Program Tool | `tools/program_tool.rs` | **L0** | C03 블로킹 |
| T07 | Kernel Tools (7개) | `tools/kernel/` | **L0** | C03 블로킹 |
| T08 | A2A Tools | `tools/a2a_tools.rs` | **L0** | C03 블로킹 |
| T09 | Tool Registration (7단계) | `tools/registration.rs` | **L1** | 빌드시 15개 인덱싱 확인 |
| T10 | Host Tools | `host_tools.rs` | **L0** | 미테스트 |

### 3.5 메모리 시스템

| # | 기능 | 모듈 | 레벨 | 블로커 |
|---|------|------|------|--------|
| M01 | Vector Store | `memory/store.rs` | **L0** | C03 블로킹 |
| M02 | HNSW 인덱스 | `memory/hnsw.rs` | **L0** | 단위테스트만 |
| M03 | Hyperbolic 임베딩 | `memory/hyperbolic.rs` | **L0** | 단위테스트만 |
| M04 | Flash Attention | `memory/flash_attention.rs` | **L0** | 단위테스트만 |
| M05 | TF-IDF 임베딩 | `embedding.rs` | **L0** | 단위테스트만 |
| M06 | Reasoning Bank | `memory/reasoning_bank.rs` | **L0** | - |
| M07 | Sona 학습 엔진 | `memory/sona.rs` | **L0** | - |
| M08 | RVF Store | `memory/rvf_store.rs` | **L0** | - |
| M09 | Auto Memory Bridge | `memory/auto_memory_bridge.rs` | **L0** | C03 블로킹 |
| M10 | Graph | `memory/graph.rs` | **L0** | - |
| M11 | Chunking | `memory/chunking.rs` | **L0** | - |
| M12 | Normalizer | `memory/normalizer.rs` | **L0** | - |
| M13 | Memory Budget | `memory/budget.rs` | **L0** | - |

### 3.6 인프라

| # | 기능 | 모듈 | 레벨 | 블로커 |
|---|------|------|------|--------|
| I01 | Daemon (start/stop/status) | `daemon.rs`, `main.rs` | **L1** | - |
| I02 | Config (show/set) | `config.rs` | **L1** | set 미동작 |
| I03 | Audit Trail (해시체인) | `audit_trail.rs` | **L1** | CLI로 확인만 |
| I04 | Access Manager (RBAC) | `access_manager/` | **L0** | C03 블로킹 |
| I05 | Budget Manager | `budget.rs` | **L0** | C03 블로킹 |
| I06 | Resource Monitor | `resource_monitor.rs` | **L0** | - |
| I07 | State Store | `state_store.rs` | **L1** | 벤치마크에서 확인 |
| I08 | Git Layer | `git_layer.rs` | **L1** | 벤치마크에서 확인 |
| I09 | Cron Scheduler | `cron.rs` | **L0** | - |
| I10 | Backup/Restore | `backup.rs` | **L0** | - |
| I11 | WASM Sandbox | `wasm_sandbox.rs` | **L0** | wasmtime feature |
| I12 | Event Bus | `event_bus.rs` | **L0** | - |
| I13 | Circuit Breaker | `circuit_breaker.rs` | **L0** | - |
| I14 | Workers Pool | `workers/` | **L0** | - |
| I15 | Onboarding Wizard | `onboarding.rs` | **L0** | - |

### 3.7 채널

| # | 기능 | 모듈 | 레벨 | 블로커 |
|---|------|------|------|--------|
| CH01 | CLI 채널 | `channels/oxios-cli/` | **L1** | oxios run 동작 |
| CH02 | Web 대시보드 | `surface/oxios-web/` | **L0** | 빌드만 됨 |
| CH03 | Telegram 채널 | `channels/oxios-telegram/` | **L0** | 빌드만 됨 |
| CH04 | Gateway (메시지 허브) | `oxios-gateway/` | **L1** | 데몬에서 구동 확인 |

### 3.8 프로그램

| # | 기능 | 모듈 | 레벨 | 블로커 |
|---|------|------|------|--------|
| P01 | Program Manager | `program/mod.rs` | **L1** | install/list 동작 |
| P02 | Program Installer | `program/installer.rs` | **L1** | - |
| P03 | code-review 프로그램 | `.programs/code-review/` | **L0** | C03 블로킹 |
| P04 | debug 프로그램 | `.programs/debug/` | **L0** | C03 블로킹 |
| P05 | refactor 프로그램 | `.programs/refactor/` | **L0** | C03 블로킹 |
| P06 | deploy 프로그램 | `.programs/deploy/` | **L0** | C03 블로킹 |
| P07 | guardian 프로그램 | `.programs/guardian/` | **L0** | C03 블로킹 |
| P08 | program-creator 프로그램 | `.programs/program-creator/` | **L0** | C03 블로킹 |

### 3.9 보안

| # | 기능 | 모듈 | 레벨 | 블로커 |
|---|------|------|------|--------|
| S01 | Path Sandboxing | `access_manager/` | **L0** | C03 블로킹 |
| S02 | Shell 메타문자 차단 | `exec_tool.rs` | **L0** | C03 블로킹 |
| S03 | RBAC 권한 관리 | `access_manager/rbac.rs` | **L0** | C03 블로킹 |
| S04 | Auth Manager | `auth.rs` | **L0** | - |

### 3.10 요약

| 레벨 | 개수 | 비율 |
|------|------|------|
| **L0** (코드만 있음) | 53 | 77% |
| **L1** (기본 동작 확인) | 16 | 23% |
| **L2** (신뢰 가능) | 0 | 0% |

**핵심 블로커: C03 (Execute Phase)** — 이게 안 되면 A*, T*, M*, S* 전체가 테스트 불가.

---

## 4. 개선 루프 워크플로우

### 4.1 단일 사이클

```
┌─────────────────────────────────────────────────────┐
│                  ONE CYCLE                           │
│                                                     │
│  1. DIAGNOSE  ─── 현재 상태 파악                    │
│     ├── 로그 읽기 (tail ~/.oxios/logs/)             │
│     ├── CLI 실행 (oxios run --json "...")           │
│     └── 코드 읽기 (관련 모듈 소스)                   │
│                                                     │
│  2. FIX  ───────── 문제 수정                        │
│     ├── 근원 원인 식별                              │
│     ├── 최소한의 수정 (YAGNI)                       │
│     └── 코드에 주석으로 추적 번호 추가              │
│                                                     │
│  3. BUILD  ─────── 빌드 & 설치                      │
│     ├── cargo build --release                       │
│     ├── cp target/release/oxios ~/.cargo/bin/oxios  │
│     └── oxios stop && oxios start                   │
│                                                     │
│  4. VERIFY  ────── 수정 확인                        │
│     ├── 동일 CLI 명령 재실행                        │
│     ├── 이전에 실패한 시나리오 재테스트             │
│     └── 새로운 회귀 없는지 확인                     │
│                                                     │
│  5. COMMIT  ────── 체크포인트                       │
│     ├── git add -A && git commit                    │
│     ├── 이 문서의 상태 업데이트                     │
│     └── PROGRESS.md에 기록                          │
│                                                     │
│  6. NEXT  ───────── 다음 우선순위로                 │
│     └── 이 문서의 우선순위 큐에서 다음 항목 선택    │
└─────────────────────────────────────────────────────┘
```

### 4.2 우선순위 규칙

1. **블로커 우선.** 다른 기능을 테스트하지 못하게 막는 문제를 먼저 해결.
2. **코어 → 주변.** 프로토콜 → 에이전트 → 도구 → 인프라 순서.
3. **위에서 아래로.** CLI 경험을 먼저 고치고, 내부 API를 나중에.
4. **한 번에 하나.** 동시에 여러 버그 수정하지 않기.

---

## 5. 현재 우선순위 큐

### 🔴 Phase 1: 코어 프로토콜 (블로커 해제)

이것이 해결되어야 전체 기능의 77%가 테스트 가능해짐.

| 순서 | 작업 | 대상 | 상태 |
|------|------|------|------|
| **1-1** | 테스트 컴파일 에러 52개 수정 | `tests/` | 🔴 TODO |
| **1-2** | Execute Phase 진입 버그 수정 | `git_layer.rs` | ✅ **완료** |
| **1-3** | 테스트 컴파일 에러 52개 수정 | `tests/` | 🔴 TODO |
| **1-3** | Evaluate Phase 동작 확인 | `evaluation.rs` | 🔴 TODO |
| **1-4** | `oxios run --json` E2E 동작 확인 | CLI | 🔴 TODO |
| **1-5** | Multi-turn 세션 동작 확인 | `--session` 옵션 | 🔴 TODO |

### 🟡 Phase 2: 에이전트 & 도구

Execute가 되면 에이전트가 도구를 호출할 수 있게 됨.

| 순서 | 작업 | 대상 | 상태 |
|------|------|------|------|
| **2-1** | ExecTool (shell) 동작 확인 | `exec_tool.rs` | 🟡 대기 |
| **2-2** | ExecTool (structured) 동작 확인 | `exec_tool.rs` | 🟡 대기 |
| **2-3** | Program Tool 동작 확인 | `program_tool.rs` | 🟡 대기 |
| **2-4** | Memory Tools 동작 확인 | `memory_tools.rs` | 🟡 대기 |
| **2-5** | Kernel Tools 7개 동작 확인 | `tools/kernel/` | 🟡 대기 |
| **2-6** | Browser Tool 동작 확인 | `tools/browser/` | 🟡 대기 |
| **2-7** | MCP Tool 동작 확인 | `mcp_tool.rs` | 🟡 대기 |
| **2-8** | A2A Tools 동작 확인 | `a2a_tools.rs` | 🟡 대기 |

### 🟢 Phase 3: 인프라 & 채널

| 순서 | 작업 | 대상 | 상태 |
|------|------|------|------|
| **3-1** | Web 대시보드 동작 확인 | `oxios-web/` | 🟢 대기 |
| **3-2** | config set 동작 수정 | `config.rs` | 🟢 대기 |
| **3-3** | Cron Scheduler 동작 확인 | `cron.rs` | 🟢 대기 |
| **3-4** | Backup/Restore 동작 확인 | `backup.rs` | 🟢 대기 |
| **3-5** | Audit 체인 검증 CLI 동작 | `oxios audit` | 🟢 대기 |

### 🔵 Phase 4: 벤치마크 시나리오

코어가 다 되면 benchmarks/ 시나리오 15개를 pi-agent가 직접 실행.

| 순서 | 작업 | 대상 | 상태 |
|------|------|------|------|
| **4-1** | Tier 1 시나리오 5개 실행 | S01-S05 | 🔵 대기 |
| **4-2** | Tier 2 시나리오 5개 실행 | S06-S10 | 🔵 대기 |
| **4-3** | Tier 3 시나리오 5개 실행 | S11-S15 | 🔵 대기 |

---

## 6. 빌드 & 설치 프로시저

매 사이클마다 다음 명령을 실행:

```bash
# 1. 빌드
cd /Volumes/MERCURY/PROJECTS/oxios
cargo build --release 2>&1 | tail -5

# 2. 설치 (수동 복사)
cp target/release/oxios ~/.cargo/bin/oxios

# 3. 데몬 재시작
oxios stop 2>/dev/null
oxios start

# 4. 확인
oxios status
oxios doctor

# 5. 환경변수 (ZAI API Key)
# 매번 필요:
export ZAI_API_KEY="$(cat ~/.oxi/auth.json | python3 -c 'import sys,json; print(json.load(sys.stdin)["zai"]["access_token"])')"
```

### 주의사항
- `cargo install --path .` 대신 `cp` 사용 — 더 빠르고 확실
- 데몬 재시작 필수 — 안 하면 예전 바이너리 사용
- 환경변수 `ZAI_API_KEY`는 데몬 모드에서도 필요 (config.toml에 `engine.api_key` 설정하면 해결 가능)

---

## 7. 테스트 명령 치트시트

### 기본 E2E 테스트

```bash
# 인터뷰만 (빠른 응답, ~15초)
oxios run --json "안녕하세요"

# 코드 생성 (Interview → Seed → Execute 전체, ~60-120초)
oxios run --json "Rust로 Hello World 프로그램을 작성해줘"

# 컨텍스트 파일과 함께
echo 'fn fib(n: u32) -> u64 { if n <= 1 { return n as u64; } fib(n-1) + fib(n-2) }' > /tmp/test.rs
oxios run --json --context-file /tmp/test.rs "이 코드의 시간복잡도를 분석해줘"

# 멀티턴
SID=$(oxios run --json "할일 관리 구조체를 만들어줘" | jq -r '.session_id')
oxios run --json --session "$SID" "여기에 완료 토글을 추가해줘"

# 설정
oxios config show
oxios models

# 프로그램
oxios pkg list
oxios pkg install .programs/code-review
oxios program code-review
```

### 인프라 테스트

```bash
oxios status
oxios doctor
oxios audit
oxios budget
oxios agent list
```

### 로그 확인

```bash
tail -50 ~/.oxios/logs/oxios.log.2026-$(date +%m-%d)
# 또는
cat ~/.oxios/logs/oxios.log | tail -50
```

---

## 8. 세션 핸드오프 프로토콜

새 세션의 pi-agent가 이 문서를 읽고 바로 작업을 시작할 수 있도록:

### 8.1 이 문서가 있는 곳

```
docs/recursive-improvement-loop.md    # ← 이 파일
PROGRESS.md                           # ← 진행 로그
```

### 8.2 새 세션 시작 시 읽을 것

1. **이 문서** (`docs/recursive-improvement-loop.md`) — 전체 계획
2. **PROGRESS.md** — 마지막 작업 위치와 미해결 이슈
3. **AGENTS.md** — 코드베이스 컨벤션
4. **마지막 커밋 로그** — `git log --oneline -10`

### 8.3 세션 종료 시 할 것

1. **PROGRESS.md 업데이트** — 무엇을 했는지, 어디까지 했는지
2. **이 문서의 상태 업데이트** — 테스트 매트릭스 갱신
3. **커밋** — 작업 내용 커밋
4. **미해결 이슈 기록** — 다음 세션에서 이어서 할 수 있게

---

## 9. 알려진 버그 & 이슈 추적

### 현재 활성 이슈

| ID | 심각도 | 설명 | 파일 | 상태 |
|----|--------|------|------|------|
| BUG-001 | 🔴 Critical | Execute Phase 진입 안 함 | `git_layer.rs` | ✅ **수정됨** (deadlock) |
| BUG-002 | 🔴 Critical | 테스트 52개 컴파일 에러 | `tests/` | 미수정 |
| BUG-003 | 🟡 Medium | `oxios config set` 미동작 | `main.rs` | 미수정 |
| BUG-004 | 🟡 Medium | BrowserApi panic (runtime 중첩) | `browser_api.rs` | 워크어라운드 (feature off) |
| BUG-005 | 🟡 Medium | credential.rs가 auth.json 포맷 인식 못함 | `credential.rs` | 워크어라운드 (env var) |
| BUG-006 | 🟢 Low | `oxios log` 명령이 로그 내용 미출력 | `main.rs` | 미수정 |
| BUG-007 | 🟢 Low | clippy 경고 다수 | 전체 | 미수정 |

### 이슈 네이밍 규칙

```
BUG-NNN: 한줄 설명
파일: 어디를 고쳤는지
원인: 왜 발생했는지
해결: 어떻게 고쳤는지
커밋: abc1234
```

---

## 10. 성공 기준

### Phase 1 완료 기준
- [ ] `cargo test --workspace` 통과 (0 에러)
- [ ] `oxios run --json "Rust로 Hello World 작성해줘"` 가 코드를 반환 (evaluation_passed: true)
- [ ] `oxios run --json --session` 멀티턴 동작

### Phase 2 완료 기준
- [ ] 에이전트가 exec 도구로 파일 생성 가능
- [ ] 에이전트가 memory 도구로 정보 저장/검색 가능
- [ ] 에이전트가 program 도구로 프로그램 설치 가능

### Phase 3 완료 기준
- [ ] Web 대시보드에서 채팅 가능
- [ ] `oxios config set` 동작
- [ ] `oxios audit` 가 체인 무결성 검증

### Phase 4 완료 기준
- [ ] 벤치마크 시나리오 15개 중 12개 이상 통과
- [ ] 종합 등급 B 이상

---

## 11. 메타: 이 문서 자체의 관리

- 매 Phase 완료 시 이 문서 갱신
- 테스트 매트릭스는 CLI 테스트 결과에 따라 실시간 갱신
- 우선순위 큐는 블로커 해제 시마다 재정렬
- 세션 핸드오프 시 PROGRESS.md와 함께 반드시 업데이트
