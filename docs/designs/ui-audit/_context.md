# UI/UX Audit — Shared Context (DO NOT EDIT except by main agent)

This file is the single source of truth for every parallel screen-batch subagent.
Read it fully. Then read the **reference worked example**: `oxios-ui-audit.html`
section `#screen-01` (대시보드) — it shows the exact structure, class usage, pin
placement, finding format, and visual fidelity you must match.

All output goes to **`frag-<group>.html`** as RAW `<section class="screen">…`
blocks. NO `<!DOCTYPE>`, `<html>`, `<head>`, or `<style>` tags. You reuse the
master stylesheet's classes. If you need something bespoke, use INLINE
`style="…"` only — NEVER a bare `<style>` block (it leaks globally on merge).

---

## 1. Route → source file map

Read the route component AND its direct child components (cards/dialogs/tables)
under `web/src/components/<area>/`. Cite `file:line` from the REAL source you read.

| # | 화면 | route file | key children (read these) |
|---|---|---|---|
| 02 | 에이전트 | `routes/agents/index.tsx` | `components/agent-monitor/{monitor-canvas,monitor-node,detail-panel}.tsx` |
| 03 | 페르소나 | `routes/personas.tsx` | `components/persona/edit-persona-dialog.tsx` |
| 04 | 스킬 | `routes/skills.tsx` | `components/skills/{skill-detail,skill-content,skill-summary-pill,import-dialog,skill-editor-dialog}.tsx` |
| 05 | 프로젝트 | `routes/projects/index.tsx` | `components/project/{project-card,create-project-dialog,edit-project-dialog}.tsx` |
| 06 | 마운트 | `routes/mounts/index.tsx` | `components/mount/edit-mount-dialog.tsx` |
| 07 | 메모리 | `routes/memory.tsx`* | `components/memory/{memory-overview,memory-search}.tsx` |
| 08 | 워크스페이스 | `routes/workspace.tsx`* | (file-manager style) |
| 09 | 크론 작업 | `routes/cron-jobs.tsx` | `components/cron/{cron-timeline-view,cron-schedule-editor,edit-cron-dialog}.tsx` |
| 10 | 비용 | `routes/budget.tsx` | `components/budget/{budget-management,set-budget-dialog,agent-budget-card}.tsx` + `components/cost/*.tsx` |
| 11 | 토큰 맥싱 | `routes/token-maxing.tsx` | `components/token-maxing/{sessions,controls,provider-cards,status-header,billing-badge}.tsx` |
| 12 | MCP 서버 | `routes/mcp.tsx`* | `components/mcp/{server-list,server-card,edit-server-dialog}.tsx` |
| 13 | 이메일 | `routes/email.tsx` | (email config/routing) |
| 14 | Git | `routes/git.tsx` | (git config) |
| 15 | 리소스 | `routes/resources.tsx`* | (system resources/charts) |
| 16 | 보안 | `routes/security.tsx` | `components/dashboard/approvals-queue.tsx`, security/approval flow |
| 17 | 채팅 | `routes/chat.tsx` | `components/chat/{chat-input,message-bubble,model-picker,empty-chat-state,live-activity-bar}.tsx` |
| 18 | 지식 | `routes/knowledge/index.tsx` | `components/knowledge/{split-editor,editor-panel,editor-toolbar,info-panel,search-modal}.tsx` |
| 19 | 설정 | `routes/settings.tsx` | `components/settings/{settings-shell,field-defs,section-icons,secrets-section,exec-allowlist-editor}.tsx` |
| 20 | 마켓플레이스 | `routes/marketplace.tsx`* | (skill/extension marketplace cards) |

`*` = route file may live directly in `routes/` (e.g. `routes/memory.tsx`) or be
auto-generated; if a path is wrong, `glob web/src/routes/*` and find it. The
subagent must read the REAL file — never guess its content. If a route is tiny
(a thin wrapper delegating to one component), read that component.

