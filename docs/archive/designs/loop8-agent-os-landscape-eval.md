# Loop 8: Agent OS Landscape — Competitive Evaluation

> **버전:** v0.3.0-alpha  
> **작성일:** 2026-05-07  
> **목표:** Oxios를 Agent OS/Framework 생태계에서 평가하고 포지셔닝 명확화

---

## 1. 생태계 개요

AI Agent 시스템은 2025년 기준 3개의 층으로 나뉩니다:

```
┌─────────────────────────────────────────────────────────┐
│                    Agent OS Layer                        │
│  (agents as processes, tools as resources, OS-level)  │
│   Oxios, miniclaw-os, OpenClaw, OS-Level Claude         │
├─────────────────────────────────────────────────────────┤
│                   Framework Layer                        │
│  (workflow orchestration, multi-agent, evaluation)     │
│   LangChain/LangGraph, CrewAI, AutoGen, AutoGPT Platform │
├─────────────────────────────────────────────────────────┤
│                     SDK Layer                            │
│  (LLM provider abstraction, tool calling, memory)      │
│   Claude Agent SDK, OpenAI Agents SDK, Google ADK,      │
│   LlamaIndex, PydanticAI, Semantic Kernel                │
├─────────────────────────────────────────────────────────┤
│                  Platform Layer                          │
│  (hosted, managed, enterprise)                          │
│   Dify, n8n, Zapier, Microsoft Copilot, Gemini CLI      │
└─────────────────────────────────────────────────────────┘
```

### 1.1 OS-Level Agents (Oxios의 직접 경쟁)

#### OpenClaw / miniclaw-os
**GitHub:** `augmentedmike/miniclaw-os`  
**접근:** Persistent autonomous Agent OS built on OpenClaw. Agents as processes, tools as resources.  
**특징:**
- Unix-like process model for agents
- Persistent agent state across sessions
- Tool registry (file, bash, web search 등)
- Session management

#### OS-Level Claude (Anthropic)
**접근:** Claude computer use + OS-level tool abstractions.  
**특징:**
- OS-level file operations, terminal control
- Browser automation via CDP
- 현재는 CLI 도구 수준, 완전한 OS는 아님

#### Gemini CLI (Google)
**GitHub:** Google's open-source AI agent.  
**특징:**
- Command-line autonomous agent
- File system + terminal tools
- Workspace-aware execution
- Sandboxed execution environment

### 1.2 Framework Layer

#### LangChain / LangGraph
**상태:** Production (v0.3+)  
**접근:** General-purpose LLM application framework. LangGraph adds DAG-based agent orchestration.  
**강점:** 가장 큰 생태계, 문서, 커뮤니티. 1,000+ integrations.  
**약점:** Over-engineered, performance overhead, complexity.

#### CrewAI
**상태:** Production  
**접근:** Role-based multi-agent. Agents have roles (e.g., "researcher", "writer"), goals, and tools.  
**강점:** 직관적 API, multi-agent 협업 단순화  
**약점:** 단일 에이전트 복잡한 작업에는 부족, eval 루프 없음

#### AutoGen / Agent2 (Microsoft)
**상태:** Production (AG2 rebranding)  
**접근:** Multi-agent conversation + code execution. Group chat, hierarchical chat.  
**강점:** Microsoft 기업 지원, code execution 내장, 다양한 대화 패턴  
**약점:** Windows-centric historical roots, 복잡한 설정

#### AutoGPT Platform (AutoGPT)
**상태:** Enterprise + Open Source  
**접근:** Visual agent builder + autonomous execution platform.  
**강점:** No-code UI, autonomous goal decomposition  
**약점:** 자율성에만 집중, traditional OS 기능 부족

#### OpenAI Swarm
**상태:** Research/Experimental  
**접근:** Lightweight multi-agent orchestration via handoffs.  
**강점:** Minimal API, handoff pattern elegant  
**약점:** Production-ready 아님, evaluation 없음

