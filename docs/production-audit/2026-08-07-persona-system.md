# Persona System Audit — 2026-08-07

> **Scope:** Is each persona "like a program" — diverse, robust, well-supported by
> GUI? This audit reads the persona system end-to-end (model → manager → API → web
> UI) against RFC-039 (persona completion) and RFC-044 §8 (persona capability
> packs) and reports what actually works versus what is wired but inert.
>
> **Method:** Source-only, no runtime. Every finding cites a file:line.

---

## TL;DR

The persona system has a **solid backend core** (single source of truth for
`set_active`, injection-resistant LLM security review, 9 well-authored default
personas, schema migration) and a **clean capability-pack design** (RFC-044 §8:
one chat substrate + capability-driven affordances). But the capability packs
are **electrically dead in the browser**: none of the three persona endpoints
serializes `capabilities`, so the UI hook always resolves to an empty set and
*every* capability-gated affordance (diff viewer, fan-out, terminal toggle) is
unreachable. A one-field-per-endpoint fix lights up three already-written
components. Beyond that, two small correctness bugs (schema-version hardcode,
fail-open/security comment mismatch) and some read-only field gaps in the editor.

| # | Finding | Severity | Effort |
|---|---------|----------|--------|
| F1 | `capabilities` omitted by all 3 read endpoints → all capability UI is dead | **P0** | S |
| F2 | `persist()` hardcodes `schema_version: 1`; `persistence.rs` declares `2` | P1 | XS |
| F3 | Inline comment says security review is "fail-closed"; code is fail-open | P2 | XS |
| F4 | Edit dialog cannot edit role / model / traits / capabilities (API supports them) | P2 | S |
| F5 | 5 of 8 declared capabilities have no consumer (longform, outline, …) | P3 | M |
| F6 | `set_active` swallows persist failure → memory/disk divergence (asymmetric) | P3 | XS |

---

## 1. What a persona *is*

`crates/oxios-kernel/src/persona/mod.rs:18-41` — a persona is a fully-formed AI
character, not just a prompt:

```rust
pub struct Persona {
    pub id: String,
    pub name: String,
    pub role: String,
    pub description: String,
    pub system_prompt: String,   // injected into agent sessions
    pub enabled: bool,
    pub model: Option<String>,   // per-persona model override
    pub personality_traits: Vec<String>,
    pub capabilities: Vec<String>, // RFC-044 §8.2 — drives UI affordances
}
```

The "a persona is a program" metaphor is real and is **the** design pillar of
RFC-044 §8.1: *"A 'program' (agent type) **is** a persona… switching persona =
switching agent type."* The full intended definition (§8.2) is
`system_prompt + model + allowed_tools (via security) + capabilities`.

### 1.1 Default roster is genuinely diverse

`mod.rs:109-421` ships **9** default personas, each with a hand-written
philosophy / approach / voice and a non-trivial system prompt (not a one-liner):

| id | role | capabilities declared | character |
|----|------|-----------------------|-----------|
| `dev` | developer | terminal, diff-viewer, approval-cards, worktree-fanout, exec | "ship working code" |
| `review` | qa | diff-viewer, approval-cards | adversarial, evidence-based |
| `research` | researcher | web-search | evidence-first |
| `architect` | architect | — | structure/tradeoffs |
| `mentor` | mentor | — | patient teacher |
| `ops` | sre | exec | failure-aware |
| `security` | security | diff-viewer | threat modeling |
| `writer` | writer | longform-editor, outline | reader-first |
| `planner` | planner | — | outcome-oriented |

These are well-authored. The voices are distinct and the "What You Do NOT Do"
section on each is a meaningful guardrail. This is the strongest part of the
system.

> Note: the roster diverges from RFC-044 §8.3, which proposed `coder` /
> `assistant` / `writer` as the canonical built-ins and listed `dev`/`qa` as
> "(existing, kept)". Implementation instead **expanded** to 9 and never added
> the `coder`/`assistant` ids. Harmless — capabilities, not ids, drive the UI —
> but the RFC and code are out of sync.

---

## 2. Backend core (robust)

### 2.1 Single source of truth for switching — RFC-039

`manager.rs:215-237` (`PersonaManager::set_active`) is the **only** path for
activating a persona. Every caller — HTTP (`persona_routes.rs:261`), the agent
tool (`persona_tool.rs:208`), and the delete-fallback (`persona_routes.rs:211`)
— funnels through it, and it does the full contract in order:

```
slot change → persist() → reseed_callback(intent engine)
```

Because persistence and re-seeding live on the *manager* (not the ephemeral
`PersonaApi` instances the `PersonaTool` spins up, `persona_tool.rs:153`), no
caller can accidentally skip them. This is exactly right and well-documented
(`manager.rs:1-10`).

### 2.2 Activation precedence is deterministic

`apply_config` (`manager.rs:151-170`) resolves the active persona with a clear
priority: StateStore snapshot → `PersonaConfig.default_persona_id` → first
enabled → None. Tested (`manager.rs:311-359`).

### 2.3 Persistence + migration