---

## 2. Design tokens (Oxios Web — light theme, OKLCH)

All mockups render the LIGHT theme. Use these as CSS values inside `.frame`:

```
--bg:oklch(0.99 0 0)        /* page bg, near-white */
--card:oklch(1 0 0)         /* card surface, pure white */
--fg:oklch(0.141 0.005 285.823)   /* primary text, near-black */
--fg2:oklch(0.552 0.016 285.938)  /* muted-foreground, gray */
--pri:oklch(0.23 0.025 265)       /* primary, dark blue */
--pri2:oklch(0.985 0 0)           /* primary-foreground, near-white */
--mut:oklch(0.967 0.001 286.375)  /* muted bg */
--acc:oklch(0.967 0.003 265)      /* accent bg */
--bd:oklch(0.92 0.004 286.32)     /* border, light gray */
--sb:oklch(0.978 0.002 265)       /* sidebar bg */
--sba:oklch(0.967 0.003 265)      /* sidebar-accent (active item bg) */
--suc:oklch(0.596 0.145 163)  --war:oklch(0.669 0.162 70)
--err:oklch(0.577 0.245 27.325)   --info:oklch(0.623 0.214 259.815)
```
- **Fonts**: Geist (sans), Geist Mono (mono). Mockup body already sets Geist.
- **Radius**: base 0.625rem → cards `border-radius:12px`, pills `99px`, items `7-8px`.
- **Shadow**: `0 1px 2px rgba(0,0,0,.04)` on cards.

---

## 3. CSS class catalog (master stylesheet — REUSE, do not redefine)

### Report structure
```
<section class="screen" id="screen-NN">          scroll anchor
  <div class="screen-head">                       title + rubric
    <div class="snum">NN</div>
    <div class="stitle"><h2>제목</h2><span class="route">/ · routes/x.tsx</span></div>
    <div class="rubric"> 8× <div class="rcell sSCORE"><span class="rn">N</span><span class="rl">한글명</span></div> </div>
  </div>
  <div class="compare">                           two panels
    <div><div class="panel-cap cur"><span class="dot"></span>현재 (주석)</div> <div class="frame">…</div></div>
    <div><div class="panel-cap prop"><span class="dot"></span>제안 (같은 토큰, 개선)</div> <div class="frame">…</div></div>
  </div>
  <div class="findings"><h3>발견 (Findings)</h3><ul class="flist"> … </ul></div>
  <div class="changes"><h3>변경 제안</h3><ul> … </ul></div>
</section>
```
Rubric cells: `.rcell.s5`(green) `.s4`(lt-green) `.s3`(amber) `.s2`(red-lt) `.s1`(red). Score 1–5.

Rubric labels (Korean, in this order):
`정보구조 · 레이아웃 · 기능배치 · 내비게이션 · 일관성 · 인터랙션 · 접근성 · 타이포`

### Finding item
```
<li class="fitem high|med|low"><span class="sev high|med|low">HIGH|MED|LOW</span><div>
  <p class="ft">제목 (한 줄)</p>
  <p class="fd">설명 — 무엇이 왜 문제인지. 구체적 요소명.</p>
  <p class="src">routes/x.tsx:LINE · component/y.tsx:LINE</p>
</div></li>
```

