# Oxios × OpenClaw 비교 분석 보고서

> 생성일: 2026-05-26  
> 목적: Agent OS 아키텍처 경쟁 제품 분석을 통한 Oxios의 위치 파악 및 시사점 도출

---

## 1. 개요

| | **Oxios** | **OpenClaw** |
|---|---|---|
| **GitHub** | `oxios/oxios` | `openclaw/openclaw` |
| **프로그래밍 언어** | Rust 2021 + TypeScript (frontend) | TypeScript (Node.js, Bun 호환) |
| **레이선스** | MIT | MIT |
| **코드 규모** | ~67K LOC (Rust) + TS 프론트엔드 | **~2.1M LOC** (TypeScript, 4,742개 소스 파일) |
| **플러그인/확장** | 채널 (web, cli, telegram), feature-gated | 136개 extensions + 58개 bundled skills |
| **AI 런타임** | oxi-sdk (crates.io), Ouroboros 프로토콜 | 내장 Anthropic/OpenAI streaming transport, plugin providers |
| **베타 버전** | 2026-05-26 | 2026.5.26 (날짜 기반 rolling) |

---

## 2. 핵심 철학 비교

### Oxios: "Rust-native Agent OS"

Oxios는 **Rust로 작성된 OS 수준 AI Agent 런타임**입니다.

> "An OS where AI agents execute real work on behalf of users — fork, exec, wait, kill, just like Unix processes."

- **Unix 철학**: 각 컴포넌트가 하나의 일만 함. 작고 composing 가능한 조각들.
- **Ouroboros First**: Interview → Seed → Execute → Evaluate → Evolve. 스펙 없이 실행 금지.
- **No Reimplementation**: oxi-sdk 재사용. 이미 있으면 다시 만들지 않음.
- **No Containers**: 직접 호스트 실행. AccessManager (RBAC + path sandboxing) 기반 보안.
- **Channel Agnostic**: Gateway가 메시지 출처를 알지 못함.

### OpenClaw: "AI that actually does things"

> "It runs on your devices, in your channels, with your rules."

- **TypeScript 우선**: "OpenClaw is primarily an orchestration system." 범용성 + 해킹 용이성.
- **Plugin-agnostic Core**: 코어는 플러그인에 의존하지 않음. 플러그인은 `openclaw/plugin-sdk/*`를 통해서만 침투.
- **Security as tradeoff**: "strong defaults without killing capability." 위험 경로를 명시적으로 만들고 운영자 통제.
- **Platform 우선**: macOS/iOS/Android 데스크톱 앱 + 멀티채널 메시징 (Slack, Discord, Telegram 등).
- **Docker sandboxing**: Exec tool 실행 시 Docker 컨테이너 기반 격리.
- **Skills as plugins**: bundled skill을 최소화하고 ClawHub 마켓플레이스로 배포.

---

## 3. 아키텍처 비교

### 3.1 전체 구조

```
Oxios (Rust)
├── oxios (main binary)
│   ├── Kernel (supervisor + scheduler + ouroboros + agent_runtime)
│   └── src/ (kernel assembler)
├── crates/
│   ├── oxios-kernel/         ← 핵심: supervisor, scheduler, memory, tools
│   ├── oxios-markdown/        ← Knowledge base (markdown VFS)
│   ├── oxios-ouroboros/       ← Ouroboros 프로토콜
│   ├── oxios-gateway/         ← 채널 agnostic message hub
│   └── oxios-mcp/             ← MCP client (JSON-RPC 2.0 over stdio)
├── channels/ (feature-gated)
│   ├── oxios-web/             ← Axum backend + React frontend
│   ├── oxios-cli/             ← CLI channel
│   └── oxios-telegram/        ← Telegram channel
└── share/                     ← 기본 skills, config
```

