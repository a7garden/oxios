# Command Palette Redesign — `⌘K`

**Date:** 2026-06-29
**Status:** Design (decisions locked, review fixes + open forks resolved, pre-implementation)
**Scope:** Web UI — `web/src/components/layout/command-palette.tsx` + provider modules

---

## 1. Problem

The current `⌘K` palette is a **quick-capture bar wearing a command-palette name tag**.
Every input is treated as a noun (a memo to store). It has no verbs: you cannot run a
skill, switch a persona, kill an agent, or trigger a cron job from it. For an *Agent
Operating System*, the central surface for intent expression cannot reach the system's
own verbs.

Concrete defects in the current implementation:

| # | Defect | Evidence |
|---|---|---|
| D1 | No agent/skill/control verbs — navigation + memo capture only | `command-palette.tsx:169-312` |
| D2 | Capture routes hardcoded; new checklists never appear | `command-palette.tsx:45-51` `CAPTURE_ROUTES` |
| D3 | Ranking betrays intent: nav matches always beat the capture default | `command-palette.tsx:207` comment + `:109-113` |
| D4 | Ranking hacked via composite `value` strings (`capture-${key}-${text}`) | `command-palette.tsx:174,213,227` |
| D5 | Nav matching is locale-coupled (`t(labelKey)` substring) | `command-palette.tsx:112` |
| D6 | No recency/frequency learning — static ordering | — |
| D7 | `shouldFilter` claimed but never wired — cmdk silently filters/sorts by `value`, which is the **root cause** of D4 | `command.tsx:12-14` comment vs `:45` (no prop); `shouldFilter` grep = 0 usages |

## 2. Goals & Non-Goals

**Goals**
- Make the palette a true **verb-first intent surface** for the Agent OS.
- Deterministic, instant, muscle-memory-friendly (Unix-style prefix grammar).
- **Data-driven**: subsystems register their own commands; nothing hardcoded in core.
- Mode-aware: defaults and ranking adapt to Console / Knowledge / Chat.
- Migrate the existing `/` capture without behavior regression.

**Non-Goals (v1)**
- Natural-language intent classification (the LLM `assess` path). Considered and rejected
  for the palette — determinism wins here. NL routing remains a chat-channel concern.
- A separate palette per mode. One palette, mode-aware.

## 3. Locked Decisions

| Fork | Decision |
|---|---|
| **Dispatch model** | **Verb-prefix grammar** (deterministic). Bare text = **mode-primary**: console→go, knowledge→capture, chat→run. `>` is the explicit cross-mode run escape hatch. |
| **`run` semantics** | **Staged.** v1 = frontend message synthesis over existing per-message slots. v2 = new backend `persona_override` + `skill_override` slots (RFC, mirrors `model_override`). |
| **v1 scope** | **Full verb set:** `go`, `capture`, `run`, `switch`, `control`, `new`. |
| **`@persona` in v1** | **Deferred to v2.** v1 `run` targets: `@skill`, `@project`, no-entity only. Persona switching = `~ @persona` (global). Raw `@role` is never exposed to users. |
| **`+` (new) creation** | **Route to existing forms/dialogs.** Inline in-palette creation is a later enhancement. |

## 4. Capability Reality (grounding)

The palette's command layer splits into **two tiers** based on what the backend actually
exposes. This is not a choice — it is a constraint that shapes the grammar.

### Tier 1 — Direct REST (verbs that hit an endpoint immediately)

| Action | Endpoint | Notes |
|---|---|---|
| Navigate (page/mode) | — (frontend `router.push`) | |
| Enable/disable skill | `POST /api/skills/{name}/enable\|disable` | |
| Trigger cron job | `POST /api/cron-jobs/:id/trigger` | `cron_jobs.rs:128` |
| Start/stop token-maxing | `POST /api/token-maxing/start\|/stop` | `token_maxing_routes.rs:44,72` |
| Kill agent | `POST /api/agents/:id/kill` | |
| Switch active persona (global) | `POST /api/personas/{id}/activate` | global side-effect, not per-message |

### Tier 2 — Execution via chat WS (no direct "run" endpoint exists)

There is **no** `run skill` / `spawn agent` REST endpoint. Agents are spawned exclusively
via the chat WebSocket → gateway → orchestrator → `AgentRuntime` flow. Skills are prompt
context, not standalone scripts.