### 1.3 SDK Layer

#### Claude Agent SDK (Anthropic)
**접近:** SDK-first, production-grade.  
**특징:**
- Built-in tools: Bash, Edit, Read, Write, Glob, Notebooks
- MCP support (Model Context Protocol)
- Streaming responses
- Local development focus

#### OpenAI Agents SDK
**접근:** OpenAI의 official agent SDK.  
**특징:**
- Handoffs pattern
- Guardrails for output validation
- Streaming + callbacks
- Guardrail-based safety

#### Google Agent Development Kit (ADK)
**상태:** GA (2025)  
**특징:**
- Multi-agent orchestration
- Gemini integration
- Vertex AI deployment
- Enterprise-grade

### 1.4 Academic / Research

#### Agent-OS Blueprint (Aniskoubaa et al.)
**위치:** TechRxiv preprint  
**접근:** "Treat agents as processes, models and tools as resources."  
**핵심 개념:**
- Agents as first-class OS processes
- Tool registry as system calls
- Memory hierarchy (register → cache → RAM → disk)
- Scheduler for agent scheduling
- Agent IPC (message passing)

---

## 2. Oxios Architecture Deep-Dive

### 2.1 Kernel Modules

| Module | 크기 | 기능 |
|--------|------|------|
| `supervisor` | 200줄 | Agent fork/exec/wait/kill |
| `event_bus` | ~200줄 | broadcast kernel events |
| `state_store` | 520줄 | Markdown/JSON persistent state |
| `scheduler` | 842줄 | Task scheduling, zombie reap |
| `access_manager` | 1676줄 | RBAC + audit log |
| `orchestrator` | 402줄 | Ouroboros lifecycle coordinator |
| `agent_runtime` | 713줄 | oxi-agent wrapper |
| `agent_lifecycle` | 170줄 | Fork→register→run→cleanup |
| `mcp` | 1227줄 | MCP bridge multi-server |
| `host_exec` | 524줄 | UDS relay + security |
| `container` | 890줄 | Apple Container backend |
| `circuit_breaker` | 176줄 | LLM outage protection |
| `metrics` | 325줄 | Prometheus registry |
| `program` | 1151줄 | Installable programs |

### 2.2 Ouroboros Protocol (고유)

```
사용자 메시지
    ↓
Interview ─── 모호성 > 0.2? ──→ 질문 반환 (재시작)
    ↓ (ambiguity ≤ 0.2)
Seed Generation (LLM)
    ↓
Execute ──── AgentRuntime (oxi-agent)
    ↓
Evaluate (LLM + mechanical check)
    ↓ (score < 0.8, iterations < 3)
Evolve ──── 개선된 Seed
    ↓
재실행 → 평가 → ...
```

**고유점:** 다른 어떤 framework에도 없는 spec-first workflow.

### 2.3 Channel-agnostic Gateway

```
Channels (Web, CLI, Telegram, ...)
    ↓
Gateway (trait Channel)
    ↓
Orchestrator (Ouroboros)
    ↓
Kernel components
    ↓
Response back via channel
```

이 구조는 OpenAI Swarm의 handoff보다 체계적입니다.

### 2.4 Tool Architecture (5-tier)

```
Tier 1: oxi native (ReadTool, WriteTool, EditTool, GrepTool, FindTool, LsTool)
Tier 2: Oxios execution (ContainerExecTool, HostExecTool)
Tier 3: Program tools (dynamic, from SKILL.md)
Tier 4: MCP servers (pre-registered)
Tier 5: Memory tools (write/read/search)
```

### 2.5 Memory System (Phase A 완료)

| Type | 용도 |
|------|------|
| `Conversation` | Compaction summaries |
| `Session` | Session metadata |
| `Fact` | Agent-saved facts |
| `Episode` | Work episodes |
| `Knowledge` | Persistent knowledge |

