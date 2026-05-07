# Oxios Progress Tracker

## 2026-05-07: Agent OS Research

### ✅ Completed: Deep Investigation into "Agent Operating System"

**Output:** `/tmp/oxios-agent-os-research.md`

**Key Findings:**
- Agent OS ≠ Agent Framework ≠ Agent SDK — the OS provides process management, scheduling, IPC, security (like Linux for processes)
- **AIOS** (COLM 2025) is the foundational academic paper — defines kernel modules: scheduler, context manager, memory manager, storage manager, tool manager, access manager
- **AgenticOS 2026** — first ASPLOS workshop on OS design for AI agents, signals formal recognition by systems research community
- **OpenFang** — most relevant existing project: Rust-based Agent OS, 180ms cold start, production-oriented
- **AARM** — emerging security spec (CSA, donated by Vanta) — the "SELinux for Agent OS"
- **MCP + A2A** — complementary protocols forming the communication stack (agent→tool + agent→agent)
- **OpenClaw / miniclaw-os** — NOT found in public sources; may be internal/private projects
- Multi-agent orchestration patterns converging on 4 models: Supervisor, Pipeline, Swarm, Hierarchical

**Status:** Research complete, ready for review.