Per-message slots that **do** exist on the chat WS payload (`chat.rs:41-45`, `:786-797`):

| Slot | Status | How it flows |
|---|---|---|
| `model` → `model_override` | ✅ **exists** | `chat.rs:135`, `gateway.rs:577`, consumed in `agent_runtime.rs:486` |
| `role` | ✅ **exists** | per-message role-routing key (`setActiveRole`) |
| `project_id` / `mount_ids` | ✅ **exists** | per-message context |

Per-message slots that **do NOT** exist (require new backend work in v2):

| Slot | Status |
|---|---|
| `persona_override` | ❌ absent. Persona is **global** (`persona_api.rs:48 set_active`) or **per-session** (`state_store active_persona_id`), never per-message. |
| `skill_override` | ❌ absent. Skills load from enabled set / session context only. |

**Consequence for `run` verb:**
- **v1** (frontend synthesis) can compose model + role + project + mount per-message.
  It **cannot** pin a persona or skill per-message. Skill pinning is best-effort via a
  composed message token; persona pinning is **deferred to v2** entirely.
- **v2** (RFC) adds `persona_override` and `skill_override` metadata slots, threaded
  `model_override` → `ExecEnv` style through gateway → orchestrator → runtime.

## 5. Command Grammar

Pure deterministic prefix grammar. One prefix char selects the verb; `@` selects an
entity target; remainder is free text (the intent/payload).

```
input     ::= verb? entity? text?
verb      ::= '>'  (run)        | '!'  (control)
            | '~'  (switch)     | '+'  (new)
            | '/'  (capture)
entity    ::= '@' namespaced?
namespaced::= type ':' name       (e.g. @skill:code-audit)
            | name                (bare → disambiguate by type/score)
type      ::= skill | persona | project | agent | cron | mount | mode
action    ::= enable | disable | start | stop    // inline arg for `!` control only
text      ::= <remainder>
bare-text ::= <no verb> → resolved to MODE-PRIMARY verb (deterministic, not NL)
```

### Verb catalog

| Verb | Prefix | Tier | Targets (`@`) | Handler |
|---|---|---|---|---|
| **go** | (implicit / nav match) | 1 | page, mode | `router.push` |
| **capture** | `/` | 1 | inbox, checklist, journal | existing hooks |
| **run** | `>` | 2 | skill, project, (none) — persona via `~` in v1 | compose → chat WS |
| **switch** | `~` | 1 | mode, persona, model | state / REST |
| **control** | `!` | 1 | agent(kill), cron(trigger), skill(enable/disable), maxing(start/stop) | direct POST |
| **new** | `+` | 1 | skill, persona, project, note, cron | route to existing creation form |

### Mode-primary verb (bare text resolution)

Bare text (no prefix) is **deterministic per mode**, not NL-classified:

| Mode | Bare text means |
|---|---|
| Console | `go` — navigate (nav match) or capture fallback (current behavior preserved) |
| Knowledge | `capture` — memo to inbox (current behavior preserved) |
| Chat | `run` — send as message in active session |

Prefix always overrides the mode-primary. `>` is the explicit cross-mode run escape hatch
(use it in Console/Knowledge to start a chat without switching modes).

### Composition examples

| Input | Resolves to |
|---|---|
| `> refactor the auth module` | run, no entity → new chat with intent |
| `> @skill:code-audit` | run skill → chat with skill context (v1: composed message; v2: skill_override) |
| `! @cron:nightly-digest` | trigger cron job |
| `! @agent:abc123` | kill agent |
| `! @skill:legacy disable` | disable skill (verb entity + inline action) |
| `~ @persona:dev` | switch active persona (the only way to pick a persona in v1) |
| `~ @mode:knowledge` | switch to Knowledge mode |
| `+ @skill` | open skill editor (new) |
| `/later 빨래` | capture to Later.md (unchanged) |
| `빨래` (knowledge mode) | capture memo to inbox (unchanged) |
| `빨래` (console mode) | go (nav match) or capture fallback (unchanged) |

### Inline action args (`!` control)

Only the `!` verb accepts an inline action token (`enable|disable|start|stop`) after the
entity: `! @skill:legacy disable`. The Lexer parses this as `action`, not free `text`.
All other verbs treat the remainder as free text (the intent). To avoid parsing ambiguity,
destructive actions (kill/trigger) take **no** action token — the entity alone implies the
verb's default (`! @agent:id` = kill, `! @cron:id` = trigger); toggle-able targets
(skill/maxing) require an explicit `action`.

