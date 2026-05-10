# Oxios — 사용자 가이드

> AI 에이전트 운영 체제. Unix 철학 + Ouroboros 방법론.

---

## 소개

Oxios는 **24/7 데몬**으로 동작하는 AI 에이전트 운영 체제입니다.

```
사용자 (Web/CLI) → Gateway → Kernel → 에이전트 실행 → 응답
                    ↑
              Cron 스케줄 (시간 기반 자동 실행)
```

**핵심 개념:**
- **Ouroboros 프로토콜**: interview → seed → execute → evaluate → evolve
- **ExecTool**: 워크스페이스 명령 직접 실행 (허용 목록 + 메타문자 차단)
- **Workspace 샌드박스**: 디렉토리 기반 격리, RBAC + 감사 로깅

---

## 설치 및 실행

### 1. 빌드

```bash
# 방법 1: cargo install (crate가 publish되면)
cargo install oxios

# 방법 2: source에서 빌드
git clone https://github.com/your-repo/oxios
cd oxios
cargo build --release
```

### 2. 환경 변수 설정

```bash
export ANTHROPIC_API_KEY=sk-ant-...
# 또는
export OPENAI_API_KEY=sk-...
```

### 3. 데몬 실행

```bash
oxios
# → http://127.0.0.1:4200 에서 Web UI 열림
# → 백그라운드에서 24/7 동작
```

### 4. 데몬 관리

```bash
oxios daemon status    # 상태 확인
oxios daemon restart    # 재시작
```

---

## Web UI 사용

브라우저에서 `http://127.0.0.1:4200` 열기.

```
┌─────────────────────────────────────────────────────────┐
│  🌿 Oxios                              [Persona ▼]       │
├──────────┬──────────────────────────────────────────────┤
│ 💬 Chat  │                                              │
│ 👥 Agents│  ┌──────────────────────────────────────┐   │
│ 📅 Cron  │  │ Welcome to Oxios. 무엇을 도와드릴까요?│   │
│ 📁 Programs│ │                                      │   │
│ 🎯 Memory │  └──────────────────────────────────────┘   │
│ ⚙️ Config │                                              │
└──────────┴──────────────────────────────────────────────┘
```

**사용법:**
1. Chat 패널에서 메시지 입력
2. 에이전트가 Ouroboros 프로토콜로 작업
3. 결과를 확인

---

## API 사용

### Chat

```bash
curl -X POST http://127.0.0.1:4200/api/chat \
  -H "Content-Type: application/json" \
  -d '{"content": "帮我写一个Rust HTTP服务器", "user_id": "user1"}'
```

### 에이전트 관리

```bash
# 실행 중인 에이전트 목록
curl http://127.0.0.1:4200/api/agents

# 에이전트 종료
curl -X POST http://127.0.0.1:4200/api/agents/<id>/kill
```

### 상태 확인

```bash
curl http://127.0.0.1:4200/api/status
```

---

## CLI 사용

### 대화형 모드

```bash
oxios chat
```

### 단일 프롬프트 실행

```bash
oxios run "帮我写一个TODO应用"
```

### 설정 확인

```bash
oxios config show
oxios config get exec.allowed_commands
```

---

## Cron Jobs (자동 실행)

매일 아침 뉴스를 요약하거나, 주기적으로 백업을 수행하는 등의 작업을 등록할 수 있습니다.

### 설정 파일로 등록

`~/.oxios/config.toml`:

```toml
[cron]
enabled = true
tick_interval_secs = 60

[cron.jobs.morning_news]
schedule = "0 9 * * *"
goal = "오늘的新闻摘要"
priority = "low"
```

### API로 등록

```bash
curl -X POST http://127.0.0.1:4200/api/cron-jobs \
  -H "Content-Type: application/json" \
  -d '{
    "name": "news-summary",
    "schedule": "0 9 * * *",
    "goal": "오늘的新闻摘要"
  }'
```

---

## Program (에이전트 앱)

Program은 에이전트가 사용할 수 있는 설치 가능한 앱입니다.

### Program 설치

```bash
oxios program install ./my-program
```

### Program 목록

```bash
oxios program list
```

### Program 활성화/비활성화

```bash
oxios program enable my-program
oxios program disable my-program
```

### Program 구조

```
my-program/
├── program.toml     # 메타데이터
├── SKILL.md        # 에이전트 지침
├── bin/            # 실행 파일 (선택)
└── config/         # 설정 (선택)
```