`persistence.rs`: snapshot under `~/.oxios/state/personas/index.json`, atomic
via `StateStore::durable_write`. Backward-compat v1→v2 is free because
`capabilities` has `#[serde(default)]` (`mod.rs:39`); load rejects out-of-range
schema versions (`persistence.rs:49-56`). Good.

### 2.4 Agent-authored writes are injection-reviewed

`persona_tool.rs:424-493`: when an *agent* creates/updates a persona, the
candidate is framed as **untrusted data** inside `<CONTENT>` tags and judged by
an LLM that is explicitly instructed never to obey the content. An explicit
`safe: false` blocks the write; the user is notified via `KernelEvent`
(`persona_tool.rs:284`). The review runs only when `system_prompt` is being
authored/changed (`persona_tool.rs:308-311`), which is correct — only the prompt
is injected into sessions.

The HTTP path intentionally **skips** this judge (`persona_routes.rs:3-8`,
documented in RFC-039 §3.9): the HTTP API is behind Bearer auth (trusted
caller), the agent tool is not. Reasonable asymmetry.

### 2.5 Concurrency

`parking_lot::RwLock` for the registry and the active slot; `Arc` sharing; store
ops are sync, only `persist` is async. `set_active` takes `&self` (interior
mutability) so it works behind `Arc<PersonaManager>`. `Clone` correctly clones
the callback (`manager.rs:246-259`). Sound.

---

## 3. Findings

### F1 — `capabilities` is never serialized to the client [P0, the big one]

This is the single most important finding and directly answers "is the GUI
well-supported?". **No.** The capability packs are designed, the React
components are written, the gating hook exists — but the wire is cut.

**The break:** three read endpoints all omit `capabilities` from their response:

| Endpoint | Serializer | `capabilities`? |
|----------|------------|-----------------|
| `GET /api/personas` | `PersonaSummary` (`persona_routes.rs:25-32`) | **missing** |
| `GET /api/personas/:id` | inline `json!` (`persona_routes.rs:58-67`) | **missing** |
| `GET /api/personas/active` | inline `json!` (`persona_routes.rs:232-239`) | **missing** |

(The write endpoints *do* accept `capabilities` — `PersonaCreateRequest:85`,
`PersonaUpdateRequest:137` — so the asymmetry is read-side only.)

**The consequence:** `usePersonaCapabilities.ts` builds the capability set from
the roster list (`:68-72`, reads `p.capabilities`) and the active endpoint
(`:116`). Both are always `undefined`, so `capsList` is always `[]` and the
returned `Set` (`:119`) is **always empty**. Therefore every gate is dead:

- `BlockStream.tsx:54` `capabilities.has('diff-viewer')` → always false
  (`InlineDiffViewer` never renders)
- `chat-input.tsx:818` `capabilities.has('worktree-fanout')` → always false
  (`FanOutButton` never renders)
- `chat.tsx:311` `capabilities.has('terminal')` → always false
  (`TerminalToggle` never renders)

The components are not buggy; they are simply unreachable. A user can set the
`dev` persona (which declares five capabilities) and the chat looks identical to
`architect` (which declares none).

**Fix:** add `capabilities` to `PersonaSummary` and the two inline `json!`
blocks. ~6 lines. Lights up three existing, reviewed components immediately. No
frontend change required — the hook already reads the field.

> Why this slipped: the field is `#[serde(default)]` on the Rust struct and
> `Optional` on the TS types, so nothing type-errors or crashes — it silently
> vanishes. A contract test asserting the list endpoint echoes back the
> `capabilities` it accepted on create would catch this regression permanently.

### F2 — `persist()` writes the wrong schema version [P1]

`manager.rs:198-202` hardcodes the snapshot version:

```rust
let snapshot = PersonaSnapshot {
    schema_version: 1,                       // ← hardcoded
    active_persona_id: self.active_persona_id(),
    personas: self.store.list_all(),
};
```

but `persistence.rs:27` declares `const SCHEMA_VERSION: u32 = 2;` and the module
doc (`persistence.rs:6`) says *"Schema (schema_version = 2)"*. The `const` is
never referenced by `persist`.

**Impact today:** none functional — load accepts v1 (`MIN_SCHEMA_VERSION = 1`),
and `capabilities` rides along inside each `Persona` regardless of the snapshot
version. So personas with capabilities round-trip correctly. But the on-disk
file *claims* to be v1 while carrying v2-only data, and any future migration that
branches on `schema_version == 2` will silently take the v1 path. The constant
exists for exactly this reason and isn't used.

**Fix:** `schema_version: crate::persona::persistence::SCHEMA_VERSION`. One line.

### F3 — Security-review comment contradicts the code [P2]

`persona_tool.rs:254` inline comment:
> `// Security review (fail-closed): blocks prompt-injection …`

but the code directly below (`:265-272`, and again at `:346-353` for `update`)
is **fail-open**:

```rust
Err(e) => {
    // Fail-open: a review that can't run must not block legitimate …
    tracing::warn!(…, "… proceeding (fail-open)");
}
```