### App mockup classes (inside `.frame > .os`)
Shell: `.os`(flex row, h:560) · `.sb`(sidebar 188px) · `.main`(flex:1) · `.bar`(topbar 44px) · `.page`(scroll content)
Sidebar: `.sb-brand` · `.sb-mode>(.mt|.mt.on)` · `.sb-sep` · `.sb-nav` · `.sb-grp`(label) · `.sb-it|.sb-it.on>(.ic+label)` · `.sb-foot`
Topbar: `.bar .bm|.bm.on` · `.btn-sm` · `.kbd` · `.clock`
Cards: `.card` · `.card-h>(.card-t+icon)` · `.card-body` · grids `.g6 .g5 .g4 .g3 .g2`
KPI: `.kpi-val` · `.kpi-delta` · `.kpi-hint` · `.kpi-ic` · `.spark`
Lists/tables: `.row>(.ava+ .grow>(.nm+.ds) +.badge|.dot-s)` · `.tbl` · `.badge.run|.idle|.err`
Inputs/btns: `.inp` · `.btn|.btn.pri|.btn.des`
Helpers: `.flex .between .center .col .gap4/6/8/10 .mut .mono .xs .sm .bold .sep .tag .grow .chip .pill(.pri|.mut)`
Color text: `.tc-suc .tc-war .tc-err .tc-info .tc-pri .tc-mut`
Pins: `<span class="pin" style="top:Ypx;right/left:Xpx">N</span>` — absolute, parent needs `position:relative`. Number = finding index (1-based).