```
OpenClaw (TypeScript/Node.js)
├── src/
│   ├── agents/                ← Agent runtime, spawn, subagent registry
│   ├── gateway/               ← WebSocket/HTTP gateway, sessions, auth
│   ├── channels/              ← Discord, Slack, Telegram, iMessage 등
│   ├── context-engine/        ← Tool execution, planner, protocol
│   ├── memory/                ← Root memory files (session memory)
│   ├── security/              ← Audit, RBAC, exec approval, path policy
│   ├── infra/                 ← Docker, binaries, install, restart
│   ├── tools/                 ← Tool registry, availability
│   ├── flows/                 ← Approval flows (system-run approval)
│   ├── daemon/                ← launchd/systemd service management
│   ├── config/                ← Schema validation, defaults
│   ├── sessions/              ← Session lifecycle, transcript
│   └── mcp/                   ← MCP client + server
├── packages/
│   ├── sdk/                   ← Public SDK
│   ├── plugin-sdk/            ← Plugin development SDK
│   └── plugin-package-contract/
├── extensions/                ← 136개 extension (provider, channel 등)
├── skills/                    ← 58개 bundled skill
├── apps/
│   ├── macos/                 ← macOS 네이티브 앱 (Swift)
│   ├── ios/                   ← iOS 네이티브 앱
│   └── android/               ← Android 앱
└── ui/                        ← Web dashboard
```

### 3.2 핵심 컴포넌트 상세 비교

#### Agent Lifecycle

| 관점 | Oxios | OpenClaw |
|---|---|---|
| **Agent 생성** | `AgentLifecycleManager` (fork → A2A register → schedule → run → cleanup) | `agents/acp-spawn.ts`, `sessions-spawn` lifecycle hooks |
| **프로세스 관리** | `Supervisor` (fork/exec/wait/kill) — Unix pid 수준 | `bash-process-registry.ts` (bash process tracking) |
| **다중 Agent** | `AgentGroup` (Seed-split), `Ouroboros` orchestration | `subagent-registry.ts`, nested spawning with depth limits |
| **Tool calling** | `AgentRuntime` wraps oxi-agent tool-calling loop | `pi-embedded-subscribe.ts`, `tool-catalog.ts`, `pi-tools.ts` |

**분석**: Oxios는 OS 수준의 프로세스 관리 (fork/exec/kill) → 실제 Unix 프로세스로 Agent 실행. OpenClaw는 bash process + subagent registry로 Agent를 관리 → Node.js 프로세스 내에서 도구 호출로 추상화. **본질적 차이**: Oxios는 OS 프로세스 모델, OpenClaw는 EventEmitter/async 패턴 기반 런타임 추상화.

#### Scheduler & Rate Limiting

| 관점 | Oxios | OpenClaw |
|---|---|---|
| **스케줄러** | `Scheduler` (priority-based, AIOS/AgentRM-inspired) | `model-fallback.ts`, `provider-selection-runtime.ts` |
| **Rate limiting** | Rate-limit-aware admission, zombie detection | `fixed-window-rate-limit.ts`, `exec-approvals.ts` |
| **Concurrency** | `max concurrent` enforcement | `concurrency-runtime.ts` |

#### Memory Architecture

| 관점 | Oxios | OpenClaw |
|---|---|---|
| **Session Memory** | `MemoryManager` (Hot/Warm/Cold tiers, Dream consolidation, HNSW, hyperbolic embeddings) | `memory/root-memory-files.ts`, `memory-core.ts` |
| **Knowledge Base** | `oxios-markdown` crate — markdown VFS, backlink index, link graph | `memory-host-markdown.ts` — markdown notes backed by VFS |
| **Vector Search** | HNSW via `oxios-markdown` | `memory-host-search.ts` (runtime search) |
| **Compaction** | Compaction tree (Raw→Daily→Weekly→Monthly→Root) | `compaction.ts` (token budget management) |

**분석**: Oxios의 Memory는 RFC-008에 정의된 세대적 메모리 계층화 (Hot/Warm/Cold)를 갖추고 있으며 Dream Consolidation으로 자동 통합. OpenClaw는 markdown 기반 파일 메모리 + search layer로 상대적으로 단순. **Oxios 우위**: 세분화된 메모리 계층과 자동 보호/임시 기억 분류.

#### Security Model

| 관점 | Oxios | OpenClaw |
|---|---|---|
| **핵심** | `AccessManager` (OWASP-inspired RBAC + path sandboxing) | `audit.ts` (deep probe), `exec-approvals.ts`, `dangerous-config-flags.ts` |
| **Sandboxing** | Path sandboxing (allowlist) + RBAC | **Docker container** (`docker-setup.e2e.test.ts`, sandbox path) |
| **Audit** | `AuditTrail` (Merkle-chain style tamper-evident) | `audit-channel.ts`, `audit-deep-probe-findings.ts` |
| **Path Policy** | `path-policy.ts` (kernel_handle) | `scan-paths.ts`, `exec-filesystem-policy.ts` |
| **Exec Safety** | Structured mode (binary allowlist + metachar blocking) | `exec-safe-bin-policy.ts`, `exec-approval-runtime.ts` |
| **Credential** | `CredentialStore` (env → config.toml → oxi auth.json) | `credentials.ts`, `auth-profiles.ts` |

