# UI/UX Audit — Shared Context for Screen Fragments

You are one of several parallel subagents. Each builds `<section>` HTML fragments for assigned screens of the **Oxios** web UI audit. Your fragments get merged into one master report. **Consistency is everything** — reuse the exact CSS classes from the reference file.

## Reference (READ FIRST, mandatory)

`docs/designs/ui-audit/dashboard.html` — the finished Dashboard screen. It is the spec. It contains:
- The full `<style>` block defining every CSS class you may use.
- A complete worked example: rubric bar + before/after comparison mockup + findings list + changes.
- The Oxios mockup aesthetic (sidebar + cards).

**Open it, study its classes, replicate its structure verbatim for your screens.** Do NOT invent new class names unless a screen genuinely needs a unique element — then add it with an inline `<style>` scoped to your fragment.

## Output contract

Write ONE file: `docs/designs/ui-audit/frag-<GROUP>.html` containing **raw `<section>` blocks only** — NO `<!DOCTYPE>`, `<html>`, `<head>`, `<body>`. Just the `<section class="screen" id="screen-...">…</section>` blocks, one per assigned screen, separated by a blank line.

Each screen section must contain, in order:
1. `<div class="screen-head">` — `.num` (e.g. `02 · Console / Agents`), `<h3>` title, `.smeta` with route `<code>` + main source file path, and a `.rubric` (8 `.cell`s, scores 1–5 as bar height % = score×20).
2. `<div class="compare">` with TWO `.panel`s:
   - **Current (annotated)**: `.panel-head` with `<span class="tag cur">현행</span>`, then `.mock-frame` > `.mock.mock-wrap` containing `<span class="pin">` markers (numbered ①②③…) at problem spots + the `.ox` mockup (faithful rebuild). Then a `.findings` list is shared below the compare, NOT inside the panel.
   - **Proposed**: `.panel-head` with `<span class="tag pro">개선</span>`, then `.mock-frame` > `.mock` containing the `.ox.pro` redesign. Then a `.changes` list.
3. After the `.compare`, a `.findings` block (`<h4>발견 사항 …</h4>` + `<ul class="flist">`) with `<li class="high|med|low">` items. Each: `<span class="fno">①</span><div class="ft"><span class="badge …">HIGH|MED|LOW</span> <b>title</b> — problem. <span class="ev">file.tsx:LINE</span></div>`. **Pin count MUST equal finding count.**
4. A `.changes` block (`<h4>개선안 — 무엇이 바뀌는가</h4>` + `<ul>` of `<li><span></span><div><b>label:</b> change.</div></li>`).

**CRITICAL — before/after parity:** both panels must cover the SAME surface. If you critique a region in the current mockup, the proposed mockup must visibly show the fix there. Do not drop regions from the proposed panel.

## Oxios design tokens (light theme — use these exact values)

Mockups use the `.ox` scope which sets these `--ox-*` vars. For inline colors use `oklch(...)` or `var(--ox-*)`:
- bg `oklch(0.99 0 0)` · card `oklch(1 0 0)` · fg `oklch(0.141 0.005 285.823)` · muted `oklch(0.552 0.016 285.938)` · border `oklch(0.92 0.004 286.32)`
- primary(navy) `oklch(0.23 0.025 265)` · accent `oklch(0.967 0.003 265)` · sidebar `oklch(0.978 0.002 265)`
- success `oklch(0.596 0.145 163)` · warning `oklch(0.669 0.162 70)` · error `oklch(0.577 0.245 27.325)` · info `oklch(0.623 0.214 259.815)`
- radius `0.625rem` · font `Geist, system-ui`
Font for Korean labels: just inherit (system handles it).

## Console sidebar template (paste into every Console-screen mockup; toggle `.active` to YOUR screen)

```html
<aside class="sb">
  <div class="brand"><span class="dot"></span>Oxios</div>
  <div class="grp"><div class="gh">Main</div><div class="it <!--active-->"><span class="ic"></span>Dashboard</div></div>
  <div class="grp"><div class="gh">Agents</div><div class="it <!--active-->"><span class="ic"></span>Agents</div><div class="it"><span class="ic"></span>Personas</div><div class="it"><span class="ic"></span>Skills</div></div>
  <div class="grp"><div class="gh">Projects</div><div class="it"><span class="ic"></span>Projects</div><div class="it"><span class="ic"></span>Mounts</div></div>
  <div class="grp"><div class="gh">Storage</div><div class="it"><span class="ic"></span>Memory</div><div class="it"><span class="ic"></span>Workspace</div></div>
  <div class="grp"><div class="gh">Operations</div><div class="it"><span class="ic"></span>Cron</div><div class="it"><span class="ic"></span>Cost</div><div class="it"><span class="ic"></span>Token Maxing</div></div>
  <div class="grp"><div class="gh">Infra</div><div class="it"><span class="ic"></span>MCP</div><div class="it"><span class="ic"></span>Email</div><div class="it"><span class="ic"></span>Git</div></div>
  <div class="grp"><div class="gh">System</div><div class="it"><span class="ic"></span>Resources</div><div class="it"><span class="ic"></span>Security</div></div>
</aside>
```
(Chat/Knowledge/Settings have different sidebar content — read those routes' layout to reproduce accurately. Chat/Knowledge modes switch the sidebar via the route; reproduce whatever the real screen shows.)

## Rubric (8 dimensions, score 1–5, justify each finding with evidence)

1. 정보구조/위계 2. 레이아웃/공간 3. 기능 배치/발견성 4. 내비/웨이파인딩 5. 일관성 6. 인터랙션/어포던스 7. 접근성 8. 타이포/시각품질

## Severity

- **HIGH** — 작업을 막거나 심각히 저해 (blocks/degrades task)
- **MED** — 마찰/혼란 (friction/confusion)
- **LOW** — 폴리시 (polish)

## Hard rules

- **Ground every finding in source you actually read.** Read the route component + its key sub-components (cards, dialogs, toolbars) under `web/src/components/<area>/`. Cite `file.tsx:LINE`.
- Faithful current mockup: reproduce the REAL layout/grid/cards/copy. Don't invent features.
- Proposed = same tokens, evolved. Fix every dimension scoring ≤2 visibly.
- Compact mockups: 3 of N rows + ellipsis, never 50 fake rows.
- Korean for user-facing labels in mockups (per project convention).
- Pin count == finding count. Number pins ①②③④⑤⑥⑦⑧⑨⑩ to match.
- Screen numbering: Dashboard=01. Yours continue from your group's start number (given in assignment).
- Don't manufacture problems — if a screen is genuinely good, score it high and say so with fewer findings.

## Acceptance (self-check before yielding)

- [ ] File is `docs/designs/ui-audit/frag-<GROUP>.html`, raw sections only.
- [ ] Each section has rubric(8) + compare(2 panels, same surface) + findings(pin count==count) + changes.
- [ ] Every finding cites a real `file:line` you read.
- [ ] No `<!DOCTYPE>`/`<html>`/`<head>` wrappers.
- [ ] Sidebar active item matches the screen.