The function's own doc comment (`:446-447`) correctly documents fail-open. So
the *behavior* is intentional and self-consistent at the function level; only
the inline comment at the call site is wrong. Fail-open here is defensible
(an engine outage shouldn't lock users out of persona creation), but a stale
"fail-closed" comment in a security-sensitive path is exactly the kind of thing
that misleads the next reader into thinking the gate is stronger than it is.

**Fix:** delete/rewrite the `:254` comment. One line. (No behavior change.)

### F4 — The editor exposes 3 of 9 fields [P2]

`edit-persona-dialog.tsx` only edits `name`, `description`, `system_prompt`
(`:99`). The PUT endpoint accepts `role`, `model`, `personality_traits`, and
`capabilities` (`PersonaUpdateRequest:129-138`), and the persona model carries
all of them — but there is **no UI** to set them after creation. In particular
there is no way to set **`capabilities`** through the web UI at all (create
dialog `personas.tsx:127-194` also omits it). So today capabilities can only be
assigned by an agent via the `persona` tool, or by hand-editing `index.json`.

Also note `PersonaItem.capabilities` in the dialog (`edit-persona-dialog.tsx:27`)
and `PersonaDetail` (`:43-51`) are dead — `GET /api/personas/:id` doesn't return
the field (see F1), so these types describe a value that never arrives.

**Fix (with F1):** add `capabilities` to the GET response, render it as editable
multi-select chips in the editor, and surface read-only capability badges on the
roster cards (`personas.tsx:204-257` shows none today).

### F5 — 5 of 8 declared capabilities have no consumer [P3]

Across the default roster, eight distinct capability strings are declared. Three
have a UI consumer; five do not:

| capability | declared by | UI consumer | status |
|------------|-------------|-------------|--------|
| `diff-viewer` | dev, review, security | `InlineDiffViewer` (`BlockStream.tsx:54`) | wired (but dead per F1) |
| `worktree-fanout` | dev | `FanOutButton` (`chat-input.tsx:818`), `AgentFanoutCard` | wired (but dead per F1) |
| `terminal` | dev | `TerminalToggle` (`chat.tsx:311`) | placeholder only ("coming soon", `TerminalToggle.tsx:5-6`) |
| `approval-cards` | dev, review | — | no gate; `ToolApprovalCard` renders for all personas |
| `exec` | dev, ops | — | no consumer |
| `web-search` | research | — | no consumer |
| `longform-editor` | writer | — | not built (RFC-044 §8.4) |
| `outline` | writer | — | not built (RFC-044 §8.4) |

This is acceptable for an evolving system (RFC-044 Phase 3 explicitly defers the
writing pack and the real terminal to later milestones), but it means
**`approval-cards` and `web-search` are declared on personas today and silently
do nothing.** Declared-but-unimplemented flags mislead users about what a
persona changes. Either implement or drop them from the defaults until shipped.

### F6 — `set_active` swallows persist failure [P3]

`manager.rs:227-229`:

```rust
if let Err(e) = self.persist().await {
    tracing::warn!(error = %e, "persona set_active: persist failed");
}
```

The active slot has already been mutated (`:223`), so on a persist IO error the
in-memory active persona and the on-disk one diverge, silently. Contrast the
HTTP create/update/delete handlers, which **propagate** persist errors as `500`
(`persona_routes.rs:113-118`, `173-178`, `215-220`). The asymmetry is
intentional-looking (don't fail a user's persona switch on a flaky disk) but it
means a restart can resurrect the *old* active persona with no signal to the
user beyond a warn log. At minimum the returned prompt/response should note the
persist failed; ideally persist failure should also re-seed-guard.

---

## 4. Verdict

**"Like a program, diverse, robust, GUI-supported?"**

- **Diverse — yes.** 9 distinct, well-authored personas with real system prompts
  and traits. Strong.
- **Robust — mostly yes.** Single-source-of-truth switching, deterministic
  precedence, schema migration, injection-resistant agent-write review, sound
  concurrency. Blemishes: F2 (schema hardcode), F6 (silent persist divergence),
  F3 (misleading comment).
- **GUI-supported — no, today.** The capability-pack architecture (RFC-044 §8)
  is the right design and the components exist, but F1 cuts the wire: the browser
  can never see `capabilities`, so the chat substrate is identical for every
  persona. The roster admin page (`/personas`) can't even *display* capabilities,
  let alone edit them (F4). This is the gap between the design and the shipped
  experience.

The encouraging part: the highest-impact fix (F1) is ~6 lines and immediately
makes three reviewed components live. The backend did its job; the API layer
forgot to forward one field.

---

## 5. Recommended fix order

1. **F1** — add `capabilities` to `PersonaSummary` + both inline `json!` blocks.
   Add a round-trip contract test (create with caps → list → assert caps echo).
2. **F2** — use `SCHEMA_VERSION` in `persist()`.
3. **F3** — fix the `:254` comment.
4. **F4** — render/edit capabilities in the editor + roster cards (post-F1).
5. **F5** — drop `approval-cards`/`web-search` from defaults until they have a
   consumer, or implement the gate.
6. **F6** — decide policy: propagate (consistent with create/update/delete) or
   surface the divergence to the user.
