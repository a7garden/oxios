# App Module Integration — oximemo & oxiline

**Status:** oximemo module **implemented + verified end-to-end** — kernel facade + agent tool + **live web-UI Connect toggle** (`/api/memo/{status,enable,disable}`, runtime RwLock swap, Settings card), green in both feature configs. oxiline module **deferred** — blocked on a cross-repo SQLite version alignment (oxiline-core's rusqlite 0.32 + rusqlite_migration 1.x vs oxios's rusqlite 0.34 conflict at the native `sqlite3` link); see §10.1.
**Scope:** opt-in first-party integration of [oximemo](https://github.com/a7garden/oximemo) and [oxiline](https://github.com/a7garden/oxiline) into oxios

---

## 1. Goal

Let oxios agents manipulate data in two sibling oxi-ecosystem apps — **oximemo**
(card-based memo capture) and **oxiline** ("time as a playhead" routine/day
manager) — directly and reliably, and let context flow both ways.

Two capabilities were chosen explicitly:

- **(A) Agent direct manipulation** — typed Rust API access, no CLI shell-out.
- **(D) Bidirectional context sharing** — app → oxios (context-in) and
  oxios → app (write-back).

## 2. Hard constraints (from the product owner)

| # | Constraint |
|---|---|
| C1 | **Not a backend.** oximemo must NOT become the oxios knowledge base
backend; oxiline must NOT become the calendar/scheduler backend. oxios is a
*co-client* of each app's own canonical store, never the owner. |
| C2 | **Opt-in, default-off.** Nothing is loaded/active unless the user turns
it on. |
| C3 | **Web-UI affordance.** Activation is a "Connect/Enable" action in the
oxios web UI, not an edit to a config file alone. |
| C4 | **Elegance.** Prefer the most graceful structure; reuse established
patterns over inventing new machinery. |

## 3. Key findings (evidence-backed)

### 3.1 Both apps ship embeddable pure-Rust core libraries

- **`oximemo-core`** (v0.6.0) — `Vault` facade: `open`, `create_memo`,
  `get_memo`, `update`, `delete`, `restore`, `purge`, `list`, `search_memos`,
  `export_*`, `reindex`, `migrate`, `doctor`.
- **`oxiline-core`** (v0.2.0) — typed CRUD over a `&Connection`:
  `open_and_migrate`, `record`, `plan`, `activities`, `categories`, `settings`;
  stable `ErrorCode`; all domain types derive `specta::Type`.

Each CLI and each Tauri app already wraps the same core facade — oxios becomes
a third consumer of the same library.

### 3.2 Both cores are concurrency-safe for the co-client model

This was the principal technical risk; it is resolved.

**oximemo** — explicitly designed for cross-process co-existence
(`crates/oximemo-core/src/lock.rs`, `vault.rs` §Concurrency model):

- Read ops take a **shared** `fs2` flock; write ops take an **exclusive** flock.
- redb + tantivy are opened **transiently within the lock scope** — no process
  holds them open across the boundary, so two processes never collide on
  redb/tantivy's own single-writer locks.
- Memo files are **never locked** (atomic rename); external editors/agents read
  and write them freely.
- The documented first-class use case is *"running `oximemo …` while the GUI is
  running."* oxios embedding the core is exactly that path.

**oxiline** — single SQLite DB in **WAL** mode (multi-reader, serialized single
writer; `busy_timeout` + `journal_mode=WAL`). oxios opens its own pool to the
same file; correctness is guaranteed by SQLite. The only gap is live event
propagation across the process boundary (see §7.2).

### 3.3 crates.io prerequisite

oxios publishes its own crates to crates.io (topological order, 8 crates).
`oxios-kernel` therefore cannot take a path/git dependency — any dependency
must be on crates.io.

**None of `oximemo-core`, `oxiline-core`, `oximemo`, `oxiline` are currently on
crates.io** (verified against the crates.io API, 2026-08-05).

**Prerequisite:** publish `oximemo-core` and `oxiline-core` to crates.io before
this integration can ship in a publishable oxios build. During local
development the sibling repos at `../oximemo`, `../oxiline` are referenced by
relative path; the publish step swaps those to crates.io versions (mirrors how
the project already uses `[patch.crates-io]` for local overrides).

## 4. Design decision: reuse the calendar/email pattern, do not invent a framework

oxios already integrates first-party domains with a proven, consistent shape:

- **`CalendarApi`** — `Arc<CalendarEngine>` facade, publishes `KernelEvent`
  variants, stored as `calendar: Option<CalendarApi>` on `KernelHandle`
  (`kernel_handle/calendar_api.rs`, `mod.rs:162`).
- **`CalendarTool`** — registered *only* when present via
  `CalendarTool::try_from_kernel(kernel) → Option<CalendarTool>`
  (`tools/builtin/mod.rs:118-121`).
- **`EmailApi`** — runtime-swappable `Arc<RwLock<Option<EmailApi>>>` activated from the web UI without a restart.
- Activation is a config section (`[calendar]`) + a settings UI toggle.

This design applies **exactly that pattern** to two more first-party domains.
Concretely, we deliberately **do not** build:

| Considered | Decision | Why |
|---|---|---|
| An `AppModule` trait + registry | **Rejected (YAGNI)** | Two modules; the kernel is intentionally monolithic. A trait abstracts over a population of one-or-two. |
| `IntegrationKind::AppModule` (HostToolsApi extension) | **Rejected** | HostToolsApi models *external CLI detection + install + credentials*. Our modules are first-party data domains, not PATH-detectable CLIs. Adding a variant would overload a different concept. |
| A runtime UI plugin system | **Rejected** | oxios has none and doesn't need one for first-party domains compiled into the binary. |

The "App Module" concept thus **collapses into "two more calendar/email-style
domain integrations, feature-gated because they are external dependencies."**
This is the most elegant outcome: zero new abstractions, full consistency with
existing code.

## 5. Architecture

### 5.1 Two-level gating

| Level | Mechanism | Purpose |
|---|---|---|
| **Compile-time** | Cargo feature `memo` / `timeline` (default-off) on `oxios` → `oxios-kernel` | Users who don't use the app don't pay the build cost (`memo` pulls `redb` + `tantivy`; `timeline` reuses `rusqlite` already present). |
| **Runtime** | Config section `[memo]` / `[timeline]` with `enabled` + store path; `Option<MemoApi>` / `Option<TimelineApi>` on `KernelHandle` | Default-off activation, no rebuild to toggle. Mirrors `[calendar]`. |

A build *without* `--features memo,timeline` compiles exactly as today — no new
types, no new tools, no config keys parsed.

### 5.2 Components (per module, calendar/email-isomorphic)

```
oxios-kernel
├── kernel_handle/
│   ├── memo_api.rs         # MemoApi  { vault: Arc<oximemo_core::Vault>, event_bus }  — CalendarApi analog
│   └── timeline_api.rs     # TimelineApi { pool, event_bus }                          — CalendarApi analog
├── tools/builtin/
│   ├── memo_tool.rs        # MemoTool::try_from_kernel(kernel) -> Option<MemoTool>    — CalendarTool analog
│   └── timeline_tool.rs    # TimelineTool::try_from_kernel(kernel) -> Option<...>
├── kernel_handle/mod.rs    # + memo: Option<MemoApi>, timeline: Option<TimelineApi>
├── tools/builtin/mod.rs    # register_all_kernel_tools: conditional register like Calendar (118-121)
└── event_bus.rs            # + MemoCreated/Updated/Deleted, TimelineRecordChanged events
src/
├── kernel.rs               # assemble MemoApi/TimelineApi from config (build_calendar_api analog)
└── api/routes/             # + /api/memo/*, /api/timeline/* (status/enable/disable) — settings-style
share/default-config.toml   # + [memo], [timeline] sections (commented/disabled by default)
web/src/                    # settings toggles (calendar/email analog); v1 has no full portal panel
```

### 5.3 Dependency wiring

`oxios-kernel/Cargo.toml`:
```toml
oximemo-core = { version = "0.6", optional = true }
oxiline-core = { version = "0.2", optional = true }

[features]
memo     = ["dep:oximemo-core"]
timeline = ["dep:oxiline-core"]
```
Binary `oxios/Cargo.toml` forwards the features:
```toml
memo     = ["oxios-kernel/memo"]
timeline = ["oxios-kernel/timeline"]
```
Local dev (pre-publish): relative path overrides via `[patch.crates-io]`.

## 6. Per-module detail

### 6.1 oximemo — `MemoApi`

```rust
pub struct MemoApi {
    vault: Arc<oximemo_core::Vault>,
    event_bus: Option<EventBus>,
}
```
- Constructed in `kernel.rs` when `[memo].enabled` and the vault path resolves.
- Every method delegates to `Vault` (which takes its own per-op shared/exclusive
  flock) and publishes `MemoCreated/Updated/Deleted` on success.
- The agent-facing `MemoTool` exposes: `create`, `get`, `update`, `delete`,
  `list`, `search` (full-text), `categories`. Parameter schema follows
  `CalendarTool`'s action-based dispatch.
- **Context-in (D):** `search` makes memos a queryable context source for the
  agent — an *additional lens*, never the knowledge backend (C1).

### 6.2 oxiline — `TimelineApi`

```rust
pub struct TimelineApi {
    pool: Arc<r2d2_pool>,            // same WAL DB the app uses
    event_bus: Option<EventBus>,
}
```
- Constructed in `kernel.rs` when `[timeline].enabled` and the DB path resolves;
  uses `oxiline_core::open_and_migrate`.
- Methods delegate to `oxiline_core` record/plan/activity functions over a
  pooled connection; publishes `TimelineRecordChanged`.
- The agent-facing `TimelineTool` exposes: `now` (current activity/plan),
  `record` (start/stop/state), `list_records`, `plan` (view/options), `report`.
- **Context-in (D):** `now` lets the scheduler / token-maxing mode read the
  current focus block as scheduling context.

## 7. Bidirectional flows (D)

### 7.1 Context-in (core of v1)

| Flow | Mechanism | Status |
|---|---|---|
| oximemo → oxios | `MemoTool.search` — agent queries memos as context | **v1 core** |
| oxiline → oxios | `TimelineTool.now` — scheduler reads current focus/routine | **v1 core** |

### 7.2 Cross-process change propagation

The app and oxios share one store, so a write by either is immediately visible
to the other on next read. *Live* notification (UI refresh without a poll) is
the only gap:

- **oxiline** emits in-process `oxiline://db-changed` events that do not cross
  the process boundary. Bridge: register a SQLite `update_hook` on oxios's pool
  connections → publish `TimelineRecordChanged` → web UI / scheduler refresh.
- **oximemo** has a `notify` file watcher (`Vault::open` watcher, `OnChange`).
  oxios subscribes the same watcher on its `Vault` handle → republish as
  `MemoCreated/Updated/Deleted` (best-effort; a re-scan reconciles).

### 7.3 Write-back (opt-in hooks)

| Flow | Mechanism | Status |
|---|---|---|
| oxios → oximemo | Opt-in `PersistenceHook` analog: after an agent run, save a summary memo via `Vault::create_memo` | **v1 opt-in hook** (off by default; C2) |
| oxios → oxiline | Opt-in: log an agent run as a time record via `record` | **v1 opt-in hook** (off by default; C2) |

Write-back is strictly opt-in and per-session — it never silently mutates the
user's apps.

## 8. Activation lifecycle

```
 ┌─────────┐  build with --features memo   ┌──────────┐  [memo].enabled=true + path  ┌─────────┐
 │ absent  │ ───────────────────────────▶ │ dormant  │ ────────────────────────────▶│ active  │
 │ (no code)│                              │ (compiled,│                              │ (facade,│
 └─────────┘                               │ Option=None)│                           │ tools)  │
                                           └──────────┘ ◀──────────────────────────── └─────────┘
                                                ▲              [memo].enabled=false / disconnect
                                                └──────────────────────────────────┘
```

1. **Absent** — feature off: zero impact, identical to today.
2. **Dormant** — feature on, config off: types compiled, `MemoApi = None`, no
   tool registered, no panel.
3. **Active** — feature on, config on (toggled from web UI): `kernel.rs`
   constructs the facade from the resolved store path, `try_from_kernel`
   returns `Some`, the tool is registered, events flow.
4. **Disconnect** — config off: facade dropped, tool unregistered. **Data is
   never touched** — oxios is a co-client, not the owner (C1).

## 9. Web UI affordance (v1)

Reuse the existing **Settings** surface (calendar/email analog), *not* a new
portal panel:

- A "Memo" / "Timeline" card under Settings → domain-integrations section
  (alongside Calendar/Email — *not* the HostToolsApi external-CLI registry).
- Card shows: detected app store path, connection status, an **Enable/Connect**
  toggle. Toggling hits `/api/memo/enable` (or `…/disable`) — the runtime-swap
  pattern (`RwLock<Option<Api>>`) like Email, so no daemon restart.
- A full *browsing* panel (Memos list, Timeline view inside the oxios portal)
  is explicitly **out of scope for v1** (the apps already ship their own UIs;
  the user did not select dashboard capability B). The activation gate + API
  make adding it later a straightforward, additive change.

## 10. Prerequisites & phasing

**Prerequisite (blocking publish):** publish `oximemo-core` and `oxiline-core`
to crates.io at their current workspace versions (0.6.0 / 0.2.0).

**Prerequisite (blocks oxiline only):** SQLite version alignment. oxiline-core
depends on `rusqlite 0.32` + `rusqlite_migration 1.x`; oxios-kernel uses
`rusqlite 0.34` (constrained by `sqlite-vec` + the memory system). Both pull
`libsqlite3-sys` with the native `links = "sqlite3"` — Cargo allows only one —
so they cannot coexist. Unblocking oxiline requires either bumping oxios's
rusqlite (re-validating the memory system + `sqlite-vec`) or reworking oxiline's
migration story onto rusqlite 0.34. (oximemo uses redb+tantivy, no SQLite —
unaffected, hence shipped.)

**Implementation phases:**

1. ✅ **Framework skeleton** — `memo` feature, `MemoConfig`, live runtime slot
   (`KernelHandle.memo: Arc<RwLock<Option<Arc<MemoApi>>>>`, `with_memo` builder),
   `try_from_kernel`, boot assembly (`build_memo_api`), `KernelEvent::MemoCreated/Deleted`.
2. ✅ **oximemo module** — `MemoApi` (create/get/list/search/delete over `Vault`,
   `spawn_blocking` + event publish) + `MemoTool` (AgentTool, live-slot aware).
   Verified: round-trip test passes, 826 lib tests pass, clippy `-D warnings` clean.
3. ⏸ **oxiline module** — *deferred* on the SQLite-alignment prerequisite above.
4. ⏸ **Write-back hooks** — opt-in `PersistenceHook` analogs (deferred; v1 ships
   context-in only, per the chosen D scope).
5. ✅ **Web UI Connect** — `/api/memo/{status,enable,disable}` (cfg-gated; live swap,
   no restart) + `MemoSectionCard` in Settings (404-tolerant when feature absent).
   Registered across all 4 nav sources + en/ko i18n; settings-consistency 9/9,
   typecheck/biome/build clean.
6. ✅ **Verify** — `cargo check/clippy` (default + `--features memo`), `cargo test`,
   `cargo fmt --check`, web typecheck + vitest + vite build — all green.

## 11. Out of scope (v1)

- Full portal data-browsing panels (Memos/Timeline views inside oxios web) —
  the apps have their own UIs; capability B was not selected.
- An MCP-server exposure of the apps (oximemo's planned v0.3 MCP is orthogonal;
  that path serves *third-party* agents, this design serves oxios's own).
- A generic third-party plugin/runtime-load system — first-party modules are
  compiled in by design.
- oximemo/oxiline becoming backends for oxios knowledge or calendar (C1).

## 12. Risks

| Risk | Mitigation |
|---|---|
| redb/tantivy build cost on `memo` feature | Default-off feature gate; only opt-in builds pay it. |
| Version drift between oxios and the core crates | Pin to compatible semver ranges; the ecosystem is single-maintainer so drift is controllable. |
| oximemo flock timeout under heavy concurrent write (5s `LOCK_TIMEOUT`) | Acceptable for a personal agent OS; per-op locking keeps hold times short. Monitor. |
| `update_hook`/watcher best-effort sync misses a change | Stores are shared, so a re-scan always reconciles; events are an optimization for live UI, not a correctness path. |
| crates.io publish of two more crates | Mechanical; their `[lib]` crates are already self-contained. |