### Entity disambiguation

A bare `@name` may match multiple types (a skill and a project both called `auth`).
Resolution order: (1) exact `type:name` → no ambiguity; (2) single-type exact match →
use it; (3) multi-type match → render a disambiguation sublist, each item showing its
type badge. This is deterministic and surfacing, not guessing.

## 6. Architecture — Federated Command Registry

Replace the single `slashMode` `useMemo` with a provider pipeline.

```mermaid
flowchart LR
  Q[Query] --> L[Lexer<br/>prefix token + entity + remainder]
  L --> D[Dispatcher<br/>verb + mode]
  D --> R[Provider Registry<br/>mode-filtered]
  R --> P1[NavProvider]
  R --> P2[CaptureProvider]
  R --> P3[RunProvider]
  R --> P4[SwitchProvider]
  R --> P5[ControlProvider]
  R --> P6[NewProvider]
  P1 --> M[Merger + Ranker]
  P2 --> M
  P3 --> M
  P4 --> M
  P5 --> M
  P6 --> M
  M --> O[CommandItem[] + recency]
```

### Core interfaces

```ts
type Verb = 'go' | 'capture' | 'run' | 'switch' | 'control' | 'new'
type SidebarMode = 'console' | 'knowledge' | 'chat'

interface QueryContext {
  raw: string
  verb: Verb | null          // null = bare text
  entity: { type?: string; name: string } | null
  text: string               // remainder
  mode: SidebarMode
}

interface CommandItem {
  id: string
  verb: Verb
  icon: LucideIcon
  titleKey: string           // i18n key
  subtitle?: string          // resolved name, e.g. entity label
  hint?: string              // e.g. "⏎ to run"
  score: number              // provider-computed; ranker only adds boosts
  onSelect: () => Promise<void> | void
}

interface CommandProvider {
  id: string
  verbs: Verb[]              // which verbs this provider answers
  modes: SidebarMode[]       // 'all' shortcut allowed
  resolve(ctx: QueryContext): CommandItem[]
}
```

### Why federated

- **D2 fix:** `CaptureProvider` reads checklist files from the knowledge tree query
  (already cached) instead of `CAPTURE_ROUTES`. New checklists appear automatically.
  *Detection mechanism (must be specified before P1):* the tree (`useKnowledgeTree`,
  `KnowledgeTreeEntry[]`) does **not** flag checklists vs notes today — the current
  `Later.md`/`Read.md` set is hardcoded by filename. Pick one: (a) frontend sniffs a
  `- [ ]` header via a lightweight list-files call; (b) backend adds
  `kind: 'checklist'|'note'` to `KnowledgeTreeEntry`; (c) a naming/frontmatter
  convention. **(a) is the v1 default** (no backend change).
- **Extensibility:** a new subsystem registers a provider; palette core never edits.
- **Testability:** each provider is a pure `(ctx) => items[]` function.

### Provider → verb mapping (v1)

| Provider | Verbs | Data source |
|---|---|---|
| `NavProvider` | go | `consoleNavGroups` (existing) |
| `CaptureProvider` | capture | knowledge checklist tree query (replaces `CAPTURE_ROUTES`) |
| `RunProvider` | run | skills query, projects query (personas excluded — `~` owns them) |
| `SwitchProvider` | switch | personas, modes, engine models |
| `ControlProvider` | control | agents (running), cron jobs, skills, token-maxing status |
| `NewProvider` | new | static creation targets (route to existing forms) |

### cmdk filtering opt-out — implementation prerequisite

The score-based ranker (§7) is **incompatible with cmdk's default behavior** and requires
an explicit opt-out. This is the single most important prerequisite; without it the
provider/ranker model collapses back into the D4 composite-value hack.

- **cmdk defaults to `shouldFilter={true}`** → it filters *and* sorts every item by its
  `value`, hiding non-matches. This is exactly why D4 exists.
- **The current code already intended this opt-out** — `command.tsx:12-14` documents
  "the global palette opts out (`shouldFilter={false}`)" — but **it was never wired**
  (`CommandDialog` at `command.tsx:45` passes no prop; `shouldFilter` grep = 0 usages).
  This is latent defect **D7**. The migration fixes it as a side effect.