**분석**: OpenClaw의 Docker 기반 sandboxing은 더 강한 격리를 제공하지만 "No containers" 철학의 Oxios는 호스트 직접 실행. 이는 성능과 투명성 우위, 하지만 보안 경계는 더 신중해야 함. Oxios의 AuditTrail (cryptographic chaining)은 OpenClaw의 audit보다 tamper-evidence 측면에서 우수.

#### Communication & Channels

| 관점 | Oxios | OpenClaw |
|---|---|---|
| **Gateway** | `oxios-gateway` — channel-agnostic message hub | `gateway/server.ts` — WebSocket/HTTP server |
| **Channels** | Web (Axum+React), CLI, Telegram (feature-gated) | Discord, Slack, Telegram, iMessage, WhatsApp, GoogleChat, Matrix, Feishu 등 **36개** |
| **A2A Protocol** | A2A (`a2a.rs`) — Google's agent-to-agent | `acp-runtime.ts` — OpenClaw의 자체 A2A 프로토콜 |
| **Native Apps** | 없음 | macOS (Swift/SwiftUI), iOS, Android, Windows (companion apps) |
| **Pairing** | 없음 | `node-pairing.ts` — device pairing across platforms |

**분석**: OpenClaw의 채널 지원이 압도적 (36개 이상). 이는 VISION.md의 "Supporting all major messaging channels" 우선순위 반영. Oxios는 3개 채널만 있지만 철학적으로 "_CHANNEL agnostic" — Gateway가 채널을 모름. Pairing/companion 앱 없음.

#### Protocol & Orchestration

| 관점 | Oxios | OpenClaw |
|---|---|---|
| **프로토콜** | **Ouroboros** (spec-first: interview→seed→execute→evaluate→evolve) | **ACP** (Agent Communication Protocol) + embedded Pi runner |
| **Execution Model** | Spec-first. 절대 스펙 없이 실행 불가. | Tool execution loop. Spec/freeze 없음 — agent가 바로 실행. |
| **Embedding** | `OxiosEngine` wraps `oxi_sdk::Oxi` | `pi-embedded-subscribe.ts`, `pi-embedded-runner.ts` |
| **Code execution** | WASM sandbox (`wasm_sandbox.rs`) + shell exec | Docker container + bash process |

**분석**: 가장 근본적 차이. Oxios의 Ouroboros는 **spec-first execution**로, 신뢰할 수 있는 AI 실행의 방법론적 기반. OpenClaw는 더 즉각적 실행에 초점 — 빠른 결과, 하지만 spec/test 없이는 추론 검증 없음.

#### Knowledge & Markdown

| 관점 | Oxios | OpenClaw |
|---|---|---|
| **Knowledge system** | `oxios-markdown` — VFS + backlink index + link graph | `memory-host-markdown.ts` — markdown file storage |
| **UI** | `/knowledge/` full-screen React app (HyperMD) | 없음 (session transcript 위주) |
| **Backlinks** | `BacklinkIndex` built-in | 파일 간 링크 추적 없음 |
| **Graph** | SVG link graph visualization | 없음 |

**분석**: Oxios의 knowledge UI는 Obsidian/files.md 스타일의arkdown.note-taking 환경. OpenClaw는 이를 session memory로 취급. **상호 보완적**: OpenClaw는 채널 메시징 중심, Oxios는 persistent knowledge 중심.

#### Skill System

| 관점 | Oxios | OpenClaw |
|---|---|---|
| **Unified model** | SKILL.md unified (RFC-009) — `SkillManager` 단일체 | skill + plugin 분리. skills는 ClawHub 배포. |
| **Definition** | SKILL.md with YAML frontmatter (4D requirements, install specs) | 별도 skill 디렉토리 + `skills-runtime.ts` |
| **Distribution** | Workspace-level + global user + bundled | ClawHub marketplace (clawhub.ai) |
| **Count** | share/default-skills/ | 58 bundled + 136 extensions |

