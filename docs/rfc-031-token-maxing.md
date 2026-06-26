# RFC-031: Token Maxing Mode

> Status: Design
> Author: design pass
> Related: RFC-029 (execution resilience), RFC-025 (Mounts/projects), RFC-011 (routing)

## 1. Goal

A mode that, during a **user-configured time window**, autonomously burns the
**subscription quota** of eligible providers to do useful work — draining each
provider's reset-window allocation, rotating across multiple subscription
providers, resuming after each window resets, and producing a **session
report** when the window ends.

Key property: it **must never** spend metered (per-token-billed) credit. Only
providers whose quota is a **reset-window allocation** (e.g. ZAI Coding Plan,
Minimax subscription) are eligible.

## 2. The hard rule: subscription-only, opt-in

There is no field today that says "this provider is subscription vs metered"
(`ProviderCategory` is `{Major, Open, Regional, Local}` — UI grouping only).
The distinction **cannot be auto-detected** (a provider's API is identical
whether you are on a metered key or a flat subscription key). Therefore
eligibility is an **explicit user annotation**:

```toml
[token-maxing]
enabled = false

# Eligible providers. Self-tracking against these limits IS the primary
# quota mechanism (see §4) — it needs no provider endpoint, so it works
# on ZAI/Minimax immediately.
[[token-maxing.providers]]
provider = "zai"
billing_model = "subscription"          # REQUIRED: the only accepted value (upholds the metered-never rule)
token_limit = 2000000                   # plan allocation per window — the self-tracked cap
reset_window_secs = 18000               # 5h — drain window + reset cadence
# Floor: stop draining this provider below this remaining% (avoid thrash).
min_remaining_percent = 5
# OPTIONAL recalibration: where the provider exposes a real usage/balance
# endpoint, periodically snap the self-tracked counter to it (erases drift
# from a shared key). Omit entirely = stay purely self-tracked.
# recalibration = "fetcher"              # "fetcher" | "header"
# Models within the provider to use (empty = all the provider's models).
models = ["zai/glm-4.6", "zai/glm-4.5-air"]

[[token-maxing.providers]]
provider = "minimax"
billing_model = "subscription"
token_limit = 3000000
reset_window_secs = 21600               # 6h
models = []
```

**Enforcement:** `billing_model = "subscription"` is the only accepted value.
Any provider without an entry here is **ineligible by default** — the engine
guarantees a metered key can never be silently drafted into maxing. This is
the single choke point that upholds the user's "절대 동작하면 안 된다" constraint.

## 3. What already exists (reuse, do not rebuild)

| Concern | Existing artifact | Reuse role |
|---|---|---|
| **Self-tracked consumption (PRIMARY)** | `BudgetManager` (`budget.rs`): `token_limit` / `tokens_used` / `window_secs` / `reset_if_expired` | The reusable primitive. Re-keyed from `AgentId` → **provider**, with `token_limit` = subscription allocation and `window_secs` = reset window. Works on ZAI/Minimax **today with zero endpoint work** — no dependency on whether their APIs expose `remaining`/`resets_at`. |
| **Provider-side recalibration (accuracy)** | `QuotaFetcher` + `RateWindow` (`src/api/quota.rs`) | Where a provider *does* expose a usage/balance endpoint, periodically **sync** the self-tracked counter to real state, erasing drift. Only OpenAI implemented; ZAI/Minimax adapters are an upgrade, not a gate. |
| Quota registry / fetch | `all_fetchers()`, `fetch_all(credentials)` (concurrent) | The recalibration fan-out. |
| Quota API + UI | `GET /api/costs/providers`, `ProviderQuotaCards`, `useProviderQuotas` | Live status already rendered; report view extends this. |
| Reactive failure detection | `classify.rs` → `FailureClass::{QuotaExhausted, RateLimited}` | **Reactive cooldown** trigger when a real request 429s. |
| Provider cooldown | `ProviderHealthRegistry` (`health.rs`): `reset_after_secs` | Mark a provider cooled-down until `resets_at`; reuse the breaker state machine. |
| Time-window activation | `CronScheduler` (`cron.rs`): tick loop, timeout, concurrency | Pattern for the activation window + per-task timeout + concurrency cap. |
| Work context | `MountManager` / `ProjectManager` (RFC-025/011), `path_promotion.rs` frequency tracking | "Registered projects" work source. |
| Skills | `SkillManager`, `SkillEntry` | "Frequently executed skills" work source (needs usage counter — see §7). |
| Execution | `AgentRuntime`, `AgentLifecycleManager`, `RecoveryCoordinator` | How a task actually runs; reuse `ExecEnv.model_override` to pin a provider. |
| Provider HTTP layer | `oxi-ai/src/providers/{anthropic,openai,google,mistral,...}.rs` (sibling crate) | **Header-derived quota** (refinement). Editable directly via `[patch.crates-io]` local path. |

