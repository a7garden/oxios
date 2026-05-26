# Oxios Top-Level Analysis

**Date:** 2026-05-26  
**Scope:** Binary entry point, kernel assembly, configuration, onboarding, daemon, Ouroboros protocol  
**Files Analyzed:** 11 source files (~5,500 lines)

---

## 1. User Journey: First Run → Executing a Prompt

### 1.1 Happy Path (First Run)

```
User runs: oxios
  │
  ├─ main.rs: parse CLI (no subcommand → default = Start)
  ├─ expand_home("~/.oxios/config.toml") → detect first run
  ├─ ensure_workspace() → create ~/.oxios/ + subdirs + default config.toml
  ├─ load_config() → OxiosConfig with empty default_model
  ├─ tracing setup (daily rolling logs)
  │
  ├─ needs_kernel = true (Start command)
  ├─ has_credentials() → false (no model, no API key)
  │
  ├─ run_onboarding()
  │   ├─ print_intro() → "⬡ Oxios Agent OS — Your AI agents, organized."
  │   ├─ Auto-detect: scan env vars (OPENAI_API_KEY, ANTHROPIC_API_KEY, etc.)
  │   │   ├─ Found? → "Use this provider?" prompt
  │   │   └─ Not found → provider selection list (filtered, sorted by env key)
  │   ├─ resolve_api_key() → auth.json → env var → manual entry
  │   ├─ prompt_model() → list from provider, or manual entry
  │   ├─ persist_config() → write config.toml
  │   ├─ setup_embedding() → download GGUF model (~329MB) if embedding-gguf feature
  │   └─ print_summary() → LLM, provider, key source, embedding, home dir
  │
  ├─ Kernel::builder().config_path(...).build()
  │   ├─ load_config()
  │   ├─ Create EventBus, StateStore
  │   ├─ Resolve model → create Provider (via oxi_sdk)
  │   ├─ OuroborosEngine (LLM-backed protocol)
  │   ├─ AccessManager, AgentScheduler, PersonaManager
  │   ├─ A2AProtocol (inter-agent communication)
  │   ├─ GitLayer, SkillManager, McpBridge
  │   ├─ MemoryManager (+ SQLite backend if feature enabled)
  │   ├─ DreamProcess (background consolidation)
  │   ├─ BudgetManager, AuthManager, AuditTrail, CronScheduler, ResourceMonitor
  │   ├─ SpaceManager
  │   ├─ KernelHandle (13 typed APIs)
  │   ├─ AgentRuntime (wraps oxi-agent tool-calling loop)
  │   ├─ BasicSupervisor → AgentLifecycleManager
  │   ├─ Orchestrator (Ouroboros + lifecycle + spaces)
  │   └─ Gateway
  │
  ├─ No subcommand → daemon start
  │   ├─ --foreground? → cmd_serve()
  │   │   ├─ init_mcp_servers()
  │   │   ├─ init_default_skills()
  │   │   ├─ activate_channels() (web, cli, telegram)
  │   │   ├─ start_guardian() (audit chain + resource checks every 5 min)
  │   │   ├─ start_daily_health_check() (3 AM: web UI update check)
  │   │   └─ gateway.run() on dedicated thread
  │   └─ background → fork self with --foreground
  │
  └─ "⬡ Oxios Agent OS v0.x.x" banner, ctrl+c to shutdown
```

### 1.2 `oxios run` Path

```
User runs: oxios run --json "review this code"
  │
  ├─ Fast-path: not a no-kernel command → falls through
  ├─ has_credentials() → true (already onboarded)
  ├─ Kernel::builder().build() → full assembly
  ├─ cmd_run(kernel, prompt, opts)
  │   ├─ build_effective_prompt() → optionally prepend context file
  │   ├─ Audit: log the run
  │   ├─ kernel.execute_prompt_with_session()
  │   │   └─ orchestrator.handle_message("cli", prompt, session_id)
  │   │       ├─ Interview phase (LLM: is this a task? ambiguity scoring)
  │   │       │   ├─ Chat response → return directly
  │   │       │   └─ Task → assess ambiguity
  │   │       │       ├─ Simple + low ambiguity → Seed::from_message()
  │   │       │       └─ Complex or high ambiguity → generate_seed() via LLM
  │   │       ├─ Execute via AgentLifecycleManager.spawn_and_run()
  │   │       ├─ Evaluation (mechanical + optional LLM semantic)
  │   │       └─ Return OrchestrationResult
  │   └─ JSON output: response, session_id, seed_id, evaluation_passed, etc.
  └─ process::exit(exit_code)
```

