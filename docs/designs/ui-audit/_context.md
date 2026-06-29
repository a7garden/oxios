# Oxios Web UI/UX Audit — Shared Context (READ FIRST)

You are producing screen-audit `<section>` fragments for a self-contained HTML report.
**Read these three files before writing anything:**
1. This file (`docs/designs/ui-audit/_context.md`).
2. `docs/designs/ui-audit/_head.html` — the master `<style>`; this is the **full CSS class catalog**. Reuse these classes. Do NOT invent new global classes.
3. `docs/designs/ui-audit/frag-01-dashboard.html` — the **worked reference**. Your sections MUST match its exact structure.

## Output contract (per assigned screen)

One `<section class="screen" id="screen-NN">` per screen, concatenated into your assigned fragment file. **Raw `<section>` blocks only — NO `<!DOCTYPE>`, `<html>`, `<head>`, or `<body>`.** Number screens with the `NN` assigned to you.

Structure (copy from the reference):
```
<section class="screen" id="screen-NN">
  <div class="screen-head">  num badge NN · titles(h2 + route) · severity tally (H/M/L counts)  </div>
  <p>한 문장 요약(이 화면이 무엇인가)</p>
  <div class="rubric">  8 cells, each .cell.rN with <span class="sc">N</span> + 짧은 라벨  </div>
  <div class="compare">
    <div class="panel cur"><div class="plabel">…</div><div class="stage">… mockup … <span class="pin h|m|l">N</span> …</div></div>
    <div class="panel prop"><div class="plabel">…</div><div class="stage">… fixed mockup …</div></div>
  </div>
  <div class="findings"><h4>발견사항 (N)</h4><ul class="flist"> <li><span class="badge h|m|l">…</span><div>…<span class="cite">file:line</span></div></li> </ul></div>
  <div class="changes"><h4>변경 방향</h4><ul>…</ul></div>
</section>
```

## Hard rules (violations = reject the fragment)
1. **Every finding cites `file:line` read from the real source.** Open the route + its sub-components before scoring. Never score a screen you didn't read.
2. **Pin count == finding count, per screen.** Each numbered `<span class="pin">` on the current panel maps 1:1 to a `<li>` in `.flist` with the same number. Pin numbers are sequential per screen (1,2,3…).
3. **Mockups reproduce real tokens** (see below). Current panel = faithful rebuild; proposed panel = same tokens evolved with fixes.
4. **No bare `<style>` and no global selectors.** All mockup styling is either inline `style="…"` or scoped under `#screen-NN …`. A bare `.row{}` leaks globally on merge and corrupts the whole report.
5. **Before/after parity.** If a pin marks a problem in the current panel, the proposed panel visibly fixes that exact spot.
6. **Korean labels** in mockups (per convention). Routes/identifiers stay as-is. Report text is 한국어.
7. **No fabricated data.** Use representative placeholder content, labeled.

## The 8-dimension rubric (score 1–5 each, cited)
| # | 차원 | 보는 것 |
|---|---|---|
| 1 | 정보구조·위계 | 3초 안에 목적 파악? 시각 무게가 중요도와 일치? |
| 2 | 레이아웃·공간 | 그리드 정합, 정렬, 밀도 vs 여백, 반응형 |
| 3 | 기능배치·발견성 | 액션이 예상 위치에? 주 액션 강조, 파괴적 액션 보호? **컨트롤의 표시와 실제 동작이 일치하는가?** (작동 안 하는 "enabled" 라벨, 죽은 버튼) |
| 4 | 내비게이션·길찾기 | 현재 위치 명확(활성 상태)? 링크 목적 예측 가능? |
| 5 | 일관성 | 셸·형제 화면과 일치(간격·형태·용어)? 이탈 패턴? |
| 6 | 인터랙션·어포던스 | hover/active/focus/disabled? loading/empty/error? 클릭 신호? |
| 7 | 접근성 | 대비, 포커스 가시성, 키보드, 타겟 크기(≥44px), 시맨틱 |
| 8 | 타이포·시각품질 | 글꼴, 크기 리듬, 색 사용(균등 분산 vs 의미있는 강조) |

점수 가이드: 문제 없음 4–5 / 실질적 문제는 그 이하, 사유를 인용. 한 화면이 노력의 80%를 차지하지 않게.