### Quota signal ordering — self-tracked first

The decisive factor is **endpoint uncertainty**: §11 notes ZAI/Minimax may not
expose `remaining_percent` / `resets_at` on their usage APIs. A mechanism that
depends on that data is fragile for these targets. The `BudgetManager` pattern
— track used → compute remaining → reset on window — needs **no provider
endpoint at all** and is already fully built. So:

- **Primary (universal, Phase 1):** reuse the `BudgetManager` window+remaining
  pattern, **re-keyed by provider** (`HashMap<ProviderId, …>` not `AgentId`),
  with `token_limit` = the subscription plan allocation and `window_secs` =
  `reset_window_secs` (e.g. 5h). `tokens_used` increments on every
  `AgentEvent::Usage` from a maxing run. This makes `availability()` work on
  ZAI/Minimax immediately. **Tradeoff:** it counts only tokens oxios itself
  sent, so it **drifts** if the same API key is used outside oxios.
- **Accuracy upgrade (where an endpoint exists):** `QuotaFetcher` periodically
  **recalibrates** the self-tracked counter to real provider-side state,
  erasing that drift. OpenAI is done; ZAI/Minimax adapters are the upgrade,
  not a Phase-1 gate.
- **Refinement:** header-derived `RateWindow` from `oxi-ai` (our adjacent crate
  via `[patch.crates-io]`) — a lower-latency recalibration between polls.
- **Safety net:** reactive `classify.rs` cooldown forces `CooledDown` on a real
  `QuotaExhausted`/`RateLimited`, regardless of what the counter says.

All signals merge into one `QuotaTracker::availability(provider) -> Available | Draining | CooledDown(until)` decision.


## 4. QuotaTracker (new)

Unifies the signals into a per-provider availability verdict. The self-tracked
counter is the base; the others correct or override it. Lives in the kernel:

1. **Self-tracked counter (base/primary)** — a `ProviderBudget` keyed by
   provider (the `BudgetManager` pattern generalized off `AgentId`). `used`
   increments on each maxing `AgentEvent::Usage`; window resets at `resets_at`,
   or `now + reset_window_secs` from the last reset when `resets_at` is
   unknown. `remaining_percent = (1 - used/limit) * 100`.
2. **Recalibration (accuracy upgrade)** — `fetch_all()` on a cadence (default
   60s) where a `QuotaFetcher` exists for the provider. When it returns a real
   `remaining_percent` / `resets_at`, **snap** the counter to it — this is what
   erases drift from a shared key. Cheap, HTTP-bound, `retry: false`.
3. **Header feed (refinement)** — an oxi-ai hook publishes `RateWindow` updates
   as responses stream; treated like a recalibration but lower-latency.
4. **Reactive override (safety net)** — a `classify()` of `QuotaExhausted`/
   `RateLimited` forces `CooledDown(until = resets_at ?? now+reset_window_secs)`.

Decision logic (per provider):

```
availability(p):
  if reactive_cooldown[p] active:          return CooledDown(cooldown[p].until)
  rem% = counter[p].remaining_percent      # self-tracked base, snapped to last recalibration
  if rem% is null:                          return Available   # unknown → try (bounded by budget guard)
  if rem% <= min_remaining_percent:         return Draining    # near floor, finishing in-flight only
  return Available
```