**분석**: Oxios의 unified skill model (RFC-009)은 programs/skills 통합으로 단순화. OpenClaw는 ClawHub를 통한 decentralized distribution에 집중. 이는 생태계 전략의 차이.

#### MCP Integration

| 관점 | Oxios | OpenClaw |
|---|---|---|
| **Client** | `oxios-mcp` (crates.io) + `crates/oxios-mcp` integration | `mcp-stdio-transport.ts`, `mcp-http.ts` |
| **Server** | 없음 | MCP over HTTP (`mcp-http.ts`) |
| **Tool bridging** | `pi-bundle-mcp-runtime.ts` | `pi-bundle-mcp-tools.ts` |

**분석**: 두 시스템 모두 MCP를 지원. OpenClaw는 MCP를 server로도 노출하는双向 integration.

---

## 4. 테스트 인프라 비교

| 관점 | Oxios | OpenClaw |
|---|---|---|
| **단위 테스트** | `#[cfg(test)] mod tests`, Vitest | Vitest (TS) — 3,474개 테스트 파일 |
| **E2E** | `tests/` per crate | `*.e2e.test.ts` pattern |
| **Live 테스트** | `OPENCLAW_LIVE_TEST=1` | `$openclaw-testing` env flag |
| **Remote CI** | GitHub Actions | **Crabbox** (custom remote test runner) |
| **Coverage** | `cargo tarpaulin` | Vitest coverage + Crabbox |
| **Format** | `cargo fmt`, `oxfmt` | `oxfmt` (OpenClaw formatter) |
| **Lint** | `cargo clippy` | `oxlint` |

OpenClaw의 Crabbox는 CI에서 Docker 기반 live test를 실행하는 자체 인프라. Oxios는 표준 GitHub Actions.

---

## 5. 코드 품질 & 엔지니어링

| 관점 | Oxios | OpenClaw |
|---|---|---|
| **타입 시스템** | Rust (strong static), TypeScript (strict mode) | TypeScript ESM, strict, `satisfies`, no `@ts-nocheck` |
| **순환 의존** | `cargo check --all-targets` | `pnpm check:import-cycles` + madge |
| **파일당 LOC** | Rust crate 기준 | ~700 LOC guideline (split bigger files) |
| **API 스타일** | `verb_noun` (e.g., `fork_agent`) | TypeScript conventions, discriminated unions |
| **Documentation** | RFC documents, AGENTS.md per subtree | AGENTS.md per subtree, public docs site |

---

## 6. 강점/약점 비교 (SWOT-ish)

### Oxios 강점

1. **Rust 성능**: Zero-cost abstraction, 메모리 안전성, 병렬성. Agent 실행 오버헤드 최소화.
2. **Spec-first execution (Ouroboros)**: Interview → Seed → Execute → Evaluate → Evolve. 실행 전에 검증 가능한 스펙 존재.
3. **OS 프로세스 모델**: Agent = Unix process. fork/exec/wait/kill. 관측 가능성 최고.
4. **세분화된 메모리 시스템**: Hot/Warm/Cold tier, Dream consolidation, HNSW, hyperbolic embeddings.
5. **Tamper-evident audit**: Merkle-chain style cryptographic audit trail.
6. **Knowledge UI**: Obsidian 스타일 markdown 편집기 + backlink + graph.
7. **Clean separation**: 두 knowledge 시스템 (session memory vs markdown knowledge) 명시적 분리.

### Oxios 약점

1. **채널 지원 부족**: Web/CLI/Telegram 3개만. OpenClaw의 36개 채널 대비 극히 제한적.
2. **Companion app 부재**: macOS/iOS/Android 네이티브 앱 없음.
3. **Rust 학습 곡선**: TypeScript 생태계 대비 확장성 낮음.
4. **Plugin 생태계**: OpenClaw의 136개 extensions 대비 거의 없음.
5. **Marketplace**: ClawHub 같은 마켓플레이스 없음.
6. **Docker sandboxing 없음**: 호스트 직접 실행. 보안 검증 필요.

### OpenClaw 강점