## Severity
- **high** (`h`): 작업을 차단하거나 심하게 저해. 붉은 핀/배지.
- **med** (`m`): 마찰·혼란. 호박색.
- **low** (`l`): 다듬기. 슬레이트.
색은 클래스가 결정(.pin.h/.badge.h 등). 임의 색 사용 금지.

## Design tokens (LIGHT — use in mockups)
```
--background: oklch(0.99 0 0)        --foreground: oklch(0.141 0.005 285.823)
--card: oklch(1 0 0)                 --muted: oklch(0.967 0.001 286.375)
--muted-foreground: oklch(0.552 0.016 285.938)
--primary: oklch(0.23 0.025 265)     --primary-foreground: oklch(0.985 0 0)
--accent: oklch(0.967 0.003 265)     --border: oklch(0.92 0.004 286.32)
--ring: oklch(0.45 0.04 265)
--sidebar: oklch(0.978 0.002 265)    --sidebar-accent: oklch(0.967 0.003 265)
--success: oklch(0.596 0.145 163)    --warning: oklch(0.669 0.162 70)
--error: oklch(0.577 0.245 27.325)   --info: oklch(0.623 0.214 259.815)
radius: 0.625rem(10px) base; sm .6×/md .8×/lg 1×/xl 1.4×
font: Geist(sans), Geist Mono(mono·routes·kbd); text-2xs=10px(micro only)
shadow-sm 0 1px 2px /0.04 · md 0 2px 8px /0.06 · lg 0 4px 16px /0.08
motion: 200ms cubic-bezier(0.16,1,0.3,1); animate-stagger 40ms
```
(DARK — only if a screen is dark-only: background oklch(0.13 0.005 285.823), card oklch(0.19 0.008 265), primary oklch(0.91 0.03 265), border oklch(1 0 0/10%).)

## Sidebar template (paste into every mockup, toggle active item per screen)
The shell: left sidebar `width:128px` (mock scale), `background:oklch(0.978 0.002 265)`, `border-right:1px solid oklch(0.92 0.004 286.32)`, padding 8px 6px. Header `⚡ Oxios` bold. Section headers: `text-transform:uppercase; font-size:9px; color:oklch(0.552 0.016 285.938)`. Active item: `background:oklch(0.967 0.003 265); border-radius:6px; font-weight:600`. Inactive: `color:oklch(0.552 0.016 285.938)`.
Nav groups (Console mode): 메인[대시보드/] · 에이전트[에이전트/agents, 페르소나/personas, 스킬/skills] · 프로젝트[프로젝트/projects, 마운트/mounts] · 저장소[메모리/memory, 워크스페이스/workspace] · 운영[크론잡/cron-jobs, 비용/budget, 토큰맥싱/token-maxing] · 인프라[MCP/mcp, 이메일/email, 깃/git] · 시스템[리소스/resources, 보안/security]. Chat/Knowledge/Settings are top-level surfaces (mode tabs).
See `frag-01-dashboard.html` for the exact sidebar mock to copy.

## Severity tally format (screen-head right side)
`<span style="color:var(--hi)">N H</span> · <span style="color:var(--med)">N M</span> · <span style="color:var(--lo)">N L</span>`

## Self-check (run before finishing each section)
- [ ] 읽지 않은 화면에 점수를 매기지 않았는가? (file:line 인용이 실제 소스 기반인가)
- [ ] 핀 수 == 발견사항 수인가? (핀 번호와 `<li>`가 1:1)
- [ ] bare `<style>` / 전역 선택자가 없는가? (모두 inline 또는 `#screen-NN` 스코프)
- [ ] 현재 패널이 실제 토큰으로 충실하게 재현되었는가?
- [ ] 제안 패널이 3점 이하 차원의 문제를 시각적으로 수정하는가?
- [ ] 마크업이 한국어인가?

## Screen inventory (for reference — your assignment lists which NN you own)
01 대시보드 index.tsx · 02 에이전트 agents/index.tsx · 03 페르소나 personas.tsx · 04 스킬 skills.tsx · 05 프로젝트 projects/index.tsx · 06 마운트 mounts/index.tsx · 07 메모리 memory.tsx · 08 워크스페이스 workspace/index.tsx · 09 크론잡 cron-jobs.tsx · 10 비용 budget.tsx · 11 토큰맥싱 token-maxing.tsx · 12 MCP mcp.tsx · 13 이메일 email.tsx · 14 깃 git.tsx · 15 리소스 resources.tsx · 16 보안 security.tsx · 17 채팅 chat.tsx · 18 설정 settings.tsx
