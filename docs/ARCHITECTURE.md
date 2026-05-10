# Oxios Architecture

> **핵심 구조 문서.** 이 프로젝트의 모든 개발은 이 구조를 따른다.

---

## Layer Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                         Terminal                             │
│              Web │ CLI │ Telegram │ Slack                     │
│                                                              │
│  사용자가 시스템에 접속하는 지점                                │
│  "터미널에 접속하다" → "웹 터미널을 열다"                      │
└──────────────────────────┬──────────────────────────────────┘
                           │ 사용자 요청
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                      Application                             │
│          code-review │ deploy │ monitor │ git-sync            │
│                                                              │
│  Kernel System Call을 조합한 완성된 워크플로우                  │
│  동적 로딩: program.toml + SKILL.md로 설치                     │
│                                                              │
│  각 Application은 System Call만 사용                          │
│  Kernel 내부 구조는 알 수 없음                                 │
└──────────────────────────┬──────────────────────────────────┘
                           │ kernel.save(), kernel.spawn(), ...
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                        Kernel                                │
│                                                              │
│  ┌─ System Call Interface (pub fn) ──────────────────────┐  │
│  │                                                        │  │
│  │  save()  load()  delete()     상태 관리                 │  │
│  │  spawn()  wait()  kill()      에이전트 수명주기          │  │
│  │  remember()  recall()         메모리                    │  │
│  │  commit()  tag()  restore()   버전 관리                 │  │
│  │  schedule()  unschedule()     스케줄링                  │  │
│  │  audit()  verify()            감사                      │  │
│  │  exec()                        실행                     │  │
│  │  subscribe()                   이벤트                    │  │
│  │  resources()  check_budget()   자원                     │  │
│  │                                                        │  │
│  │  Application, Daemon, Terminal이 호출하는 유일한 인터페이스 │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌─ Subsystems (pub(crate)) ─────────────────────────────┐  │
│  │                                                        │  │
│  │  Supervisor    에이전트 수명주기 관리 (init)             │  │
│  │  Scheduler     작업 스케줄링                            │  │
│  │  StateStore    영속 상태 저장 (vfs)                     │  │
│  │  GitLayer      버전 관리 (gix)                         │  │
│  │  AuditTrail    감사 로그 (blake3 hash-chain)           │  │
│  │  BudgetManager 에이전트 자원 제한 (cgroup)              │  │
│  │  WasmSandbox   샌드박스 격리 (seccomp)                 │  │
│  │  EventBus      이벤트 브로드캐스트 (mqueue)             │  │
│  │  CronScheduler 시간 기반 스케줄러 (cron)               │  │
│  │  ResourceMon   시스템 리소스 모니터 (procfs)            │  │
│  │  ExecTool      호스트 명령 실행 (직접 실행)             │  │
│  │  McpBridge     외부 프로토콜 연결 (net)                 │  │
│  │  A2A           에이전트 간 통신 (signal)                │  │
│  │  Orchestrator  Ouroboros 실행 엔진                     │  │
│  │  MemoryManager 에이전트 메모리                          │  │
│  │  ProgramManager 프로그램 관리                           │  │
│  │  AuthManager   인증                                    │  │
│  │  AccessManager 접근 제어 (RBAC)                        │  │
│  │                                                        │  │
│  │  외부에서 직접 접근 불가                                 │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
└──────────────────────────┬──────────────────────────────────┘
                           │ kernel.spawn() → Agent 생성
                           │ kernel.exec()  → ExecTool 실행
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                        Runtime                               │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                    Engine (oxi)                       │   │
│  │                                                      │   │
│  │  oxi-ai:    LLM provider, streaming, context         │   │
│  │  oxi-agent: agent loop, tool registry, compaction    │   │
│  │                                                      │   │
│  │  Agent가 "생각하고 도구를 쓰는" 핵심 엔진               │   │
│  │  Kernel과 Runtime 양쪽에서 사용 (수평 종속성)          │   │
│  └──────────────────────────┬───────────────────────────┘   │
│                              │                               │
│       ┌──────────────────────┼──────────────────────┐       │
│       ▼                      ▼                      ▼       │
│  ┌──────────┐         ┌──────────┐          ┌──────────┐    │
│  │  Agent   │         │ Workspace │          │   Host   │    │
│  │          │         │          │          │          │    │
│  │  (oxi)   │         │   bash    │          │ macOS    │    │
│  │  LLM 추론 │         │  직접실행  │          │ git/gh   │    │
│  │  Tool 호출│         │ 빌드/테스트│          │ osascript│    │
│  └──────────┘         └──────────┘          └──────────┘    │
│                                                              │
│  Workspace: 디렉토리 기반 샌드박스                             │
│  Host: ExecTool (allowlist + metachar blocking)              │
└─────────────────────────────────────────────────────────────┘
```

---

## Layer Summary

```
Layer         Implementation                  Role
────────────  ──────────────────────────────  ──────────────
Terminal      oxios-web, oxios-cli            접속 (사용자 인터페이스)
Application   ProgramManager, programs        조합 (Kernel System Call 워크플로우)
Kernel        oxios-kernel                    관리 (상태, 스케줄, 감사, 버전관리)
Runtime       Agent + Workspace + Host        실행 (실제 작업)
Engine (oxi)  oxi-ai + oxi-agent              Kernel↔Runtime 양콕 핵심 (수평 종속성)
```

---

## Unix Philosophy Mapping

```
Unix 원칙                    Oxios 적용
─────────────────────────── ──────────────────────────────
모든 것은 파일이다            모든 것은 StateStore에 버전 관리된다
작은 프로그램이 큰 일을 한다   작은 Tool이 Agent에 의해 조합된다
프로그램은 텍스트 스트림 처리  Agent는 메시지 스트림 처리
한 가지를 잘 하라             각 Subsystem은 한 가지 책임
커널은 인터페이스만 제공       Kernel은 System Call만 노출
```

---

## Dependency Rule

```
Terminal    →  Application  →  Kernel  →  Runtime
                                        ←  Engine (oxi)