### 1.3 Journey Quality Assessment

| Aspect | Rating | Notes |
|--------|--------|-------|
| First-run friction | ⭐⭐⭐⭐ | Auto-detection of env keys and auth.json is smart. ~30 seconds claimed, realistic for the provider→key→model flow. |
| Zero-config defaults | ⭐⭐⭐⭐⭐ | Sensible defaults everywhere. 24 config sections but only `engine.default_model` is strictly required. |
| Error guidance | ⭐⭐⭐⭐ | `oxios doctor` checks 8 things with actionable fixes. `oxios onboard` re-runs onboarding. |
| Daemon UX | ⭐⭐⭐ | `oxios` starts daemon silently. `oxios --foreground` for debug. But daemon output goes to log file, not stdout — first-time users may not know it started. |

---

## 2. Configuration Complexity

### 2.1 Configuration Surface

The `OxiosConfig` struct contains **24 top-level sections** with **~120 configurable fields**:

| Section | Fields | Purpose | User-Facing? |
|---------|--------|---------|-------------|
| `kernel` | 3 | Workspace, event bus, max agents | Rarely |
| `engine` | 6 | Model, API key, routing, fallbacks | **Yes — onboarding sets these** |
| `daemon` | 2 | PID file, log dir | Rarely |
| `gateway` | 2 | Host, port | Sometimes |
| `scheduler` | 3 | Concurrency, rate limit, zombie timeout | Advanced |
| `orchestrator` | 2 | Evolution iterations, eval score threshold | Advanced |
| `context` | 2 | Token limits for context window | Advanced |
| `security` | 10 | Tools, network, execution time, CORS, audit | Admin |
| `persona` | 2 | Default persona, max concurrent | Rarely |
| `memory` | 8+ | Recall limits, summarization, retention | Rarely |
| `memory.sqlite` | 4 | SQLite backend, embedding dimension | Advanced |
| `memory.embedding` | 3 | Provider (gguf/tfidf), dimension, TTL | Advanced |
| `memory.learning` | 5 | SONA mode, distillation, auto-promote | Advanced |
| `memory.bridge` | 2 | MEMORY.md sync | Rarely |
| `memory.consolidation` | 18 | Dream, tiers, decay, protection, compaction | Expert |
| `cron` | 2+ | Jobs, tick interval | Advanced |
| `mcp` | 1+ | Server definitions (command, args, env) | **Yes — power users** |
| `git` | 1 | Auto-commit toggle | Rarely |
| `audit` | 2 | Max entries, enabled | Rarely |
| `budget` | 4 | Token/call budgets, window | Admin |
| `exec` | 5 | Mode, allowlist, timeouts | Admin |
| `resource_monitor` | 5 | Intervals, thresholds | Expert |
| `otel` | 4 | OpenTelemetry endpoint, sampling | Expert |
| `logging` | 2 | Format, level | Sometimes |
| `channels` | 2+ | Enabled channels, telegram config | Sometimes |
| `browser` | 2 | Enabled, engine config | Rarely |
| `session` | 3 | Max sessions, TTL, auto-prune | Rarely |
| `marketplace` | 2 | ClawHub URL, enabled | Rarely |

### 2.2 Organization Assessment

**Strengths:**
- Every field has a sensible default — users can run with zero manual config
- `default-config.toml` is well-commented with examples
- Config validation catches 10+ error conditions with actionable messages
- `oxios config show/set/get` for runtime inspection
- `api_key` is `skip_serializing` — won't leak in API responses

**Issues:**
1. **Config key coverage in `get_config_value`/`set_config_value` is incomplete** — only 9 of ~120 fields are settable via `oxios config set`. Missing: memory.*, security.*, scheduler.*, cron.*, otel.*, persona.*, budget.*, channels.*, browser.*, session.*, marketplace.*. Users must edit config.toml directly for these.
2. **`memory.consolidation` has 18 fields** — this is a config surface nightmare. Most users will never touch these, but they're all deserialized. A preset system ("conservative", "balanced", "aggressive") would reduce this.
3. **`max_agents` mismatch** — default-config.toml says `10`, `KernelConfig::default()` says `16`. The TOML file is written on first run, so the file wins, but the inconsistency is confusing.
4. **Gateway port** — default-config.toml says `4200`, `GatewayConfig::default()` says `4200`, but `cmd_web()` constructs URLs with the config port. The `default-config.toml` has `host = "0.0.0.0"` but code default is `"127.0.0.1"`. This is intentional (TOML = production-ready, code = dev-safe), but could confuse.