### Sidebar template (console mode) — paste into BOTH panels, set ONE `.sb-it.on`
```html
<div class="sb">
  <div class="sb-brand"><span class="zk">⚡</span>Oxios</div>
  <div class="sb-mode"><span class="mt on">Console</span><span class="mt">Knowledge</span><span class="mt">Chat</span></div>
  <div class="sb-nav">
    <div class="sb-grp">메인</div>
    <div class="sb-it"><span class="ic" style="background:#bbb"></span>대시보드</div>
    <div class="sb-grp">에이전트</div>
    <div class="sb-it"><span class="ic" style="background:#bbb"></span>에이전트</div>
    <div class="sb-it"><span class="ic" style="background:#bbb"></span>페르소나</div>
    <div class="sb-it"><span class="ic" style="background:#bbb"></span>스킬</div>
    <div class="sb-grp">프로젝트</div>
    <div class="sb-it"><span class="ic" style="background:#bbb"></span>프로젝트</div>
    <div class="sb-it"><span class="ic" style="background:#bbb"></span>마운트</div>
    <div class="sb-grp">저장소</div>
    <div class="sb-it"><span class="ic" style="background:#bbb"></span>메모리</div>
    <div class="sb-it"><span class="ic" style="background:#bbb"></span>워크스페이스</div>
    <div class="sb-grp">운영</div>
    <div class="sb-it"><span class="ic" style="background:#bbb"></span>크론 작업</div>
    <div class="sb-it"><span class="ic" style="background:#bbb"></span>비용</div>
    <div class="sb-it"><span class="ic" style="background:#bbb"></span>토큰 맥싱</div>
  </div>
  <div class="sb-foot"><span class="ft-it">☀</span><span class="ft-it">⚙</span></div>
</div>
```
→ Add `on` class to the item matching THIS screen (e.g. screen 04 스킬 → the 스킬 `.sb-it` gets `on`, others not). Put `on` on exactly one item. Trim unused groups to keep the sidebar readable (you don't need all 14 items — keep ~8-10 most relevant + the active one).

### Topbar template (paste, keep mode tabs consistent)
```html
<div class="bar">
  <span class="bm on">Console</span><span class="bm">Knowledge</span><span class="bm">Chat</span>
  <div class="grow"></div>
  <span class="btn-sm">⚡<span class="kbd">⌘J</span></span>
  <span class="btn-sm">⌕ 검색 <span class="kbd">⌘K</span></span>
  <span class="clock">◷</span>
</div>
```

---

## 4. Rubric (8 dimensions — score 1–5, cite why)

| # | dim (한글) | what to look for |
|---|---|---|
| 1 | 정보구조 | 3초 안에 용도 파악? 시각 무게가 중요도와 일치? 영역별 초점? |
| 2 | 레이아웃 | 그리드 정합, 정렬, 밀도 vs 여백, 반응형, 낭비/과밀 |
| 3 | 기능배치 | 액션이 보이는 자리에? 1차 액션 강조, 파괴적 액션 보호? 숨겨진 것? **라벨/표시가 실제 동작과 일치?** (감시 전용 "제한"/"가드", 죽은 버튼, 강제 안 되는 "활성화") |
| 4 | 내비게이션 | 현재 위치 명확(활성 상태)? 링크 목적 예측 가능? 뒤로/탈출 명확? |
| 5 | 일관성 | 셸·형제 화면과 일치(간격·모양·용어·아이콘)? 이탈 패턴? |
| 6 | 인터랙션 | hover/active/focus/disabled? loading/empty/error? 클릭 가능 시그널? |
| 7 | 접근성 | 대비, 포커스 가시성, 키보드, 시맨틱, 타겟 크기≥44px, ARIA |
| 8 | 타이포 | 폰트·크기 리듬·행길이·색 사용(주도색+강조, 진흙 회색 분배 회피)·아이콘 |

Scoring: 5=훌륭 · 4=양호(사소한) · 3=보통(개선 여지) · 2=약함(명확한 문제) · 1=심각.
**Dimension 3 is weighted** — actively look for "label says X but behavior is Y" (this is the
most valuable finding type: dead toggles, monitor-only guards, fake "enabled" states).

---

## 5. Severity
- `high` — 작업을 차단하거나 심각히 저해 (broken, misleading, blocking)
- `med` — 마찰/혼란 (friction, inconsistency, discoverability gap)
- `low` — 폴리시 (polish, minor density/spacing/typography)

A genuinely clean screen scores high (4-5) with fewer findings. Do NOT manufacture problems.

---

## 6. Output contract (HARD — violations reject the fragment)

1. Output file: `frag-<group>.html` (group name given in your assignment).
2. Content: **raw `<section class="screen" id="screen-NN">…</section>` blocks only**, one per assigned screen, in screen-number order. No doctype/html/head/body. No `<style>` anywhere.
3. **Pin count == finding count per screen.** Pin N in the current mockup ↔ finding N (1-based, in order). Every finding has a numbered pin at its exact spot in the CURRENT panel.
4. **Every finding cites `file:line`** read from real source. No guessed paths. If unsure of a line, re-read the file range — do not approximate.
5. Reuse ONLY the master classes above + inline `style=""`. Bespoke layout = inline styles, never a new class that needs definition.
6. Two panels in `.compare` (current-annotated + proposed), EXCEPT the proposed panel must visibly fix every dimension scored ≤2.
7. Mockups are FAITHFUL reconstructions at the panel width (~550px). Compact: show 3 representative rows + ellipsis, never 20 fake rows. Representative placeholder content, labeled as such — never invent features.
8. Korean for all headings/labels/findings (per $language=ko). Identifiers/routes/file paths stay verbatim (English/source).
9. Sidebar/topbar are SHARED chrome — copy from §3 templates, set the active item, trim to ~8-10 items. Do NOT redesign the sidebar per screen.

---

## 7. Self-check (run before yielding — each screen)

- [ ] `<section class="screen" id="screen-NN">` present, id matches screen number
- [ ] rubric has exactly 8 `.rcell`, scores 1–5, labels in Korean in order
- [ ] `.compare` has 2 panels (cur + prop), each with a `.frame`
- [ ] **count(`.pin` in current panel) == count(`.fitem`)** for this screen
- [ ] every `.fitem` has a `.src` with at least one `file:line` from source I actually read
- [ ] NO `<style>` tag anywhere; all bespoke styling is inline
- [ ] proposed panel addresses every dimension scored ≤2
- [ ] sidebar has exactly one `.sb-it.on`

---

## 8. Worked example
Open `oxios-ui-audit.html`, find `<!-- SCREEN 01 — DASHBOARD (reference) -->`.
That section is the gold standard: rubric, two panels, 5 pins ↔ 5 findings, changes.
Match its density, fidelity, and citation style exactly.