- **Required:** set `shouldFilter={false}` on the `<Command>` rendered by `CommandDialog`
  (or thread it through `CommandDialog`). Then:
  1. **Providers own all filtering** — each returns only items matching `ctx`; no other
     filter runs.
  2. **The ranker sorts by `score` and items render in that order.**
  3. **cmdk keyboard nav follows DOM order** when filtering is off, so the sorted order
     *is* the navigated order — no extra wiring.
- `CommandItem.value` becomes a pure a11y/search-string concern (or is dropped); it no
  longer drives ranking.

## 7. Ranking Model

Replaces the composite-`value` hack (D4). Each `CommandItem` carries an explicit `score`.
Providers compute a base score; the ranker applies boosts. **Only viable because cmdk
filtering is disabled** (§6 prerequisite) — otherwise cmdk's own filter/sort overrides
these scores and re-imposes D4.

```
final = base
      + (exactPrefixMatch      ? 100 : 0)
      + (verbExplicitMatch     ?  60 : 0)   // user typed the verb prefix
      + (entityExactName       ?  40 : 0)
      + fuzzyTitle(0..20)
      + modeBoost              // +15 if verb == mode-primary
      + recencyBoost           // 0..25, decays over last N selections
```

- **D3 fix:** mode-primary items get `modeBoost`, so in Knowledge mode a capture intent
  is no longer buried under a coincidental nav match. Explicit verb prefix (`verbExplicitMatch`)
  always wins over bare-text resolution.
- **Recency** (new): track the last ~20 selections keyed by `item.id`; decay-weighted
  boost. Persisted to `localStorage`.

## 8. `run` Verb — Staged Semantics

### v1 — Frontend message synthesis (ships first)

`RunProvider.onSelect` for `> [text]` / `> @entity [text]`:

1. Resolve entity → per-message slot values available **today**: `model`, `role`,
   `project_id`, `mount_ids`.
2. Entity pinning in v1 is **best-effort, non-isolated**:
   - `> @skill:name [intent]` → compose message text `[skill: name] {intent}` (skill must
     be enabled; the agent loads it from context). Surfaced as "composed" in the subtitle.
     If `intent` is empty, the composed text is just the skill invocation token.
   - `> @project:name [intent]` → `setActiveProject` + `setActiveMountIds`, then compose.
     If `intent` is empty, the run is **undefined** → the palette prompts for intent
     rather than sending an empty message.
   - `> @persona` → **not supported in v1** (deferred to v2 — §3). The user switches the
     persona first with `~ @persona`, then runs `>`. No per-message persona slot exists
     until v2, and v1 never exposes raw `@role` (misaligned mental model).
3. Navigate to `/chat`, call `chatStore.sendMessage(composed)`.
4. Close palette.

> v1 is explicitly marked "composed" in the UI subtitle so users know skill pinning is
> message-level, not a true override slot, until v2.

### v2 — Backend override slots (RFC, later)

New RFC (number TBD) adding two per-message metadata slots mirroring `model_override`:

- `persona_override` → `ExecEnv.persona_override` (thread gateway → orchestrator → runtime,
  resolving the persona per-message instead of the global active).
- `skill_override` → `ExecEnv.skill_override` (force-load a skill into the session context
  regardless of enabled set).

Touch points (from `model_override` precedent): `chat.rs` (WS + POST handlers),
`gateway.rs:577`, `orchestrator.rs:514`, `agent_runtime.rs:486`, `ExecEnv` struct.

Once v2 lands, `RunProvider` switches from composed/global to the real slots; the v1
fallback is removed, and `> @persona` becomes a first-class run target.

## 9. UX Details

- **Empty state:** show verb legend (the 5 prefixes) + top recents + mode-primary hint.
  Replaces the current "dump all nav groups" wall.
- **Per-item feedback:** `control` verbs toast on success/failure (consistent with current
  capture toasts). `run`/`switch`/`go` navigate.
- **Keyboard:** `↑↓` navigate, `⏎` select, `Tab` completes an entity/type prefix,
  `Esc` closes, `⌘K` toggles (unchanged). Prefix keys (`>!~/+`) are typed, not hotkeys.
- **Mode detection:** the palette derives the current mode from `pathname`
  (`mode-tabs.tsx:27-31` convention: `/knowledge` → knowledge, `/chat` → chat, else
  console). The store gains a `mode` field set on open and kept in sync.