**현재:** Keyword-based search only. Phase B: vector search planned.

---

## 3. Capability Matrix

### 3.1 Agent Lifecycle

| Capability | Oxios | LangChain | CrewAI | AutoGen | AutoGPT | Claude SDK | OpenAI SDK | miniclaw-os |
|---|---|---|---|---|---|---|---|---|
| Create/fork agent | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Kill/terminate | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Stateful sessions | ✅ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ✅ |
| Persistent agents | ⚠️ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Multi-turn context | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Agent lifecycle events | ✅ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ✅ |

### 3.2 Tool System

| Capability | Oxios | LangChain | CrewAI | AutoGen | AutoGPT | Claude SDK | OpenAI SDK | miniclaw-os |
|---|---|---|---|---|---|---|---|---|
| Built-in tools | ⚠️ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ⚠️ |
| Custom tool registration | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Tool allowlisting | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ⚠️ |
| MCP integration | ✅ | ⚠️ | ❌ | ❌ | ⚠️ | ✅ | ❌ | ❌ |
| Program/skill system | ✅ | ❌ | ❌ | ❌ | ⚠️ | ❌ | ❌ | ❌ |
| Container isolation | ✅ | ❌ | ❌ | ⚠️ | ❌ | ❌ | ❌ | ⚠️ |
| Host exec bridge | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |

### 3.3 Memory

| Capability | Oxios | LangChain | CrewAI | AutoGen | AutoGPT | Claude SDK | OpenAI SDK | miniclaw-os |
|---|---|---|---|---|---|---|---|---|
| Short-term (context window) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Long-term (vector store) | ⚠️ | ✅ | ⚠️ | ⚠️ | ✅ | ⚠️ | ⚠️ | ⚠️ |
| Session persistence | ✅ | ❌ | ❌ | ❌ | ⚠️ | ❌ | ❌ | ✅ |
| Memory types (structured) | ✅ | ⚠️ | ❌ | ❌ | ⚠️ | ❌ | ❌ | ❌ |
| Auto-memory on compaction | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |

### 3.4 Multi-Agent

| Capability | Oxios | LangChain | CrewAI | AutoGen | AutoGPT | Claude SDK | OpenAI SDK | miniclaw-os |
|---|---|---|---|---|---|---|---|---|
| Multi-agent orchestration | ❌ | ⚠️ | ✅ | ✅ | ✅ | ❌ | ⚠️ | ⚠️ |
| A2A protocol | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ⚠️ |
| Role-based agents | ⚠️ | ⚠️ | ✅ | ⚠️ | ⚠️ | ❌ | ⚠️ | ❌ |
| Agent handoffs | ❌ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ❌ | ✅ | ❌ |
| Hierarchical agents | ❌ | ⚠️ | ⚠️ | ✅ | ⚠️ | ❌ | ⚠️ | ❌ |

### 3.5 Security

| Capability | Oxios | LangChain | CrewAI | AutoGen | AutoGPT | Claude SDK | OpenAI SDK | miniclaw-os |
|---|---|---|---|---|---|---|---|---|
| RBAC/permissions | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Audit logging | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Shell injection prevention | ✅ | ❌ | ❌ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ |
| Container isolation | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ⚠️ |
| Rate limiting | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Circuit breaker | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Path traversal protection | ✅ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ |
| Auth middleware | ✅ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ |

### 3.6 Channels

| Capability | Oxios | LangChain | CrewAI | AutoGen | AutoGPT | Claude SDK | OpenAI SDK | miniclaw-os |
|---|---|---|---|---|---|---|---|---|
| REST API | ✅ | ⚠️ | ⚠️ | ⚠️ | ✅ | ❌ | ⚠️ | ❌ |
| WebSocket | ✅ | ⚠️ | ❌ | ❌ | ✅ | ❌ | ⚠️ | ❌ |
| SSE/streaming | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| CLI interactive | ❌ | ⚠️ | ❌ | ❌ | ✅ | ⚠️ | ⚠️ | ✅ |
| TUI dashboard | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ⚠️ |
| Web UI | ✅ | ⚠️ | ⚠️ | ⚠️ | ✅ | ❌ | ❌ | ❌ |