### 2.3 Default Config vs Code Defaults

| Field | `default-config.toml` | Rust `Default` | Match? |
|-------|----------------------|-----------------|--------|
| `kernel.max_agents` | 10 | 16 | ❌ |
| `gateway.host` | `0.0.0.0` | `127.0.0.1` | ❌ (intentional) |
| `gateway.port` | 4200 | 4200 | ✅ |
| `memory.consolidation` | commented out | full defaults | ✅ (TOML omitted = Rust default) |
| `exec.default_mode` | `structured` | `Structured` | ✅ |

---

## 3. Ouroboros Protocol Analysis

### 3.1 Protocol Design

The protocol is a **five-phase lifecycle**: `Interview → Seed → Execute → Evaluate → Evolve`

```
                    ┌─────────────┐
                    │  Interview   │ ← LLM: classify (task vs chat) + ambiguity scoring
                    └──────┬──────┘
                           │ ambiguity ≤ 0.2?
                    ┌──────▼──────┐
                    │    Seed     │ ← LLM: crystallize into immutable spec
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │   Execute   │ ← AgentRuntime (tool-calling loop via Supervisor)
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │  Evaluate   │ ← 3-stage: mechanical → semantic → consensus
                    └──────┬──────┘
                           │ score < 0.8?
                    ┌──────▼──────┐
                    │   Evolve    │ ← LLM: improve seed, loop back to Execute
                    └─────────────┘
```

### 3.2 Current Implementation State

| Phase | Status | Implementation Notes |
|-------|--------|---------------------|
| **Interview** | ✅ Fully wired | LLM classifies task vs chat, scores ambiguity along 3 dimensions. Falls back to degraded heuristic on JSON parse failure. |
| **Seed** | ✅ Fully wired | LLM generates structured spec. Simple tasks (`complexity == "simple"`) bypass this via `Seed::from_message()`. |
| **Execute** | ⚠️ Delegated, not via protocol | `OuroborosEngine::execute()` exists but is `#[allow(dead_code)]`. Orchestrator calls `AgentLifecycleManager::spawn_and_run()` directly. |
| **Evaluate** | ⚠️ Partially wired | `OuroborosEngine::evaluate()` exists but is `#[allow(dead_code)]`. Orchestrator does a simpler inline evaluation. Full 3-stage (mechanical + semantic + consensus) is not wired. |
| **Evolve** | ❌ Not wired | `OuroborosEngine::evolve()` exists but is `#[allow(dead_code)]`. The evolution loop is not connected. `lateral.rs` and `regression.rs` are commented out. |

### 3.3 What Actually Happens

The Orchestrator (in `oxios-kernel`) runs a **simplified flow**:

```
Interview → (if simple) Seed::from_message() / (if complex) generate_seed()
           → AgentLifecycleManager::spawn_and_run()
           → Simple evaluation
           → Return result
```

The full Ouroboros loop (execute → evaluate → evolve → re-execute) is **architected but not connected**. The orchestrator has `max_evolution_iterations = 3` in config but the loop body isn't active.

### 3.4 Protocol Complexity Assessment

| Metric | Value |
|--------|-------|
| System prompts | 4 (Interview, Seed, Evaluate, Evolve) — ~500 lines total |
| LLM calls per request (simple task) | 1 (interview only) |
| LLM calls per request (complex task) | 2-3 (interview + seed + optional eval) |
| LLM calls per request (with evolution) | 4-12 (not wired) |
| JSON schemas to parse | 4 (InterviewResponse, SeedResponse, EvaluationResponse, SeedResponse for evolve) |
| Degraded fallback | ✅ Each phase has a non-LLM fallback |

**Key concern:** The protocol does **4+ LLM calls per request** before the agent even starts executing. For `oxios run "fix the typo in main.rs"`, the flow is:
1. Interview LLM call → classify as simple task
2. Skip seed generation (use `Seed::from_message()`)
3. Agent executes (another LLM call for the actual work)
4. Optional evaluation LLM call

That's 2-3 LLM calls minimum. For complex tasks, it's 3-4. This adds latency and cost.

### 3.5 Interview Phase Quality

The interview phase is well-designed:

- **Task vs. chat classification** prevents unnecessary orchestration for "hello" / "thanks"
- **Complexity detection** ("simple" vs "complex") enables fast-path for clear requests
- **Ambiguity scoring** with honest-low scoring policy is a good design
- **Degraded fallback** via `degraded.rs` ensures the system works even with bad LLM output

**Weakness:** The interview is a **single-shot** — it classifies and scores in one LLM call. True Socratic interview (ask → answer → re-assess → ask again) is designed but not implemented for interactive use. The multi-turn happens at the Orchestrator level, not within the interview phase.

---

## 4. Daemon UX Analysis

### 4.1 Daemon Lifecycle

| Command | Behavior |
|---------|----------|
| `oxios` | Start daemon in background (default) |
| `oxios start` | Same as default |
| `oxios --foreground` | Run in foreground (debugging) |
| `oxios stop` | SIGTERM + cleanup |
| `oxios restart` | Stop + start |
| `oxios daemon install` | Install as launchd/systemd service |
| `oxios daemon uninstall` | Remove system service |
| `oxios log --lines 50` | Tail log file |

### 4.2 Strengths

1. **Silent background start** — daemon forks and detaches cleanly
2. **Stale PID detection** — if process died without cleanup, stale PID is auto-cleaned
3. **Cross-platform service management** — launchd (macOS) and systemd (Linux) with proper plist/unit generation
4. **Graceful shutdown** — ctrl+c → signal gateway → cancel channels → join gateway thread → kill agents → flush audit → MCP shutdown
5. **Daily health check** — auto-updates web UI from GitHub releases at 3 AM

### 4.3 Issues

1. **No startup confirmation in background mode** — `oxios` forks and the parent exits. The only output is `⬡ oxios started (PID 1234)` to stdout, but if the user isn't watching, they miss it. There's no `--wait` flag to block until the gateway is listening.

2. **Daemon startup failures are silent** — If the daemon crashes immediately (e.g., port in use), the user sees "started (PID 1234)" but the process is dead. No health check after fork.

3. **Log file is rolling daily** — good for production, but the `oxios log` command reads the full file and tails in memory. For large logs this could be slow. No `--follow` / tail -f mode.

4. **No `oxios status` daemon health endpoint** — `cmd_status()` checks PID but doesn't verify the gateway is actually responding. A TCP check to the gateway port would be more reliable.

5. **`oxios web` auto-starts daemon** — Good UX, but the wait loop (20 × 300ms = 6s timeout) is fragile. If the daemon takes longer to start (e.g., MCP server initialization), it gives up.

---

## 5. User-Facing Inconsistencies

### 5.1 Critical Inconsistencies

| # | Issue | Impact | Location |
|---|-------|--------|----------|
| 1 | **`max_agents` default mismatch**: TOML=10, Rust=16 | First-run users get 10 (from TOML), but anyone constructing config programmatically gets 16 | `share/default-config.toml` vs `config.rs` |
| 2 | **`gateway.host` default mismatch**: TOML=`0.0.0.0`, Rust=`127.0.0.1` | TOML binds publicly (security concern for unconfigured machines), Rust default is localhost-only | `share/default-config.toml` vs `config.rs` |
| 3 | **Config `set` only supports 9/120 fields** | Users can't configure most settings via CLI. Must edit TOML manually. | `main.rs:get_config_value/set_config_value` |
| 4 | **`allowed_commands` in default config includes `osascript`** | On macOS, `osascript` can run arbitrary AppleScript — this is a security risk in the "structured" execution mode which is supposed to be safe | `share/default-config.toml` |
| 5 | **KnowledgeBase created twice** in kernel assembly | `KernelBuilder::build()` creates KnowledgeBase for KernelHandle, then `Kernel::handle()` creates it again (cached). Different instances, same path. If the first one holds file locks, the second could fail. | `src/kernel.rs` |

### 5.2 Minor Inconsistencies

| # | Issue | Location |
|---|-------|----------|
| 6 | **`oxios pkg install` says "not yet implemented"** | `main.rs:cmd_pkg` — but `oxios marketplace install` works. Two install paths that do different things. |
| 7 | **`PkgAction::Search` lists all skills with bold names** — identical output to `PkgAction::List` but different formatting | `main.rs:cmd_pkg` |
| 8 | **`orchestrator.max_evolution_iterations` in config but evolution loop not wired** | User-facing config for a feature that doesn't work yet |
| 9 | **Gateway default port** in AGENTS.md says `3000`, but code and TOML say `4200` | `AGENTS.md` is outdated |
| 10 | **`oxios chat` requires `cli` feature** — error message says "Rebuild with --features cli" but user installed via `cargo install`, can't easily rebuild | `main.rs` Chat command |
| 11 | **`engine.default_model` defaults to empty string** — `OxiosConfig::default()` is not usable as-is (requires onboarding). But `Default::default()` for `OxiosConfig` doesn't indicate this. | `config.rs` |
| 12 | **Two workspace subdirectory lists** — `main.rs:WORKSPACE_SUBDIRS` and `onboarding.rs:WORKSPACE_SUBDIRS` are identical but duplicated | DRY violation |