1. **방대한 채널 지원**: 36개 이상 메시징 채널.
2. **풍부한 생태계**: 136개 extensions, 58개 skills, companion apps.
3. **TypeScript 유연성**: 빠른 iteration, 광범위한 npm 생태계 활용.
4. **ClawHub marketplace**: decentralized skill/plugin 배포.
5. **Docker sandboxing**: 강력한 실행 격리.
6. **Device pairing**: cross-platform device management.
7. **ACP protocol**: 자체 A2A 프로토콜로 multi-agent coordination.

### OpenClaw 약점

1. **성능**: Node.js runtime. Rust 대비 오버헤드 존재.
2. **Spec-first 부재**: 실행 전 검증 가능한 spec 없이 agent 실행.
3. **단순한 메모리**: markdown 파일 기반. tiered memory 없음.
4. **Kubernetes 없음**: Docker는 사용하지만 Kubernetes orchestration 없음.
5. **Config migration 복잡성**: VISION.md에서 명시한 "no long-lived aliases" 정책이 마이그레이션 부담.
6. **Plugin-agnostic 강조**: 코어가 lean해지지만 plugin 의존성이 늘어날 수 있음.

---

## 7. 시사점 및 권고

### 7.1 Oxios가 채택할 수 있는 것

1. **채널 확장**: OpenClaw의 채널 아키텍처 (`channels/`)를 참고하여 더 많은 채널을 지원. Gateway의 channel-agnostic 특성을 유지하면서.
2. **Docker sandboxing 옵션**: "No containers" 철학을 깨지 않으면서, `WasmSandbox` alongside에 Docker 기반 sandbox 옵션 추가 검토.
3. **Plugin SDK**: OpenClaw의 plugin-sdk 구조를 참고하여 확장성 메커니즘 설계.
4. **Marketplace 준비**: ClawHub 패턴을 참고하여 skill/plugin 마켓플레이스 구조 검토.
5. **Companion apps**: 장기 로드맵에 macOS/iOS/Android companion app 추가.

### 7.2 Oxios가 차별화해야 할 것

1. **Ouroboros spec-first**: OpenClaw가 따라올 수 없는 방법론적 차별점. Ouroboros의 가치를 더 명확하게 마케팅.
2. **세분화된 메모리**: OpenClaw의 session memory보다 월등히 우수한 tiered memory system을 문서화하고 시연.
3. **Rust 성능**: Agent OS로서 OS 수준의 관측 가능성과 제어를 Edge로 활용.
4. **Knowledge UI**: Obsidian 스타일 markdown 환경은 OpenClaw가 아직 제공하지 않는 영역.

### 7.3 학습 포인트

| OpenClaw에서 배울 점 | 구현 고려사항 |
|---|---|
| Crabbox (remote test runner) | E2E test infrastructure for Rust |
| exec-approval UX flow | Interactive approval UX |
| ClawHub marketplace pattern | Decentralized skill distribution |
| multi-channel routing | Channel abstraction patterns |
| device pairing | Platform pairing UX research |

---

## 8. 결론

Oxios와 OpenClaw는 모두 "Agent OS"라는 범주에 속하지만, **근본적으로 다른 아키텍처 철학**을 가지고 있습니다.

- **OpenClaw**는 **TypeScript 생태계 + 멀티채널 메시징 + 방대한 plugin 생태계**에 집중한 pragmatic한 제품입니다. 136개 extensions, 36개 이상 채널, companion apps, ClawHub marketplace까지 갖춘 성숙한 플랫폼입니다. "AI that actually does things" — 채널에서 실제로 동작하는 AI.

- **Oxios**는 **Rust-native OS 수준 Agent 런타임**으로, spec-first execution (Ouroboros), tiered memory, OS 프로세스 모델, tamper-evident audit이라는 **방법론적 차별점**을 가집니다. "fork, exec, wait, kill, just like Unix processes" — AI agent를 Unix citizen으로 만드는 것.

두 시스템은 상호 배타적이지 않습니다. OpenClaw의 채널/플랫폼 인프라 위에 Oxios의 Ouroboros execution engine을 배치하거나, Oxios의 knowledge UI에 OpenClaw의 채널 연결을 붙이는 등의 통합이 가능합니다.

**핵심 교훈**: Oxios의 가장 강력한 차별점은 Ouroboros spec-first execution입니다. 이것을 더 깊이 구현하고 문서화하는 것이 OpenClaw와의 경쟁에서 가장 효과적인 전략입니다.