### 3.7 Observability

| Capability | Oxios | LangChain | CrewAI | AutoGen | AutoGPT | Claude SDK | OpenAI SDK | miniclaw-os |
|---|---|---|---|---|---|---|---|---|
| Prometheus metrics | ✅ | ❌ | ❌ | ❌ | ⚠️ | ❌ | ❌ | ❌ |
| Structured logging | ✅ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ |
| Health endpoints | ✅ | ❌ | ❌ | ❌ | ⚠️ | ❌ | ❌ | ❌ |
| OpenTelemetry | ❌ | ⚠️ | ❌ | ❌ | ⚠️ | ⚠️ | ⚠️ | ❌ |
| Pagination | ✅ | ❌ | ❌ | ❌ | ⚠️ | ❌ | ❌ | ❌ |

### 3.8 Protocol & Methodology

| Capability | Oxios | LangChain | CrewAI | AutoGen | AutoGPT | Claude SDK | OpenAI SDK | miniclaw-os |
|---|---|---|---|---|---|---|---|---|
| Spec-first protocol | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ⚠️ |
| Iterative eval+evolve | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Seed/versioning | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Evaluation framework | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |

### 3.9 Production Readiness

| Capability | Oxios | LangChain | CrewAI | AutoGen | AutoGPT | Claude SDK | OpenAI SDK | miniclaw-os |
|---|---|---|---|---|---|---|---|---|
| Config hot-reload | ✅ | ❌ | ❌ | ❌ | ⚠️ | ❌ | ❌ | ⚠️ |
| Graceful shutdown | ✅ | ⚠️ | ❌ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ |
| Input validation | ✅ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ |
| Pagination | ✅ | ❌ | ❌ | ❌ | ⚠️ | ❌ | ❌ | ❌ |
| Test coverage | ✅ | ✅ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ❌ |
| Circuit breaker | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |

---

## 4. Radar Chart Summary

```
                    Multi-Agent
                        ↑
                       /|\\
                      / | \\
                     /  |  \\
        Security ───/   |   \\── Channels
           ↑            |        ↑
           |            |        |
    Protocol ←─────────┼───────→ Observability
           |            |        |
    Tool System ───────●─────── Memory
           ↑          / \
          /|\\       /   \
         / | \\     /     \\
        /  |  \\   /       \\
       /   |   \\ /         \\
      /    |    X           \\
     /     |     \\          \\
Agent ←───┼──────→ Production
Lifecycle           Readiness
```

**Oxios 점수:**
- Agent Lifecycle: 8/10
- Tool System: 9/10 (5-tier + container + MCP + programs)
- Memory: 7/10 (Phase B vector needed)
- Multi-Agent: 4/10 (A2A exists but no orchestration)
- Security: 10/10 (most secure of all)
- Channels: 7/10 (TUI + CLI missing)
- Observability: 7/10 (metrics, no OTEL)
- Protocol: 10/10 (Ouroboros only)
- Production: 8/10 (circuit breaker, hot-reload, pagination)
- **Total: 70/100**

**평균 framework 점수 (추정):**
- Agent Lifecycle: 6/10
- Tool System: 7/10
- Memory: 6/10
- Multi-Agent: 7/10
- Security: 3/10
- Channels: 5/10
- Observability: 4/10
- Protocol: 2/10
- Production: 4/10
- **Total: 44/100**

---

## 5. Oxios 고유 가치제안

### 5.1 Ouroboros Protocol — Only One
어떤 framework도 spec-first로 interview → seed → execute → evaluate → evolve 사이클을 내장하지 않습니다. 이는:
- 에이전트가 명확한 목표 없이 실행되는 것을 방지
- Iterative improvement를 통한 품질 향상
- Seed를 통한 재현 가능한 실행

