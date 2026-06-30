---
name: skill-creator
description: Create new skills, edit and improve existing ones, and benchmark them with/without-skill evaluation. Use this whenever the user wants to build, author, draft, edit, refine, package, ship, or test a skill — even if they only say "turn this into a skill" or "make oxios remember how to do X". Also use it to validate a skill's structure, package a skill into a .skill archive, or run an evaluation to measure whether a skill actually helps.
---

# Skill Creator

A skill for creating new skills and iteratively improving them. Adapted from Anthropic's skill-creator for the Oxios agent OS, where skills follow the same structure and the deterministic operations are native Rust (`skill_forge` tool) instead of Python scripts.

The agent using this skill has two engines available:

- **`skill_forge` tool** — deterministic operations: `create`, `write`, `validate`, `package`, `import`, `list`, `get`, `benchmark`, `view`. Always available, no external runtime.
- **subagent runs** — spawn child agent runs (with the skill, and without it as a baseline) to generate test outputs and to grade them. The grader/improver prompts live in this skill's `agents/` folder.

## How to use this skill

Figure out where the user is, then help them progress:

- **"I want a skill for X"** → capture intent, interview, draft, test, iterate.
- **"Here's a skill / a workflow, make it a skill"** → go straight to drafting or improving.
- **"Is this skill any good? / does it actually help?"** → run an evaluation.
- **"It's not triggering"** → improve the description (see [Optimizing the trigger](#optimizing-the-trigger)).

Be flexible. If the user just wants to vibe and not run benchmarks, do that instead.

---

## Anatomy of a skill

```
skill-name/
├── SKILL.md            (required) YAML frontmatter + markdown instructions
├── references/         (optional) docs loaded into context on demand
├── scripts/            (optional) executable code for deterministic tasks
├── assets/             (optional) files used in output (templates, icons)
└── evals/              (optional) test cases — NOT shipped in the .skill archive
    └── evals.json
```

### Progressive disclosure (three levels)

Skills load in three tiers — design for this:

1. **Metadata** (`name` + `description`) — always in context (~100 words). This is the **primary trigger**. The model decides whether to activate the skill from this alone.
2. **SKILL.md body** — loaded when the skill triggers. Keep it **under ~500 lines**. If you approach the limit, push detail into `references/` and point to it.
3. **Bundled resources** (`references/`, `scripts/`, `assets/`) — read on demand, by their absolute path under the skill directory. Unlimited; scripts can run without being loaded into context.

The `skill_forge` tool always carries the essentials in its own description, so basic authoring works even before this skill body is loaded. This body is the deeper reference.

---

## Creating a skill

### 1. Capture intent

If the conversation already contains a workflow the user wants captured ("turn this into a skill"), extract from history first: the tools used, the sequence of steps, the corrections the user made, the input/output formats. Then confirm:

1. What should this skill enable the agent to do?
2. When should it trigger? (which user phrases / contexts)
3. What's the expected output format?
4. Should we set up test cases? Skills with objectively verifiable outputs (file transforms, data extraction, code generation, fixed workflows) benefit from test cases. Subjective skills (writing style, art) usually don't. Suggest the right default, let the user decide.

### 2. Interview and research

Proactively ask about edge cases, input/output formats, example files, success criteria, dependencies. Don't write test prompts until this is settled. Research in parallel via subagents where useful — come prepared so the user does less work.

### 3. Write the SKILL.md

Use `skill_forge` `write` (rich frontmatter preserved) or `create` (frontmatter synthesized). Fill in:

- **`name`** — skill identifier, lowercase-hyphens, must equal the skill directory name.
- **`description`** — when to trigger AND what it does. This is the primary trigger mechanism; all "when to use" info lives here, not in the body. Lean a little **pushy**: name the contexts where it should fire, including ones where the user doesn't explicitly say the word. (Agents tend to under-trigger.)
- the body — imperative instructions, examples, output templates.

#### Writing style

- Prefer the imperative form.
- Explain *why* things matter rather than stacking MUSTs; use theory of mind, keep it general.
- Define output formats with explicit templates. Include realistic examples.
- Draft it, then re-read with fresh eyes and tighten.

Then **always validate**: call `skill_forge` `validate` on the skill name. It checks frontmatter parses, `name`+`description` are present, the name is valid and matches the directory, and flags a body over 500 lines or a thin description. Fix errors before shipping.

---

## Packaging and shipping

Once a skill is validated, call `skill_forge` `package` to produce a distributable `.skill` zip. The archive excludes `evals/`, `__pycache__`, `node_modules`, `.git`, `.pyc`, `.DS_Store` — test cases and build artifacts do not ship. The skill folder is the top-level entry, so extracting lands at `<name>/...` and can be re-imported with `skill_forge` `import` (raw text) or the web UI's archive upload.

---

## Running an evaluation

Use this to measure whether a skill actually helps, or to compare a revised skill against the previous version. Skip it for purely subjective skills or when the user just wants a quick draft.

This is one continuous sequence — don't stop partway through.

### Step 1 — Write test cases

Save them to `evals/evals.json` inside the skill directory. Start with 2–3 realistic prompts a real user would actually say. Don't write assertions yet — just prompts. See `references/schemas.md` for the full `evals.json` schema (the `expectations` field is added in the next step).

Share them with the user: "Here are the test cases I'd like to try — do these look right, or want to add more?"

### Step 2 — Spawn all runs in the same turn

For each test case, spawn **two** child agent runs at once — one **with** the skill, one **without** (baseline). Launch them all in parallel so they finish together.

- **With-skill run:** give the child the skill path and the eval prompt; have it save the outputs the user cares about to `<workspace>/iteration-N/<eval-name>/with_skill/outputs/`.
- **Baseline run:** same prompt, no skill (or, when *improving* an existing skill, the old version — snapshot it first). Save to `without_skill/outputs/` (or `old_skill/outputs/`).

Write an `eval_metadata.json` per test case (assertions can be empty for now):

```json
{ "eval_id": 0, "eval_name": "descriptive-name", "prompt": "...", "assertions": [] }
```

### Step 3 — Draft assertions while runs are in progress

Don't idle. Draft objectively-verifiable assertions per test case and explain them to the user. Good assertions read clearly in the viewer — someone glancing should instantly understand each check. Don't force assertions onto subjective outcomes. Update `eval_metadata.json` and `evals/evals.json` with them.

### Step 4 — Capture timing

When each child run finishes, its completion notification carries `total_tokens` and `duration_ms`. Save them immediately to `timing.json` in the run directory (this data is not persisted elsewhere):

```json
{ "total_tokens": 84852, "duration_ms": 23332, "total_duration_seconds": 23.3 }
```

### Step 5 — Grade each run

Spawn a **grader** subagent for each run. Hand it the prompt from `agents/grader.md` (read that file — it defines the grading process, PASS/FAIL criteria, evidence requirements, and the exact `grading.json` output schema). The grader writes `grading.json` next to the run's outputs. For assertions checkable by a script, write and run the script instead of eyeballing — faster and reusable.

### Step 6 — Benchmark and review

Call `skill_forge` `benchmark` with the iteration directory. It aggregates every `grading.json` + `timing.json` into `benchmark.json` and `benchmark.md` — pass-rate / time / tokens per configuration as mean ± stddev, plus the with_skill−baseline delta and analyst notes (non-discriminating assertions, high-spread evals). See `references/schemas.md` for the `benchmark.json` schema.

Then call `skill_forge` `view` to generate a self-contained static HTML review of the iteration. Open it (or hand the path to the user). Two tabs: **Outputs** — each test case's prompt, produced files, formal grades, and a feedback textbox; **Benchmark** — the quantitative comparison. The **Download feedback** button serializes the textareas to `feedback.json`.

Tell the user: "I've generated the review — open the Outputs tab to click through each case and leave feedback, and the Benchmark tab for the numbers. Come back when you're done."

### Step 7 — Read feedback and iterate

Read `feedback.json`. Empty feedback means the case was fine; focus improvements on cases with specific complaints. Rewrite the skill, snapshot the old version as the next baseline, and repeat. For deeper pattern analysis across runs, use the `agents/analyzer.md` prompt; to compare two skill versions head-to-head, use `agents/comparator.md`.

---

## Optimizing the trigger

If a skill works well but doesn't *fire* when it should, the description is the lever. Draft alternative descriptions and re-run the evaluation against a set of prompts that *should* trigger it and a set that *shouldn't*. Pick the description that maximizes correct triggers and minimizes false ones. Keep it specific about contexts — vague descriptions under-trigger.

---

## Recap of the `skill_forge` actions

| Action | When |
|---|---|
| `create` | Scaffold a new skill (frontmatter synthesized from name+description). |
| `write` | Author rich SKILL.md verbatim (frontmatter preserved). |
| `validate` | Check structure before shipping — name, description, dir match, body size. |
| `package` | Export a distributable `.skill` zip. |
| `import` | Import a skill from raw SKILL.md text. |
| `list` / `get` / `delete` / `enable` / `disable` | Manage installed skills. |
| `benchmark` | Aggregate a graded iteration into benchmark.json + benchmark.md. |
| `view` | Generate a static HTML review of an iteration. |

The bundled `references/schemas.md` defines every JSON shape (`evals.json`, `grading.json`, `timing.json`, `benchmark.json`, `history.json`, `metrics.json`). The `agents/` folder holds the grader, analyzer, and comparator subagent prompts.
