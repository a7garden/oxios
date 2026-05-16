<div align="center">

# ⬡ Oxios

**Agent Operating System**

*Where AI agents don't just talk — they work.*

[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![GitHub](https://img.shields.io/badge/GitHub-a7garden%2Foxios-181717?logo=github)](https://github.com/a7garden/oxios)

**Built with**

[![oxi](https://img.shields.io/badge/oxi-agent_runtime-6E40C9?logo=rust&logoColor=white)](https://github.com/a7garden/oxi)
&nbsp;
[![oxibrowser](https://img.shields.io/badge/oxibrowser-headless_browser-00A86B?logo=rust&logoColor=white)](https://github.com/a7garden/oxibrowser)
&nbsp;
[![ouroboros](https://img.shields.io/badge/ouroboros-specification_framework-E95420?logo=rust&logoColor=white)](https://github.com/Q00/ouroboros)

</div>

---

## Why Oxios?

Large language models are powerful, but they're stuck in chat boxes. Oxios gives them an **operating system** — lifecycle management, tool execution, state persistence, security boundaries, and an orchestration protocol — so agents can autonomously complete real tasks.

| The problem | What Oxios does |
|---|---|
| Agents die when the chat closes | **Supervisor** manages agent lifecycle: fork, exec, wait, kill |
| No specification → unreliable output | **[Ouroboros](https://github.com/Q00/ouroboros)**: interview → seed → execute → evaluate → evolve |
| Every app reinvents browser/execution | **Built-in engine**: headless browser, host exec, MCP bridge, programs |
| Agents have no memory between sessions | **State store** + **vector memory**: persistent, searchable knowledge |
| No security boundary between agents | **Access manager**: RBAC, path sandboxing, audit trail |

---

## Get Started

```bash
cargo install oxios
```

Set your LLM key, then run:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
oxios
```

Open **http://127.0.0.1:4200** — start talking to your agent.

That's it. The OS handles the rest.

---

## Architecture at a Glance

```
┌──────────────── Gateway ────────────────┐
│   Web · CLI · Telegram · Discord · …    │
│         (plugin channels)                │
└──────────────────┬──────────────────────┘
                   │
┌──────────────────▼──────────────────────┐
│               Kernel                      │
│                                           │
│  Supervisor · Ouroboros · Event Bus       │
│  State Store · Vector Memory · Scheduler  │
│  Access Manager · Audit Trail · Budget    │
│                                           │
│  ┌─────────────────────────────────────┐  │
│  │         Agent Runtime               │  │
│  │  oxi-agent + oxi-ai (multi-provider)│  │
│  │  read · write · edit · bash · grep  │  │
│  │  browser · programs · MCP · memory  │  │
│  └─────────────────────────────────────┘  │
└───────────────────────────────────────────┘
         │                    │
    ┌────▼────┐         ┌────▼────┐
    │  Host   │         │OxiBrowser│
    │  Exec   │         │(in-proc) │
    └─────────┘         └──────────┘
```

**No containers. No subprocess browser.** Everything runs in-process, sandboxed by workspace rules and RBAC.

---

## Core Concepts

### 🔄 Ouroboros Protocol

Powered by the [Ouroboros specification framework](https://github.com/Q00/ouroboros). Agents never execute blindly — every task starts with a specification.

```
Interview → Seed → Execute → Evaluate → Evolve
   ↑                                    │
   └────────────────────────────────────┘
```

### 🧭 Supervisor

Agent lifecycle as process management. Fork an agent, let it work, kill it if it misbehaves.

### 🌐 Built-in Browser

[OxiBrowser](https://github.com/a7garden/oxibrowser) — pure Rust headless browser, running in-process. ~10MB memory. No Chromium, no CDP overhead.

```
"Read this URL"    →  browse(url)              →  Markdown (one-shot)
"Fill this form"   →  goto → click → type      →  Interactive Tab session
"Run this JS"      →  evaluate(code)            →  JSON result
```

### 📦 Programs

OS-level installable capabilities for agents. Each program is a self-contained directory:

```bash
oxios program install ./my-program
oxios program list
```

### 🧠 Vector Memory

Agents remember across sessions. Semantic search with budget-aware curation.

### 🔒 Security

| Layer | Mechanism |
|-------|-----------|
| Tool access | RBAC per agent (capability-based) |
| File system | Workspace path sandboxing |
| Network | SSRF protection, robots.txt obedience |
| Execution | Command allowlist + metacharacter blocking |
| Audit | Immutable audit trail |

---

## Ecosystem

Oxios is part of the **a7garden** Rust AI stack:

| Project | Purpose |
|---------|---------|
| [**oxi**](https://github.com/a7garden/oxi) | LLM engine + agent runtime |
| [**oxibrowser**](https://github.com/a7garden/oxibrowser) | Pure Rust headless browser |
| [**ouroboros**](https://github.com/Q00/ouroboros) | Specification-first agent framework |
| **oxios** | Agent Operating System *(you are here)* |

```
oxi-ai ──── LLM abstraction (multi-provider)
oxi-agent ── Tool-calling agent loop
  │
ouroboros ── Specification-first protocol
  │
oxios-kernel ── Supervisor, tools, state, security
  │
oxios ── Binary + channels (Web, CLI, Telegram, …)
```

---

## License

[MIT](LICENSE)

---

<div align="center">

*Built by [a7garden](https://github.com/a7garden)*

</div>