### 5.2 Agent OS Concept — Programs with SKILL.md
LangChain의 Chains나 CrewAI의 Roles과 달리, Oxios는 프로그램 설치 방식으로 확장합니다:
- 각 프로그램: `program.toml` + `SKILL.md` + `bin/`
- 에이전트가 프로그램을 "설치하고 사용" 가능
- Unix philosophy: 작고 composable

### 5.3 Channel-agnostic Architecture
모든 프레임워크가 API-first인데, Oxios는 Gateway 패턴으로 채널을 플러그인합니다:
- Web, CLI, Telegram 등 같은 pipeline
- 새로운 채널 추가가 100줄 이하

### 5.4 Security-first Design
보안 관련 점수에서 Oxios가 압도적입니다:
- RBAC + audit log (file persistence)
- Shell injection prevention
- Container isolation
- Rate limiting
- Circuit breaker
- Path traversal protection
- Workspace scoping

### 5.5 5-tier Tool Architecture
다른 framework의 flat tool list와 달리, Oxios는 계층 구조:
- Tier 1: Native file ops
- Tier 2: Secure execution (container + host)
- Tier 3: Dynamic programs
- Tier 4: MCP servers
- Tier 5: Memory

---

## 6. Competitive Gaps

### 6.1 Multi-Agent Orchestration (Critical)
Oxios에는 A2A 프로토콜이 있지만, 실제로 agent-to-agent 통신을 사용하는 orchestrator가 없습니다. CrewAI의 hierarchical manager나 AutoGen의 group chat 같은 것이 필요합니다.

**예시 필요 상황:**
```
User: "Review the backend code and also update the docs"

→ Backend reviewer agent: reads code, finds bugs
→ Docs agent: reads requirements, updates docs
→ (동시에 실행, 결과를 합침)
→ Final report
```

**해결:** `handle_multi_agent_message()` + `AgentGroup` 스케줄링

### 6.2 E2E Integration Testing (High)
269개 테스트가 있지만 전부 unit test. Real agent execution을 검증하는 integration test가 없습니다.

**필요:** `tests/e2e_test.rs`를 확장하여 실제 oxi-agent AgentLoop를 테스트

### 6.3 Vector Search — Memory Phase B (Medium)
현재 keyword-only search. Semantic recall을 위해 embeddings 필요.

**해결:** `oxi-ai`의 embedding provider 활용 또는 lightweight crate (`fastembed`, `tantivy`)

### 6.4 TUI (Medium)
`loop7-tui-design.md` 설계 문서만 있고 구현 없음.

**해결:** ratatui + crossterm, dashboard + chat 패널

### 6.5 CLI Interactive Channel (Medium)
`loop7-cli-channel.md` 설계 문서만 있고 구현 없음.

**해결:** reedline + CliChannel implementation

### 6.6 OpenAPI Spec (Medium)
53개 엔드포인트에 대한 Swagger/OpenAPI 문서 없음.

**해결:** `axum-api-builder` 또는 `utoipa` crate

### 6.7 Documentation & Community (High)
-autoGPT는 thousands of stars, 문서, YouTube tutorials. Oxios는 GitHub에 공개된 지 얼마 안 됨.

---

## 7. 생태계 내 포지셔닝

```
                    High Multi-Agent
                           │
        CrewAI ────────────┼──────────── AutoGen
              \\           │           /
               \\          │          /
    Low         \\         │         /         High Security
    Security ────┼─────────┼────────┼──────────── Oxios
                  │         │        │
                  │         │        │
    LangChain ────┴─────────┴────────┴─── AutoGPT Platform
                    Low Spec-First
                     Protocol
```

**Oxios 포지셔닝:**
- **Security-first Agent OS** — 가장 안전한 에이전트 실행 환경
- **Spec-first workflow** — Ouroboros only
- **Unix philosophy** — 작은 도구, composable
- **macOS-native** — Apple Container integration