---

## 설정

설정 파일: `~/.oxios/config.toml`

### 전체 설정 예시

```toml
[kernel]
workspace = "~/.oxios/workspace"
max_agents = 10

[gateway]
host = "127.0.0.1"
port = 4200

[exec]
# 허용된 호스트 명령 (빈 배열 = 모두 허용, 개발 모드)
allowed_commands = ["git", "gh", "open", "osascript"]
# 명령 기본 타임아웃 (초)
default_timeout_secs = 120
# 명령 최대 타임아웃 (초)
max_timeout_secs = 600
# 필수 호스트 도구
required_host_tools = ["git"]
# 선택적 호스트 도구
optional_host_tools = ["gh", "osascript", "shortcuts", "remindctl"]

[scheduler]
max_concurrent = 5
rate_limit_per_minute = 60
zombie_timeout_secs = 300

[security]
auth_enabled = false
max_execution_time_secs = 300
```

---

## 도구 (Tools)

에이전트가 사용할 수 있는 도구들:

| 도구 | 설명 |
|------|------|
| `exec` (shell) | 워크스페이스에서 bash 명령 실행 |
| `exec` (structured) | 허용 목록 기반 호스트 명령 실행 |
| `read` | 파일 읽기 |
| `write` | 파일 쓰기 |
| `edit` | 파일 편집 |
| `grep` | 텍스트 검색 |
| `find` | 파일 검색 |
| `ls` | 디렉토리 목록 |

---

## 메모리

에이전트는 세션 간 기억할 수 있습니다:

```bash
# 메모리 저장
curl -X PUT http://127.0.0.1:4200/api/memory/my-note \
  -d '{"category": "notes", "content": "중요한 내용"}'

# 메모리 검색
curl http://127.0.0.1:4200/api/memory
```

---

## 감사 로그

모든 도구 사용이 감사됩니다:

```bash
# 감사 로그 확인
curl http://127.0.0.1:4200/api/audit
```

응답 형식:
```json
[
  {
    "timestamp": "2026-05-10T18:00:00Z",
    "agent_name": "agent-123",
    "action": "exec",
    "resource": "bash -c 'git status'",
    "allowed": true,
    "reason": null
  }
]
```

---

## 문제 해결

### 데몬이 시작되지 않음

```bash
# 로그 확인
RUST_LOG=debug oxios 2>&1

# 포트 충돌 확인
lsof -i :4200
```

### 에이전트가 멈춤

```bash
# 실행 중인 에이전트 확인
oxios agent list

# 강제 종료
oxios agent kill <id>
```

### 설정 오류

```bash
# 설정 검증
oxios config show

# 기본 설정으로 복원
rm ~/.oxios/config.toml
oxios  # 자동 재생성
```

---

## 환경 변수

| 변수 | 설명 |
|------|------|
| `ANTHROPIC_API_KEY` | Anthropic (Claude) API 키 |
| `OPENAI_API_KEY` | OpenAI API 키 |
| `OXIOS_API_KEY` | Oxios API 키 (선택) |
| `RUST_LOG` | 로깅 레벨 (`info`, `debug`) |

---

## 파일 구조

```
~/.oxios/
├── config.toml              # 설정
├── workspace/               # 작업 디렉토리
│   ├── memory/             # 에이전트 메모리
│   │   ├── knowledge/     # 지식 베이스
│   │   └── conversations/ # 세션 기록
│   ├── sessions/          # 세션
│   ├── seeds/             # Ouroboros 시드
│   ├── skills/            # 스킬 템플릿
│   └── programs/         # 설치된 프로그램
└── api-keys.json         # API 키 (production)
```

---

## 참고

- **추가 문서:**
  - `README.md` — 설치 및 개발 가이드
  - `DESIGN.md` — 아키텍처 및 설계 결정
  - `AGENTS.md` — AI 에이전트 개발 규칙

- **프로토콜:** Ouroboros (spec-first)
  - interview: 요구사항 분석
  - seed: 실행 계획 생성
  - execute: 에이전트 작업 실행
  - evaluate: 결과 평가
  - evolve: 개선

- **보안 모델:**
  - RBAC (역할 기반 접근 제어)
  - Workspace 샌드박스 (디렉토리 제한)
  - 감사 로깅 (모든 작업 기록)
  - 허용 목록 (허용된 명령만 실행)