각 레이어는 바로 아래 레이어만 의존한다.
위에서 아래로만 의존한다. 역방향 의존 없음.
Engine은 수평 종속성 — Kernel과 Runtime 양쪽에서 사용.
```

---

## Crates to Layers

```
oxios-web/         →  Terminal
oxios-cli/         →  Terminal
oxios-gateway/     →  Kernel (Subsystem: 메시지 라우팅)
oxios-kernel/      →  Kernel (All subsystems + System Call)
oxios-ouroboros/   →  Kernel (Subsystem: spec-first execution)
oxios/ (binary)    →  Kernel Assembly (KernelBuilder)
../oxi/oxi-ai/     →  Engine
../oxi/oxi-agent/  →  Engine
```

---

## Key Principle

> **Kernel은 System Call만 노출한다.**
> **Application은 System Call만 사용한다.**
> **Kernel 내부(Subsystems)는 외부에서 보이지 않는다.**
> **Engine(oxi)은 Kernel과 Runtime 양쪽에서 사용되는 핵심 라이브러리다.**

---

## Current Status

| Component | Layer | Status |
|-----------|-------|--------|
| oxios-web | Terminal | ✅ REST API + Dashboard |
| oxios-cli | Terminal | ✅ Basic CLI |
| ProgramManager | Application | ⚠️ Tool provider only, no System Call composition |
| Kernel System Call | Kernel | ⚠️ 6 methods, need ~15 more |
| Kernel Subsystems | Kernel | ✅ 20 subsystems implemented |
| AgentRuntime | Runtime | ✅ oxi-agent wrapper |
| ExecTool | Runtime | ✅ Direct host execution via tokio::process::Command |
| Workspace Sandbox | Runtime | ✅ Directory-based sandbox via AccessManager |
| Engine (oxi) | Engine | ✅ oxi-ai + oxi-agent |