- **Footer legend:** update to show the 5 verb prefixes + `@` entity + `⏎` + `Esc`.
- **i18n:** all titles via keys; entity *names* are data (not translated). Nav matching
  stays locale-coupled (D5) but is now only one provider among many, lowering its blast
  radius; a future token/alias index can decouple it.

## 10. Migration Path

Incremental — the palette never loses capability during migration.

1. Build registry core (Lexer, Dispatcher, Provider interface, Ranker) behind the feature.
2. Port `NavProvider` + `CaptureProvider` from current code → **behavior parity**, better
   architecture. `CAPTURE_ROUTES` deleted; checklists become data-driven.
3. Add `RunProvider` (v1 synthesis), `SwitchProvider`, `ControlProvider`, `NewProvider`.
4. Wire recency + mode-primary resolution.
5. Delete the old `slashMode` `useMemo` + composite-`value` items.

## 11. Implementation Phases (task breakdown)

| Phase | Deliverable | Verbs live |
|---|---|---|
| **P1** | Registry core + Lexer/Dispatcher/Ranker; `shouldFilter={false}` (D7 fix); `NavProvider` + `CaptureProvider` ported; parity verified | go, capture |
| **P2** | `RunProvider` v1 (frontend synthesis, `@skill`/`@project`/no-entity) | run |
| **P3** | `SwitchProvider` (mode/persona/model) + `ControlProvider` (kill/trigger/enable-disable/maxing) | switch, control |
| **P4** | `NewProvider` (route to existing creation forms) | new |
| **P5** | Recency persistence, empty-state redesign, footer legend, i18n, unit tests for Lexer + each provider | — |
| **P6 (RFC, later)** | `persona_override` + `skill_override` backend slots; migrate RunProvider v1→v2; enable `> @persona` | — |

## 12. Risks & Open Questions

- **R1 — v1 persona side-effect.** ✅ **Resolved:** `@persona` is deferred to v2; v1 `run`
  targets are `@skill`/`@project`/no-entity. Persona switching is `~ @persona` (explicit,
  no hidden global mutation from `run`).
- **R2 — Skill "run" is really "chat with skill context".** Users may expect a discrete
  execution. The composed-message v1 is honest but less crisp. v2 `skill_override` is the
  real fix.
- **R3 — Entity name collisions** across types. Resolved by disambiguation sublist (§5),
  but UX needs validation.
- **R4 — Provider data freshness.** Control/Run providers read live queries (running agents,
  cron jobs). Stale data → stale targets. Mitigation: providers read existing react-query
  caches with their own `staleTime`; acceptable for a palette.
- **R5 — `+` (new) assumes creation UIs exist** for every target (skill/persona/project/
  note/cron). Skill editor and persona/project/cron creation are confirmed; "new note"
  has no dedicated route. **Verify each target's creation entry exists before registering
  it**, else it becomes a dead command.
- **R6 — console bare-text behavior.** ✅ **Resolved:** console bare-text keeps current
  behavior (go nav-match + capture fallback); `run` is gated behind `>` only. Mode-primary:
  console→go, knowledge→capture, chat→run.
- **R7 — `@role` exposure.** ✅ **Resolved:** raw `@role` is never exposed; persona names
  surface only via `~ @persona`. Folded into R1.
- **OQ — `+` (new) creation mode.** ✅ **Resolved:** route to existing forms/dialogs in v1;
  inline in-palette creation is a later enhancement.

## 13. Files Touched (v1, P1–P5)

- `web/src/components/layout/command-palette.tsx` — rewrite to registry host
- `web/src/components/layout/command-palette/` — new: `lexer.ts`, `types.ts`,
  `registry.ts`, `ranker.ts`, providers (`nav.ts`, `capture.ts`, `run.ts`, `switch.ts`,
  `control.ts`, `new.ts`)
- `web/src/components/ui/command.tsx` — set `shouldFilter={false}` on the `Command` inside
  `CommandDialog` (D7 fix)
- `web/src/stores/command-palette.ts` — add `mode`, `recents[]`
- `web/src/i18n/locales/{en,ko}.json` — verb + entity label keys
- `web/src/hooks/use-knowledge.ts` — expose checklist-list query for `CaptureProvider`

P6 (RFC) touches: `chat.rs`, `crates/oxios-gateway/src/gateway.rs`,
`crates/oxios-kernel/src/orchestrator.rs`, `agent_runtime.rs`, `ExecEnv`.