### 5.3 Missing UX Features

| Feature | Current State | Recommendation |
|---------|---------------|----------------|
| `oxios config set` for nested keys | Only 9 flat keys supported | Support dot-notation for all fields (`memory.consolidation.dream_enabled`) |
| `oxios log --follow` | Reads static file | Add tail -f mode |
| `oxios status --json` | Human-readable only | Add `--json` flag for programmatic consumption |
| `oxios run --verbose` | No verbosity control | Show interview/seed/eval phases as they happen |
| Config file backup before modification | No backup on `config set` | Create `.bak` before overwriting |

---

## 6. Architecture Quality

### 6.1 Strengths

1. **Builder pattern for kernel assembly** — clean, testable, single point of wiring
2. **KernelHandle facade** — 13 typed APIs with clear separation of concerns
3. **Feature-gated channels** — web, cli, telegram compile independently
4. **Graceful degradation** — every Ouroboros phase has a non-LLM fallback
5. **Credential resolution chain** — config.toml → oxi auth.json → env vars (smart multi-source)
6. **Guardian daemon** — continuous integrity monitoring (audit chain, git repo, resource overload)
7. **Config validation** — catches errors at load time, not runtime

### 6.2 Technical Debt

1. **KnowledgeBase double creation** — created in `KernelBuilder::build()` for KernelHandle, then again in `Kernel::handle()`. The `handle_cache: OnceLock` means `Kernel::handle()` always creates a second instance, but the first one (built inline) is used for AgentRuntime. Two KnowledgeBases on the same directory.

2. **Dead code in Ouroboros** — `execute()`, `evaluate()`, `evolve()` are all `#[allow(dead_code)]`. The protocol trait defines a full lifecycle that isn't used. This should either be wired or removed from the trait (keep as a design doc, not dead code).

3. **`Box::leak(Box::new(_guard))`** — tracing appender guard and OTel guard are leaked to achieve `'static` lifetime. Acceptable for a long-running daemon, but prevents clean test teardown.

4. **No kernel assembly error recovery** — if any component fails (e.g., SQLite open), the entire build fails. No partial startup or degraded mode.

5. **AgentRuntime needs a KernelHandle that's partially constructed** — the builder creates a "placeholder" KernelHandle with a NoOpSupervisor, then creates the real supervisor, then creates the real KernelHandle (cached). This is fragile — if any code calls into the placeholder handle during build, it silently no-ops.

---

## 7. Summary Scorecard

| Category | Score | Key Takeaway |
|----------|-------|-------------|
| **First-run experience** | 8/10 | Smooth onboarding, auto-detection, ~30 seconds. Missing: no startup health check for daemon. |
| **Configuration design** | 7/10 | Sensible defaults, good validation. But 120 fields is a lot, and CLI only supports 9. |
| **Daemon UX** | 7/10 | Clean start/stop, cross-platform service install. Missing: `--wait`, health check, `--follow`. |
| **Ouroboros protocol** | 6/10 | Well-architected 5-phase design. But only 2.5/5 phases are actually wired. Dead code should be cleaned. |
| **Code quality** | 8/10 | Well-documented, consistent patterns, proper error handling. Some tech debt (double KB, dead code). |
| **User-facing consistency** | 6/10 | Default mismatches, incomplete CLI config, outdated docs (AGENTS.md port), confusing dual install paths. |

### Top 5 Actionable Items

1. **Fix `max_agents` and `gateway.host` defaults** — align TOML and Rust defaults, or add a comment explaining the intentional difference
2. **Wire the Ouroboros evaluation loop** or remove dead code — the gap between protocol design and implementation is the biggest architectural inconsistency
3. **Expand `oxios config set/get`** to cover all fields — 9/120 is not useful
4. **Remove `osascript` from default `allowed_commands`** — security risk in structured mode
5. **Fix KnowledgeBase double creation** — create once, share everywhere