`remaining_percent` reads the self-tracked counter, **snapped** to the most
recent recalibration (fetcher or header) when one is fresher than the last
reset. A 429 always wins via the reactive override even if the counter still
shows headroom — that is the drift failsafe.

## 5. TokenMaxer orchestrator (new)

The drain → rotate → wait → resume loop. Activated by a time window
(cron-like, reusing `CronScheduler`'s schedule parsing) OR a manual toggle.

```
run(window):
  session = TokenMaxingSession::start(window)
  while within window and not cancelled:
    pool = eligible providers sorted by availability (Available first, Draining last)
    live = pool.filter(|p| availability(p) != CooledDown)
    if live.is_empty():
      wait until min(cooldown[p].until for p in pool) or window-end   # the "all drained, wait for reset" case
      continue
    provider = live[0]
    task = work_planner.next_task(provider, session.history)
    if task is None:
      break                                                              # nothing to do
    result = execute(task, ExecEnv{ model_override: provider.next_model(), budget: guard })
    session.record(task, provider, result)
  report = session.finalize()
  persist(report); emit event
```

Properties this gives for free from the user's requirements:
- **"5시간 후 토큰 다시 제공되면 다시 동작"** → the `CooledDown(until)` +
  `wait until min(resets_at)` branch. After `resets_at` passes, the provider's
  `availability()` flips back to `Available` and draining resumes.
- **"구독형 여러 개면 알아서 전환"** → sorting `live` by availability; when
  provider A hits `Draining`/`CooledDown`, provider B becomes `live[0]`.
- **Safety net** → a per-run `AttemptBudget` (RFC-029) caps total executions so
  a runaway planner can't loop forever even if all signals read "available".

### Per-task `ExecEnv.model_override`

Reuse `ExecEnv.model_override` (already used by `RecoveryCoordinator` L2 swap)
to pin each task to the chosen provider's model. The `models` list per
provider is round-robined so a single provider's quota is spread across its
own models (some providers quota per-model).

### Concurrency

Single-stream by default (one task at a time) — subscription windows are
usually per-account, so parallelism within one provider gains nothing and
risks double-counting quota. Cross-provider parallelism (run a task on ZAI
while Minimax cools down) is a config option `parallel_providers = true`,
gated on whether `availability()` can still be trusted under concurrency
(header-derived yes; self-tracked-only no).

## 6. Safety & guardrails (unattended mode)

Token-maxing runs for hours ("하루종일") executing real work with nobody watching.
Every guardrail below reuses the existing stack — token-maxing *composes* it,
inventing no new policy. The decisive difference from interactive mode is §6.4:
the approval default inverts to **fail-closed**.

### 6.1 Capability scope
Run every maxing agent under a restricted `CapabilityTemplate::standard()`
(worker + memory read), **never** `operator()`/`supervisor()` (those grant
Space/Agent/A2A/Persona/MCP write). `standard()` keeps the agent to exec +
browser + recall; the exec right is further neutered by §6.2.

### 6.2 Workspace sandbox + exec policy
- Sandbox each agent to its target project workspace via
  `AccessManager::assign_workspace(agent, project_workspace)`. Writes outside
  the project are blocked and audited as `SandboxViolation`.
- `ExecConfig { allowlist_mode: AllowlistMode::Enforced, allow_shell_mode: false,
  allowed_commands: <read-only build set> }`. No shell mode; only the explicit
  allowlist (e.g. `cargo`, `git status`, `rg`, `cat`). Anything not on the list
  is denied at the gate (`gate.rs`).

### 6.3 Skill eligibility — explicit opt-in, not frequency
Only skills with frontmatter `autonomous: true` may fire unattended. A
frequently-used skill that sends email or runs `rm` must **not** auto-fire just
because it is popular. Frequency is a *ranking* signal among eligible skills,
never the gate (see §7 Source A).

### 6.4 CRITICAL — HitL approval is fail-closed, not pending
The existing human-in-the-loop path assumes a human is present:
`Action::requires_approval()` (ManageRBAC, SystemConfig, `osascript`, `rm`,
wildcard `*`) → `KernelEvent::ApprovalRequested` → `PendingToolApprovals`
waits on a oneshot for a human to approve. **During a burn window there is no
human.** A pending approval would hang the autonomous loop forever.

Therefore token-maxing **denies** high-risk actions outright instead of
pending them — the opposite default from interactive mode:
- `requires_approval()` actions → hard DENY (an error the agent can route
  around), never `ApprovalRequested`.
- The `ask_user` tool (`PendingAskUser`) is disabled for maxing agents — an
  agent that asks a question unattended also hangs.

This is the single most important policy difference and must be enforced at
the approval gate when the agent is tagged as a maxing run.

### 6.5 Audit
Every tool call flows through the existing `AuditSink` (Merkle audit trail).
The session report (§8) includes any denials / sandbox violations so the user
sees what the agent attempted and was blocked from.

## 7. WorkPlanner (new)

Selects the next task. Three sources, prioritized, each filtered by
"non-destructive and bounded" (maxing runs unattended — no `rm`, no
deploys, no outbound network beyond read):

### Source A — Skills (autonomous-eligible)
Add a `usage_count` + `last_used_at` to `SkillEntry` (or a side-table), bumped
on every skill invocation. Skills with frontmatter `autonomous: true` (new
field) and `usage_count >= threshold` become candidates. The planner picks the
least-recently-run eligible skill. This is the "자주 실행되던 스킬" axis.

### Source B — Projects / Mounts
`MountManager.list_mounts()` + `ProjectManager.list_projects()` give registered
work contexts. For each, the planner synthesizes a bounded, read-mostly task,
e.g. "review recent changes and summarize open work", "run lint/build and
report", "scan TODOs/FIXMEs". Work is **sourced from the project's paths** so
the agent has real context. This is the "현재 등록된 프로젝트" axis.

### Source C — Recurring patterns
Derived from session history: tasks the user runs on a recognizable cadence
(e.g. "weekly digest", "daily standup notes"). Reuse the
`path_promotion.rs` frequency approach generalized from paths → task intents.
Lowest priority; only used when A and B are exhausted (avoids inventing work).

### Planner contract
```
next_task(provider, history) -> Option<Task>:
  task = pick from A ∪ B ∪ C, skipping tasks already done this session
         and tasks whose acceptance criteria can't be verified cheaply
  task.budget_guard = per-task token cap (prevents one task from eating a whole window)
  task.constraints  = { no_shell, no_network, read_mostly, toolchain from skill/project }
  task.goal         = synthesized prompt
  return Some(task) or None
```

`None` terminates the window early — better to stop than fabricate work.

## 8. TokenMaxingSession + report

Persisted per run (reuse `StateStore::save_json("token-maxing", ...)`,
mirroring cron persistence):

```
TokenMaxingSession {
  id, started_at, ended_at, window { start, end },
  providers: [{ provider, models_used, tasks_run, tokens_consumed,
                windows_drained: [{ started, ended_at, resets_observed }] }],
  tasks: [{ source: skill|project|recurring, goal, provider, model,
            success, tokens, duration, summary }],
  totals: { tasks, tokens, providers_fully_drained, resets_observed },
}
```

The **report view** is a read of the last session: what ran, on which provider,
how much quota was burned, how many reset cycles were observed, and per-task
summaries. Surfaced at `GET /api/token-maxing/sessions/:id` and a new UI panel.
A run that ended with `None` (no more work) is reported differently from one
that ended on the window boundary — the user should see "stopped: nothing to
do" vs "stopped: window ended".

## 9. API & UI

- `POST /api/token-maxing/start` `{ window: {start,end} | manual }` → starts a
  session; refuses if no eligible (`billing_model=subscription`) provider has
  a working `quota_source`.
- `POST /api/token-maxing/stop` → graceful stop after in-flight task.
- `GET /api/token-maxing/status` → live: current provider, current task,
  per-provider `availability()`, tokens this session.
- `GET /api/token-maxing/sessions[/:id]` → history + report.
- `GET /api/token-maxing/providers` → eligibility summary (which providers are
  subscription-eligible, their quota_source health, last `RateWindow`).

UI: a "Token Maxing" panel under the existing Cost area (reuses
`ProviderQuotaCards` for live quota), a schedule configurator (cron-expression
or time-range picker), and the report view. Activation can also be a scheduled
job — conceptually a specialized cron entry that, instead of running one goal,
runs the drain loop for the window.

## 10. Phased implementation

**Phase 1 — Provider-keyed self-tracker (unblock everything, no endpoint work)**
- Generalize the `BudgetManager` window+remaining+reset pattern to a
  `ProviderBudget` keyed by provider (`HashMap<ProviderId, …>`). `token_limit`
  and `reset_window_secs` come from `[token-maxing.providers]`.
- Feed it from `AgentEvent::Usage` during maxing runs.
- This alone makes `availability()` work on ZAI/Minimax — no provider API calls.

**Phase 2 — Recalibration (accuracy, where endpoints exist)**
- Add `ZaiQuotaFetcher` / `MinimaxQuotaFetcher` to `quota.rs::all_fetchers()`
  *if* their usage APIs expose remaining/resets — each periodically snaps the
  Phase-1 counter to real state, erasing shared-key drift. Optional per
  provider; a missing fetcher just leaves that provider self-tracked.
- (Refinement) capture rate-limit headers in `oxi-ai` via the `[patch.crates-io]`
  local path as a lower-latency recalibration source.
- Wire reactive override into `classify.rs` + `ProviderHealthRegistry`.

**Phase 3 — TokenMaxer + planner (kernel)**
- Orchestrator loop, `ExecEnv.model_override` pinning, `AttemptBudget` guard.
- Skill `usage_count`; project/mount task synthesis; recurring-pattern stub.

**Phase 4 — Session/report + API + UI**
- Persistence, report view, schedule configurator, live status panel.

**Phase 5 — Cross-provider parallelism** (opt-in, gated on signal trust).

## 11. Risks & open questions

1. **ZAI/Minimax usage endpoints may not expose `remaining`/`resets_at`.**
   This is *why* self-tracking is the primary (§4): it never depends on those
   fields. The cost is **drift** — the self-tracked counter reflects only tokens
   oxios itself sent, so a key shared with another app/instance under-counts.
   Where a provider *does* expose a real endpoint, the Phase-2 `QuotaFetcher`
   recalibration snaps the counter back to truth; where it doesn't, the reactive
   429 cooldown (§4 safety net) is the failsafe. The report must label quota
   numbers *self-tracked* vs *recalibrated*.
2. **Quota double-counting under cross-provider parallelism.** Self-tracked
   consumption is per-process; if two tasks hit the same provider concurrently
   before either reports usage, both may read "available". **Mitigation:** a
   provider-local reservation lock (reserve before dispatch, settle on usage).
3. **Unattended safety.** See §6 — the full guardrail stack (capability scope,
   workspace sandbox, enforced exec allowlist, explicit skill opt-in, fail-closed
   HitL approval, audit trail). The non-obvious invariant is §6.4: high-risk
   actions are DENIED in unattended mode, whereas interactive mode PENDS them.
4. **`billing_model` is an honor-system annotation.** A user could mark a
   metered key as `subscription` and burn real money. **Mitigation:** the
   config validator cross-checks: if `billing_model=subscription` but the
   provider's models.dev `cost_input`/`cost_output` are non-zero AND no
   subscription endpoint confirms a flat plan, emit a loud warning. Cannot be
   made airtight (the API can't prove your billing plan), so this is a guardrail,
   not a guarantee.
5. **Report fidelity when a window straddles multiple resets.** Track
   `windows_drained[]` per provider so the report distinguishes "drained 1
   window on ZAI, then 1 on Minimax, then ZAI reset and drained again".