**Target users:**
- 보안에 민감한 엔터프라이즈
- Specification-driven development teams
- macOS 개발자 (native container)
- 에이전트 신뢰성/추적성 중요시하는 사용자

---

## 8. Roadmap Recommendations

### Phase 8a: Multi-Agent Orchestration (1 week)
**가장 큰 기능적 격차.** A2A 프로토콜을 활용하여 agent-to-agent 메시징을 구현합니다.

```
AgentRegistry (기존)
    ↓ 확장
AgentGroup
    ↓
Hierarchical orchestration (manager가 subtasks 분배)
```

### Phase 8b: E2E Testing (3 days)
`tests/e2e_test.rs`를 확장하여 실제 agent execution을 검증합니다.

### Phase 8c: TUI + CLI (1 week each, 병렬)
parallel subagent로 ratatui dashboard와 reedline CLI를 동시에 구현합니다.

### Phase 8d: Vector Search (3 days)
`fastembed` 또는 `tantivy` 활용하여 semantic memory recall 구현.

### Phase 8e: Documentation (지속)
- Quick start guide
- Architecture overview
- API reference (utoipa)
- Tutorial: building a program

---

## 9. 경쟁력 평가 요약

```
╔══════════════════════════════════════════════════════════╗
║              AGENT OS 경쟁력 평가                          ║
╠══════════════════════════════════════════════════════════╣
║                                                          ║
║  Oxios vs 평균 Framework:                               ║
║                                                          ║
║  Security:       Oxios 10  vs 평균 3   ←碾压胜          ║
║  Protocol:       Oxios 10  vs 평균 2   ←碾压胜          ║
║  Tool System:    Oxios 9   vs 평균 7   ←小幅领先       ║
║  Agent Lifecycle:Oxios 8   vs 평균 6   ←小幅领先       ║
║  Production:     Oxios 8   vs 평균 4   ←领先          ║
║  Memory:         Oxios 7   vs 평균 6   ←持平          ║
║  Channels:       Oxios 7   vs 평균 5   ←小幅领先       ║
║  Observability:  Oxios 7   vs 평균 4   ←领先          ║
║  Multi-Agent:    Oxios 4   vs 평균 7   ←落后          ║
║                                                          ║
║  Overall:        Oxios 70  vs 평균 44  ←领先59%     ║
║                                                          ║
╠══════════════════════════════════════════════════════════╣
║  가장 큰 격차: Multi-Agent Orchestration                ║
║  가장 큰 강점: Security + Protocol + Tool Architecture   ║
╚══════════════════════════════════════════════════════════╝
```

### 핵심 발견

1. **Security는 Oxios의 가장 큰 차별점입니다.** 어떤 framework도 RBAC, audit log, circuit breaker, container isolation을 이렇게 모두 갖추고 있지 않습니다.

2. **Ouroboros protocol은 고유합니다.** spec-first + iterative evaluation + evolution 사이클은 다른 어떤 framework에도 없습니다.

3. **Multi-agent가 가장 큰 약점입니다.** A2A 프로토콜이 있지만 실제로 multi-agent 워크플로우를 오케스트레이션하는 기능이 없습니다. CrewAI의 manager-subordinate 패턴이 참조 구현입니다.

4. **Production readiness는 상위권입니다.** Circuit breaker, config hot-reload, pagination, input validation, graceful shutdown이 모두 구현되어 있습니다. LangChain/CrewAI/AutoGen보다 나은 수준입니다.

5. **Channels는 보통 수준입니다.** REST API + Web + SSE + WebSocket은 잘 갖춰져 있지만, TUI와 interactive CLI는 설계만 있고 구현이 없습니다.

---

*Research conducted: 2026-05-07. Competitor data based on public documentation, GitHub repos, and known capabilities. Subject to change as ecosystem evolves rapidly.*