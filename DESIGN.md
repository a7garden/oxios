# Oxi Design System

> Unified design specification for the **oxi** family: `oximemo`, `oxibuilder`, `oxios`.
> Canonical source of truth — each project's own design docs are now references into this file.
> Version: **v1.0** · Date: 2026-07-31

---

## Config

design-farmer Phase 0 re-entry reconstruction block. Base = shared across all three projects; per-project overrides listed separately.

```yaml
# ── Base (shared) ──
framework: vite-react
stylingApproach: tailwind-v4
packageManager: bun
headlessLibrary: base-ui
themeStrategy: light-dark
themeLibrary: custom
accessibilityLevel: apca
radiusTone: rounded
colorSpace: oklch
targetPlatforms: multi-platform
designMaturity: emerging
maturityScore: 6
fontBody: "SUIT Variable"
fontDisplay: "SUITE Variable"
fontMono: "Geist Mono Variable"
darkModeTrigger: ".dark class"
semanticTokenRule: "components consume Tailwind utilities only; no dark: variant; no [data-theme=dark]"

# ── Per-project overrides ──
oximemo:
  targetPlatform: macos-native
  framework: tauri-2-react
  virtualScroll: "@tanstack/react-virtual"
  globalShortcut: true
  overlayWarmup: true
oxibuilder:
  targetPlatform: web
  isMonorepo: true
  themeTriggerLegacy: "[data-theme] (migrating to .dark)"
  fontBodyLegacy: "Pretendard Variable (migrating to SUIT)"
  fontDisplayLegacy: "Fraunces (migrating to SUITE)"
  lobbyModes: [list, grid, canvas]
oxios:
  targetPlatform: web
  isMonorepo: true
  routing: tanstack-router
  serverState: tanstack-query
  clientState: zustand
  fontBodyLegacy: "Geist (migrating to SUIT)"
  storageKeyLegacy: "oxios-theme (migrating to oxi-theme)"
```

---

## Table of Contents

Config · [Design Farmer re-entry](#config)
0. [How to read this document](#0-how-to-read-this-document)
1. [Direction](#1-direction)
2. [Token architecture](#2-token-architecture)
3. [Color](#3-color)
4. [Typography](#4-typography)
5. [Spacing, radius, elevation](#5-spacing-radius-elevation)
6. [Component patterns](#6-component-patterns) — §6.1 Shared size scale · §6.2 Buttons · §6.3 Cards · §6.4 Inputs · §6.5 Badges · §6.6 Select · §6.7 Dialog · §6.8 Popover/Tooltip · §6.9 Toast · §6.10 Tabs · §6.11 Sidebar · §6.12 Do/Don't
7. [Layout & motion](#7-layout--motion)
8. [Theming & dark mode](#8-theming--dark-mode)
9. [Accessibility](#9-accessibility)
10. [Per-project surfaces](#10-per-project-surfaces)
11. [Decision log](#11-decision-log)
12. [Migration plan](#12-migration-plan)

---

## 0. How to read this document

**Three projects, one system.** `oximemo` (macOS note app), `oxibuilder` (multi-extension personal site), `oxios` (agent operating system dashboard) share an underlying design grammar but each owns a distinct surface identity. This document defines the shared grammar; per-project customizations live in §10.

**Priority of authority.** When this document disagrees with an existing project doc, this document wins. Out-of-date claims in project docs (e.g. accent colors, dark-mode triggers) must be rewritten to point here.

**Audience.** Engineers (component implementations), designers (new screens, audits), AI agents (the "Agent Prompt Guide" appendix in §10).

**Gaps.** oximemo's `doc/DESIGN.md` defines a UI stack (§7.1) and a 6-hue OKLCH label palette (§7.7) but does **not** specify a font, a type scale, or a sidebar pattern — those are defined here for the first time. oxios defines typography around Geist (10 references across 3 files in `web/src/`); its migration to SUIT is straightforward and tracked in §12.3.

---

## 1. Direction

### 1.1 Voice

| Tone | Read |
|------|------|
| **Calm authority** | Information density over decoration; nothing competes with the work. |
| **Ink on paper** | Neutral surfaces, hairline borders, weight-led hierarchy — color carries meaning, never decoration. |
| **Native where it lives** | oximemo follows macOS overlays · oxibuilder follows web shell + canvas · oxios follows dense dashboard patterns. |
| **Functional hues** | Color is data, not branding. Six semantic hues label notes, status, and chrome. No single "brand accent." |

### 1.2 Design principles

1. **Capture is friction-free.** (from oximemo) The fastest path wins; the design must never make a user wait.
2. **Files are the source of truth.** (from oximemo) CSS variables are just aliases of semantic tokens. Never hand-edit a primitive in a component.
3. **Less is more.** (from oximemo) No decorative chrome, no AI-default gradients, no Inter-as-identity.
4. **Paper and ink.** (from oxibuilder) Neutral surfaces with one quiet accent — but the accent is *a palette of six*, not a single hue.
5. **State is calm.** (from oxios) Status colors earn their weight by meaning; nothing else gets a hue.

### 1.3 What this rules out

- Single brand hue anchoring identity.
- `dark:` variant sprinkled through component files (forbidden).
- `[data-theme="dark"]` selectors for light/dark switching (deprecated — use `.dark` class; see §8). The `[data-theme="brand-x"]` variant axis (§8.4) is a separate concern and remains permitted.
- Hex, `rgb()`, `hsl()` in component files — OKLCH only, and only inside the token layer.
- Inter / Roboto / system-ui as identity font (acceptable as fallback).
- HSL-based dark-mode inversion (`filter: invert()`) — perceptual contrast breaks.
- A serif/sans contrast as the type system's identity move (SUIT + SUITE are both sans).

### 1.4 What this permits

- Six-hue OKLCH label palette shared across all three projects.
- `.dark` class as the single light/dark trigger.
- Two-tier semantic tokens: components consume `surface`/`text`/`border` only.
- SUIT (UI body) + SUITE (UI headline) as the unified Korean type pairing; Latin `Geist Mono` for code.
- Per-project console chrome accent *only* for oxibuilder's legacy sidebar (kept v1, removed v2; see §10.2).

---

## 2. Token architecture

### 2.1 The three tiers

```
Primitive tokens (raw OKLCH ramps, never touched by components)
       │
       ▼
Semantic tokens (purpose-driven aliases — surface, text, border, status, hue-label)
       │
       ▼
Component tokens (component-shaped composites — button.bg, card.border, badge.success-bg)
       │
       ▼
Utility classes (Tailwind v4 utilities bound to component tokens via @theme inline)
       │
       ▼
Component code (consumes utilities; never primitives, never semantic raw, never `dark:`)
```

**Single rule:** Component code uses Tailwind utilities (e.g. `bg-surface`, `text-text`, `border-line`). It never reads `--color-*` directly, never imports a primitive, and never writes `dark:`.

### 2.2 File layout (per project)

```
src/
├── tokens/
│   ├── primitives.css       ← Tier 1: OKLCH ramps (hues, neutrals)
│   ├── semantic.css         ← Tier 2: light theme semantic aliases
│   ├── semantic-dark.css    ← Tier 2: dark theme overrides (.dark)
│   ├── components.css       ← Tier 3: component-shaped aliases
│   └── theme.css            ← @theme inline exposure to Tailwind utilities
└── components/              ← consumes Tailwind utilities only
```

`oxibuilder/web/src/shared/tokens.css` and `oxios/web/src/index.css` are the v0 anchors; their semantic tokens are migrated to the layout above (see §12). `oximemo/apps/desktop/src/lib/color.ts` continues to host the OKLCH label palette + clamp helpers.

### 2.3 Naming convention

| Tier | Pattern | Example |
|------|---------|---------|
| Primitive | `--p-{hue}-{step}` | `--p-red-500`, `--p-neutral-100` |
| Semantic | `--color-{role}-{variant?}` | `--color-surface`, `--color-text`, `--color-text-muted`, `--color-hue-red` |
| Component | `--cmp-{component}-{part}-{state?}` | `--cmp-button-bg`, `--cmp-card-border` |
| Utility | `{role}-{variant?}` (Tailwind class) | `bg-surface`, `text-muted`, `border-line`, `bg-hue-red` |

---

## 3. Color

### 3.1 Why OKLCH

Inherited from oximemo §7.7 and oxibuilder §3.2:

- **Perceptual uniformity.** Same L → same perceived brightness, hue-independent.
- **CSS-native.** Modern WebKit (Tauri), Blink (Vite), and all evergreen browsers render `oklch()` directly.
- **Predictable dark-mode inversion.** Flip L only; H and C stay fixed for neutrals and label hues. Status hues follow APCA-optimized perceptual tuning (see below).
- **Mechanical palette generation.** Rotate H, keep L/C fixed → safe 6-hue label palette.

> **L-only rule (scoped):** For **neutral** and **label-hue** tokens, dark-mode adjustment modifies **L only** — never C, never H. For **status hues**, both L and C/H may shift between light and dark, because the canonical values are APCA-optimized for each mode independently (inherited verbatim from oxios's measured dashboard palette). The warm→cool hue shift (95°→265°) on neutrals is the sole structural exception, applied once at the primitive tier.

### 3.2 Primitive palette

#### Neutral (warm-tinted "paper / ink")

| Step | Light | Dark | Note |
|------|-------|------|------|
| `p-neutral-0`   | `oklch(98.5% 0.004 95)`  | — | Lightest paper |
| `p-neutral-50`  | `oklch(95% 0.006 95)`   | — | Surface (light) |
| `p-neutral-100` | `oklch(90% 0.007 95)`   | `oklch(28% 0.015 265)` | Border (light), elevated surface (dark) |
| `p-neutral-300` | `oklch(75% 0.010 95)`   | — | Tertiary text (light) |
| `p-neutral-500` | `oklch(55% 0.012 95)`   | `oklch(65% 0.012 265)` | Muted text both modes |
| `p-neutral-700` | `oklch(35% 0.012 265)`  | — | Secondary text (light) |
| `p-neutral-900` | `oklch(18% 0.015 265)`  | — | Primary text (light) — ink |
| `p-neutral-950` | `oklch(13% 0.020 265)`  | `oklch(13% 0.020 265)` | Canvas (dark) |
| `p-neutral-999` | `oklch(0% 0 0)`         | — | Pure ink for shadows |

Light tints sit on hue `95` (warm paper); dark shades shift to `265` (cool ink). This is the only place the warm/cool split happens — semantic tokens below stay neutral.

**CSS declaration** (copy-pasteable — this is the only `:root` block primitives live in):

```css
:root {
  /* Neutral ramp — "paper / ink" */
  --p-neutral-0:   oklch(98.5% 0.004 95);
  --p-neutral-50:  oklch(95%   0.006 95);
  --p-neutral-100: oklch(90%   0.007 95);
  --p-neutral-300: oklch(75%   0.010 95);
  --p-neutral-500: oklch(55%   0.012 95);
  --p-neutral-700: oklch(35%   0.012 265);
  --p-neutral-900: oklch(18%   0.015 265);
  --p-neutral-950: oklch(13%   0.020 265);
  --p-neutral-999: oklch(0%    0 0);
}
```

Dark-mode neutral overrides (applied in `.dark`, see §3.3 — only steps that differ from light are re-declared):

```css
.dark {
  --p-neutral-100: oklch(28% 0.015 265);  /* border on dark */
  --p-neutral-500: oklch(65% 0.012 265);  /* muted text on dark */
  --p-neutral-950: oklch(13% 0.020 265);  /* canvas (same value, explicit) */
}
```

#### Functional hues — the six-hue label palette

Shared by all three projects. Replaces per-project accent colors.

| Name | OKLCH (canonical) | Use |
|------|-------------------|-----|
| Red    | `oklch(0.75 0.15 25)`  | Urgent, blocked, destructive action |
| Amber  | `oklch(0.75 0.15 75)`  | Caution, idea, pending |
| Green  | `oklch(0.75 0.13 145)` | Complete, positive, success |
| Teal   | `oklch(0.75 0.12 195)` | Reference, informational |
| Blue   | `oklch(0.70 0.14 250)` | Working, in-progress |
| Purple | `oklch(0.72 0.15 310)` | Inspiration, personal |

All six share **L ≈ 0.70–0.75** and **C ≈ 0.12–0.15** so a card with any label has the same "visual weight" against any background. The dark-mode adjustment is applied in the semantic tier (L +0.05 via a precomputed step) — primitives stay single-valued.

#### Status hues (oxios-style dashboard semantics)

| Semantic | Light | Dark | Maps to hue |
|----------|-------|------|-------------|
| Success | `oklch(0.596 0.145 163)` | `oklch(0.723 0.219 149.579)` | Green family |
| Warning | `oklch(0.669 0.162 70)`  | `oklch(0.769 0.188 70.08)` | Amber family |
| Error   | `oklch(0.577 0.245 27.325)` | `oklch(0.704 0.191 22.216)` | Red family |
| Info    | `oklch(0.623 0.214 259.815)` | `oklch(0.685 0.196 259)` | Blue family |

Status colors are *not* the same as label hues. Labels are user-chosen and tag-like; status colors are system-driven and live in the dashboard domain. Both reuse the same six hue families.

### 3.3 Semantic tier

Light theme (default `:root`):

```css
:root {
  /* Surfaces */
  --color-surface:        var(--p-neutral-0);
  --color-surface-raised: oklch(100% 0 0);
  --color-surface-sunken: var(--p-neutral-50);
  --color-surface-muted:  oklch(96% 0.005 265);

  /* Text */
  --color-text:        var(--p-neutral-900);  /* ink */
  --color-text-muted:  var(--p-neutral-700);
  --color-text-subtle: oklch(42% 0.010 95);  /* L=0.42 — passes L≤0.45 on light surfaces */
  --color-text-inverse: oklch(98.5% 0.004 95);

  /* Border */
  --color-border:      var(--p-neutral-100);
  --color-border-strong: oklch(82% 0.008 265);
  --color-focus-ring:  oklch(0.45 0.04 265);

  /* Functional hue labels (oximemo §7.7) */
  --color-hue-red:    oklch(0.75 0.15 25);
  --color-hue-amber:  oklch(0.75 0.15 75);
  --color-hue-green:  oklch(0.75 0.13 145);
  --color-hue-teal:   oklch(0.75 0.12 195);
  --color-hue-blue:   oklch(0.70 0.14 250);
  --color-hue-purple: oklch(0.72 0.15 310);

  /* Status — verbatim OKLCH from §3.2 status hues table (no primitive ramp needed) */
  --color-status-success: oklch(0.596 0.145 163);
  --color-status-warning: oklch(0.669 0.162 70);
  --color-status-error:   oklch(0.577 0.245 27.325);
  --color-status-info:    oklch(0.623 0.214 259.815);

  /* Status surfaces (panel backgrounds) */
  --color-status-success-subtle: oklch(0.97 0.014 163);
  --color-status-warning-subtle: oklch(0.97 0.014 70);
  --color-status-error-subtle:   oklch(0.97 0.014 27);
  --color-status-info-subtle:    oklch(0.97 0.014 259);

  /* Interactive primary — dedicated button fill (darker than label hue for white-text contrast) */
  --color-interactive-primary:           oklch(0.45 0.14 250);  /* L=0.45 — white text passes APCA Lc 60 */
  --color-interactive-primary-foreground: oklch(98.5% 0.004 95);

  /* Status text on subtle — darker variants for text inside -subtle surfaces (Lc 75+ at 10px) */
  --color-status-success-on-subtle: oklch(0.40 0.13 163);
  --color-status-warning-on-subtle: oklch(0.45 0.14 70);
  --color-status-error-on-subtle:   oklch(0.42 0.18 27);
  --color-status-info-on-subtle:    oklch(0.42 0.16 259);
}
```

Dark theme (`.dark`):

```css
.dark {
  --color-surface:        var(--p-neutral-950);
  --color-surface-raised: oklch(22% 0.016 265);
  --color-surface-sunken: oklch(11% 0.018 265);
  --color-surface-muted:  oklch(20% 0.012 265);

  --color-text:        var(--p-neutral-0);
  --color-text-muted:  var(--p-neutral-300);
  --color-text-subtle: oklch(75% 0.012 265);  /* L=0.75 — passes L≥0.75 on dark surfaces */
  --color-text-inverse: var(--p-neutral-900);

  --color-border:      oklch(28% 0.015 265);
  --color-border-strong: oklch(40% 0.015 265);
  --color-focus-ring:  oklch(0.65 0.05 265);

  /* Hue labels: L +0.05 via precomputed steps */
  --color-hue-red:    oklch(0.78 0.14 25);
  --color-hue-amber:  oklch(0.78 0.14 75);
  --color-hue-green:  oklch(0.78 0.12 145);
  --color-hue-teal:   oklch(0.78 0.11 195);
  --color-hue-blue:   oklch(0.74 0.13 250);
  --color-hue-purple: oklch(0.76 0.14 310);
  /* Status — APCA-optimized for dark (C/H may shift per §3.1; values from §3.2 table) */
  --color-status-success: oklch(0.723 0.219 149.579);
  --color-status-warning: oklch(0.769 0.188 70.08);
  --color-status-error:   oklch(0.704 0.191 22.216);
  --color-status-info:    oklch(0.685 0.196 259);

  /* Status surfaces (dark) */
  --color-status-success-subtle: oklch(0.20 0.03 163);
  --color-status-warning-subtle: oklch(0.20 0.03 70);
  --color-status-error-subtle:   oklch(0.20 0.03 27);
  --color-status-info-subtle:    oklch(0.20 0.03 259);

  /* Interactive primary — lighter fill for dark mode (dark text on lighter blue) */
  --color-interactive-primary:           oklch(0.70 0.14 250);  /* L=0.70 — dark text passes APCA Lc 60 */
  --color-interactive-primary-foreground: oklch(15% 0.015 265);

  /* Status text on subtle — lighter variants for text inside dark -subtle surfaces */
  --color-status-success-on-subtle: oklch(0.80 0.10 163);
  --color-status-warning-on-subtle: oklch(0.82 0.10 70);
  --color-status-error-on-subtle:   oklch(0.78 0.12 27);
  --color-status-info-on-subtle:    oklch(0.78 0.10 259);
}
```

> **L inversion rule (scoped per §3.1):** For neutrals and label hues, dark-mode adjusts L only. Status hues are APCA-optimized per-mode and may shift C/H — the verbatim oxios dashboard values are authoritative, not derived by inversion.

### 3.4 Custom OKLCH input (oximemo §7.7)

Users may enter raw OKLCH values. UI clamps to perceptually safe ranges:

```ts
const SAFE_RANGES = {
  L: [0.50, 0.90],
  C: [0.05, 0.25],
  H: [0, 360],
};

export function clampOklch({ L, C, H }: OklchTriplet): OklchTriplet {
  return {
    L: clamp(L, 0.50, 0.90),
    C: clamp(C, 0.05, 0.25),
    H: ((H % 360) + 360) % 360,
  };
}

export function oklchToCss({ L, C, H }: OklchTriplet): string {
  return `oklch(${(L * 100).toFixed(1)}% ${C.toFixed(2)} ${H.toFixed(0)})`;
}
```

Stored verbatim in frontmatter as `color = "oklch(L C H)"`. Parse failures fall back to `--color-hue-blue` (neutral default) and emit a `doctor` warning.

---

## 4. Typography

### 4.1 Font pairing — both sans, density contrast only

**No serif in this system.** SUIT and SUITE are *both* sans-serif. The pair works by **density** rather than by category:

- **SUIT** = UI 본문용 폰트. 본고딕-based, optimized for long Korean passages at small sizes (10–16px) with even rhythm and high x-height. Variable wght 100–900.
- **SUITE** = UI 헤드라인 타입페이스. Geometric construction (정원·직각·직선·사선), tighter proportions, designed for display sizes (≥ 20px). Variable wght 300–900.

Using both means Korean surfaces stay inside a single typographic family — readable at body sizes via SUIT, distinctive at headline sizes via SUITE — without introducing a serif/sans contrast that would compete with the "ink on paper" tone. oxibuilder's old Fraunces display serif is removed in this unification (see §12.2).

| Role | Family | Notes |
|------|--------|-------|
| Body / UI | **SUIT** (`'SUIT Variable'`) | Variable wght 100–900, SIL OFL. Korean-first; Latin sized to match SUIT's vertical metrics. |
| Headline | **SUITE** (`'SUITE Variable'`) | Variable wght 300–900. Display headings and hero sections only. |
| Monospace | **Geist Mono** | Latin-first mono. Sufficient for code, IDs, JSON fragments. |
| Latin fallback | `system-ui, -apple-system, "Inter", sans-serif` | Only when SUIT is loading or unavailable. |

### 4.2 Distribution (jsDelivr CDN, NOT Google Fonts)

Both fonts ship via jsDelivr at the `sun-typeface` GitHub org. **They are not on Google Fonts.** Verified live CSS:

```css
/* SUIT */
@font-face {
  font-family: 'SUIT Variable';
  font-weight: 100 900;
  src: url('./SUIT-Variable.woff2') format('woff2-variations');
}

/* SUITE */
@font-face {
  font-family: 'SUITE Variable';
  font-weight: 300 900;
  src: url('./SUITE-Variable.woff2') format('woff2-variations');
}
```

**Import (web projects):**

```css
@import url('https://cdn.jsdelivr.net/gh/sun-typeface/SUIT@2/fonts/variable/woff2/SUIT-Variable.css');
@import url('https://cdn.jsdelivr.net/gh/sun-typeface/SUITE@2/fonts/variable/woff2/SUITE-Variable.css');
```

For self-hosted builds (Tauri / production), bundle the woff2 files directly:
- `apps/desktop/src/assets/fonts/SUIT-Variable.woff2`
- `apps/desktop/src/assets/fonts/SUITE-Variable.woff2`

Vite projects may also use the npm mirror `@fontsource-variable/suit` (where available) and `@fontsource-variable/geist-mono`. Geist Mono ships from Fontsource, not jsDelivr.

### 4.3 Stack declaration

```css
:root {
  --font-sans:    "SUIT Variable", "SUIT", system-ui, -apple-system, "Inter", sans-serif;
  --font-display: "SUITE Variable", "SUITE", system-ui, -apple-system, "Inter", sans-serif;
  --font-mono:    "Geist Mono Variable", "Geist Mono", ui-monospace, "SF Mono", Menlo, Consolas, monospace;
}
```

Both SUIT and SUITE share the same Latin fallback. SUITE never falls back to a serif.

### 4.4 Loading

```html
<link rel="preload" as="font" type="font/woff2" crossorigin
      href="/assets/fonts/SUIT-Variable.woff2" />
<link rel="preload" as="font" type="font/woff2" crossorigin
      href="/assets/fonts/SUITE-Variable.woff2" />
```

`font-display: swap` is the default — never `block`, which causes invisible-text flash. Preload **one body weight** (400) and **one display weight** (700). Variable axes handle the rest.

### 4.5 Type scale

| Role | Size | Line height | Weight | Tailwind |
|------|------|-------------|--------|----------|
| Display | 36px | 1.2 | 700 (SUITE) | `text-display` |
| Heading 1 | 30px | 1.25 | 700 (SUITE) | `text-3xl` |
| Heading 2 | 24px | 1.3  | 600 (SUITE) | `text-2xl` |
| Heading 3 | 20px | 1.35 | 600 (SUITE/SUIT) | `text-xl` |
| Heading 4 | 18px | 1.4  | 600 (SUIT) | `text-lg` |
| Body      | 16px | 1.55 | 400 (SUIT) | `text-base` |
| Body small | 14px | 1.5  | 400 (SUIT) | `text-sm` |
| Caption    | 12px | 1.45 | 500 (SUIT) | `text-xs` |
| Micro label | 10px | 1.4  | 500 + tracking-wide + uppercase | `text-2xs` |

**Principles** (inherited from oxios §3):
- Weight creates hierarchy, not color.
- `font-medium` (500) is the workhorse — labels, badges, inline emphasis.
- `font-semibold` (600) marks headings and section titles.
- `font-bold` (700) reserved for display.
- `tracking-wider` (0.05em) used exclusively for uppercase micro labels and status badges.
- **Headline rule.** Whenever the rendered text could exceed 20px, switch `font-family` to SUITE. This is a token swap, not a per-element rule: `font-display` Tailwind utility resolves to SUITE.

### 4.6 Per-project variant

| Project | Body | Headline | Note |
|---------|------|----------|------|
| oximemo | SUIT | SUITE | macOS native overlays use `-apple-system` for system menu text; in-app UI uses SUIT. No prior font, no migration. |
| oxibuilder | SUIT (migrating from Pretendard) | SUITE (migrating from Fraunces) | See §12.2 — Pretendard→SUIT, Fraunces→SUITE. |
| oxios | SUIT (migrating from Geist) | SUITE (new, hero titles only) | **10 Geist references** across 3 files (`index.css`, `tokens/index.ts`, `editor-prefs.ts`) — no component `.tsx` files reference Geist directly. v1 keeps Geist as fallback. See §12.3. |

---

## 5. Spacing, radius, elevation

### 5.1 Spacing scale

Base unit **4px** (Tailwind default). Scale: 2, 4, 6, 8, 10, 12, 16, 20, 24, 32, 40, 48, 64.

Default rhythm: `gap-2` (8px) inside components, `gap-4` (16px) between sections (oxios §5).

### 5.2 Radius scale

| Token | Value | Use |
|-------|-------|-----|
| `--radius-xs` | 0.25rem (4px) | Tags, dense chips |
| `--radius-sm` | 0.375rem (6px) | Inline elements, inputs (alt) |
| `--radius-md` | 0.5rem (8px) | Buttons, inputs, selects |
| `--radius-lg` | 0.75rem (12px) | Cards, dialogs |
| `--radius-xl` | 1rem (16px) | Popovers, tooltips |
| `--radius-2xl` | 1.25rem (20px) | Modals, hero surfaces |
| `--radius-full` | 9999px | Badges, pills, avatars |

oximemo cards use `--radius-lg`. oxios buttons use `--radius-md`. oxibuilder keeps `--radius-md` for v1 cards.

### 5.2.1 Component → radius mapping

Every component MUST reference a radius tier explicitly — never a raw `px` value. Component tokens alias the tier so per-project overrides are centralized.

| Component | Radius token | Tier | Value |
|-----------|-------------|------|-------|
| Button | `--button-radius` | `md` | 8px |
| Input | `--input-radius` | `md` | 8px |
| Select | `--select-radius` | `md` | 8px |
| Textarea | `--input-radius` | `md` | 8px |
| Card | `--card-radius` | `lg` | 12px |
| Dialog / Modal | `--dialog-radius` | `lg` | 12px |
| Popover / Dropdown | `--popover-radius` | `xl` | 16px |
| Tooltip | `--tooltip-radius` | `xl` | 16px |
| Badge / Pill | `--badge-radius` | `full` | 9999px |
| Avatar | `--avatar-radius` | `full` | 9999px |
| Tag / Chip (dense) | `--tag-radius` | `xs` | 4px |
| Sidebar item | `--nav-item-radius` | `md` | 8px |
| Sidebar item (dense) | `--nav-item-dense-radius` | `sm` | 6px |

**CSS token declarations** (add to `tokens/components.css`):

```css
:root {
  --button-radius:         var(--radius-md);
  --input-radius:          var(--radius-md);
  --select-radius:         var(--radius-md);
  --card-radius:           var(--radius-lg);
  --dialog-radius:         var(--radius-lg);
  --popover-radius:        var(--radius-xl);
  --tooltip-radius:        var(--radius-xl);
  --badge-radius:          var(--radius-full);
  --avatar-radius:         var(--radius-full);
  --tag-radius:            var(--radius-xs);
  --nav-item-radius:       var(--radius-md);
  --nav-item-dense-radius: var(--radius-sm);
}
```

**Why explicit tokens per component?** oximemo, oxibuilder, and oxios each have different `radiusTone` preferences (oxios=rounded, oxibuilder=soft, oximemo=native). Pointing components at tier tokens (`--card-radius` → `--radius-lg`) instead of hardcoding `12px` means a project can shift the entire card radius by changing one tier value, without touching any component file.

### 5.3 Elevation

| Level | Light | Dark | Use |
|-------|-------|------|-----|
| `shadow-xs` | `0 1px 2px oklch(0% 0 0 / 0.04)` | `0 1px 2px oklch(0% 0 0 / 0.30)` | Hover-only lift |
| `shadow-sm` | `0 1px 3px oklch(0% 0 0 / 0.07), 0 1px 2px oklch(0% 0 0 / 0.04)` | `0 1px 3px oklch(0% 0 0 / 0.40), 0 1px 2px oklch(0% 0 0 / 0.30)` | Cards, inputs |
| `shadow-md` | `0 4px 8px oklch(0% 0 0 / 0.08), 0 2px 4px oklch(0% 0 0 / 0.04)` | `0 4px 8px oklch(0% 0 0 / 0.45), 0 2px 4px oklch(0% 0 0 / 0.35)` | Dropdowns, popovers |
| `shadow-lg` | `0 12px 24px oklch(0% 0 0 / 0.10), 0 4px 8px oklch(0% 0 0 / 0.06)` | `0 12px 24px oklch(0% 0 0 / 0.50), 0 4px 8px oklch(0% 0 0 / 0.40)` | Modals, drawers |
| Focus | `outline: 2px solid var(--color-focus-ring); outline-offset: 2px` | same | All interactive elements **except form inputs/selects** (those use `--input-shadow-focus` per §6.4) |

Dark mode raises alpha significantly (oxibuilder §3.3 finding: shadows otherwise vanish on dark).

---

## 6. Component patterns

### 6.1 Shared control size scale

Button, Input, Select, and Textarea MUST share identical height/padding/font-size at each tier. This prevents visual inconsistency when controls sit next to each other in toolbars and forms.

| Size | Height | Padding X | Font Size | Tailwind | Use |
|------|--------|-----------|----------|----------|-----|
| xs | 28px | 10px | 12px | `h-7 px-2.5 text-xs` | Dense toolbars, inline filters |
| sm | 32px | 12px | 13px | `h-8 px-3 text-[13px]` | Inline, compact forms |
| md (default) | 36px | 14px | 14px | `h-9 px-3.5 text-sm` | Standard actions, default inputs |
| lg | 40px | 16px | 15px | `h-10 px-4 text-[15px]` | Primary CTAs, prominent inputs |
| icon | 36×36px | — | — | `h-9 w-9` | Icon-only buttons (matches md height) |

> The 4px height step (28→32→36→40) maintains visual rhythm. **Touch targets:** `lg` (40px + padding) clears the 44px minimum (§7.1). `md` (36px) meets it with border-shadow. `xs`/`sm` are desktop-only — on touch surfaces, bump to `md` minimum.

### 6.2 Buttons

| Variant | Background | Text | Border | Hover | Active | Disabled | Use |
|---------|-----------|------|--------|-------|--------|----------|-----|
| Primary | `bg-interactive-primary` | `text-interactive-primary-foreground` | none | `bg-interactive-primary/90` | `bg-interactive-primary/80` | `opacity-40 pointer-events-none` | Single CTA per screen |
| Secondary | `bg-surface-muted` | `text-text` | none | `bg-surface-muted/80` | `bg-surface-muted/70` | `opacity-40 pointer-events-none` | Supporting actions |
| Ghost | transparent | `text-text` | none | `bg-surface-muted` | `bg-surface-muted/80` | `opacity-40 pointer-events-none` | Inline, low-emphasis |
| Outline | transparent | `text-text` | `shadow-[0_0_0_1px_var(--color-border)]` | `bg-surface-muted` | `bg-surface-muted/80` | `opacity-40 pointer-events-none` | Tertiary actions |
| Destructive | `bg-status-error` | `text-text-inverse` | none | `bg-status-error/90` | `bg-status-error/80` | `opacity-40 pointer-events-none` | Delete, irreversible |

All variants use `rounded-[var(--button-radius)]` (maps to `--radius-md` = 8px). Sizes from §6.1 shared scale. `active:` state uses CSS `:active`, not JS handlers.

**Rule:** No more than one Primary per visible area. Use the project status color mapping when the action's domain is informational (e.g. "Run" → `bg-status-info`).

### 6.3 Cards

Three variants — choose by visual intent, not arbitrarily:

| Variant | Background | Border | Shadow | Use |
|---------|-----------|--------|--------|-----|
| **Elevated** | `bg-surface-raised` | none | `shadow-sm` | Floating content, popovers-as-cards |
| **Outlined** (default) | `bg-surface-raised` | `1px solid var(--color-border)` | none | Standard content containers |
| **Filled** | `bg-surface-sunken` | none | none | Nested sections, inset panels |

```html
<!-- Outlined (default) -->
<article class="bg-surface-raised text-text border border-line rounded-[var(--card-radius)] p-4">
  ...
</article>

<!-- Elevated -->
<article class="bg-surface-raised text-text rounded-[var(--card-radius)] shadow-sm p-4">
  ...
</article>

<!-- Filled -->
<div class="bg-surface-sunken text-text rounded-[var(--card-radius)] p-4">
  ...
</div>
```

Card sub-components: `CardHeader` (border-bottom `border-line/50`, `px-4 py-3`), `CardTitle` (`font-semibold text-text`), `CardDescription` (`text-text-muted text-sm`), `CardContent` (`p-4`), `CardFooter` (border-top `border-line/50`, `px-4 py-3`, flex-end).

Card anatomy (oximemo): optional 2px left bar `bg-hue-{name}` for labeled notes (oximemo §7.3). Hover on interactive cards: `hover:shadow-md transition-shadow`. oximemo grid uses `grid-cols-[repeat(auto-fill,minmax(240px,1fr))]`.

### 6.4 Inputs & Forms

**Border approach: `box-shadow`, not CSS `border`.** Using `box-shadow: 0 0 0 1px` prevents 1px layout shift when switching between default/focus/error states (border width adds to element size; box-shadow does not).

CSS tokens (define in `tokens/components.css`):

```css
:root {
  --input-shadow:        0 0 0 1px var(--color-border);
  --input-shadow-focus:  0 0 0 1px var(--color-focus-ring), 0 0 0 4px oklch(0.45 0.04 265 / 0.15);
  --input-shadow-error:  0 0 0 1px var(--color-status-error);
}
```

```html
<input class="h-9 px-3.5 rounded-[var(--input-radius)] bg-surface text-text text-sm
               placeholder:text-text-subtle
               shadow-[var(--input-shadow)]
               focus-visible:shadow-[var(--input-shadow-focus)]
               focus-visible:outline-none
               aria-[invalid=true]:shadow-[var(--input-shadow-error)]" />
```

| State | Shadow token | Notes |
|-------|-------------|-------|
| Default | `--input-shadow` | 1px border-line |
| Focus (keyboard) | `--input-shadow-focus` | 1px focus-ring + 4px glow halo |
| Error | `--input-shadow-error` | 1px status-error |
| Disabled | `opacity-40 cursor-not-allowed` | No shadow change |

> **Two focus layers:** the box-shadow border handles the *input outline* (no layout shift); the `outline` approach (§5.3, §9.4) handles *keyboard focus indication* on all other interactive elements. They are complementary, not conflicting — inputs use box-shadow for state, the focus halo is also box-shadow (not `outline`) to avoid double-ring.

Error state: `aria-invalid="true"`, helper text in `text-status-error text-xs`.

### 6.5 Badges

| Variant | Background | Text |
|---------|-----------|------|
| default | `bg-surface-muted` | `text-text-muted` |
| outline | transparent | `text-text` (shadow `0 0 0 1px var(--color-border)`) |
| success | `bg-status-success-subtle` | `text-status-success-on-subtle` |
| warning | `bg-status-warning-subtle` | `text-status-warning-on-subtle` |
| error | `bg-status-error-subtle` | `text-status-error-on-subtle` |
| info | `bg-status-info-subtle` | `text-status-info-on-subtle` |

Shape: `rounded-[var(--badge-radius)]` (9999px pill). Micro labels: `text-2xs font-medium tracking-wider uppercase`.

### 6.6 Select

**Select Trigger** — shares §6.1 size scale, same box-shadow border approach as Input.

```html
<button class="h-9 px-3.5 rounded-[var(--select-radius)] bg-surface text-text text-sm
               shadow-[var(--input-shadow)]
               focus-visible:shadow-[var(--input-shadow-focus)] flex items-center justify-between gap-2">
  <span>{value}</span>
  <ChevronDown class="size-4 text-text-muted shrink-0" />
</button>
```

- Chevron icon: trailing inline-end, `size-4` (16px), `text-text-muted`, vertically centered.
- Open state: trigger gets `shadow-[var(--input-shadow-focus)]`.
- Dropdown panel: see §6.8 Popover.

### 6.7 Dialog / Modal

```html
<Dialog>
  <DialogOverlay class="fixed inset-0 bg-black/40 backdrop-blur-sm" />
  <DialogContent class="bg-surface-raised text-text rounded-[var(--dialog-radius)] shadow-lg
                     w-full max-w-[520px] mx-auto p-6
                     data-[state=open]:animate-in data-[state=open]:fade-in
                     data-[state=open]:zoom-in-95" />
</Dialog>
```

| Property | Value |
----------|-------
| Backdrop | `oklch(0 0 0 / 0.4)` + `backdrop-blur-sm` |
| Surface | `bg-surface-raised` |
| Shadow | `shadow-lg` (§5.3) |
| Radius | `--dialog-radius` (12px) |
| Max widths | sm 380px · md 520px · lg 680px |
| Width | `w-full max-w-[Xpx]` (responsive, never fixed `width`) |
| Close button | top-right, `size-6`, `text-text-muted hover:text-text` |
| Entrance animation | scale + fade: `scale(0.95)` → `scale(1)` over `--duration-base` with `--ease-out` |
| Exit animation | reverse, `--duration-fast` |

### 6.8 Popover / Tooltip / Dropdown

All overlay surfaces share the same treatment:

```html
<div class="bg-surface-raised text-text border border-line rounded-[var(--popover-radius)] shadow-md p-3
            data-[state=open]:animate-in data-[state=open]:fade-in data-[state=open]:slide-in-from-bottom-1">
  ...
</div>
```

| Property | Value |
|----------|-------|
| Background | `bg-surface-raised` |
| Border | `1px solid var(--color-border)` (CSS border OK here — no state changes) |
| Shadow | `shadow-md` (§5.3) |
| Radius | `--popover-radius` (16px) |
| Entrance | fade + `translateY(-4px)` → `translateY(0)`, `--duration-fast` + `--ease-out` |
| Exit | reverse, `--duration-fast` |
| z-index | above dialogs (see §7) |

Tooltip variant: `p-2 text-xs text-text-muted bg-surface-sunken`, max-width 240px, `--duration-fast` delay.

### 6.9 Toast / Notification

Use the **Sonner** library (`sonner` package) — do not build a custom toast system.

```tsx
// Mount once at app root:
<Toaster
  position="bottom-right"
  richColors
  closeButton
  toastOptions={{
    style: {
      borderRadius: "var(--popover-radius)",
      fontFamily: "var(--font-sans)",
    },
  }}
/>
```

| Sonner token | Maps to |
|-------------|---------|
| `--normal-bg` | `var(--color-surface-raised)` |
| `--normal-border` | `var(--color-border)` |
| `--normal-text` | `var(--color-text)` |
| success `--bg` | `var(--color-status-success-subtle)` |
| error `--bg` | `var(--color-status-error-subtle)` |

### 6.10 Tabs

```html
<div role="tablist" class="flex gap-1 border-b border-line">
  <button role="tab" data-[state=active] class="px-3 py-2 text-sm font-medium text-text
                   border-b-2 border-interactive-primary -mb-px">
    Active tab
  </button>
  <button role="tab" class="px-3 py-2 text-sm text-text-muted hover:text-text">
    Inactive tab
  </button>
</div>
```

| Property | Value |
|----------|-------|
| Tab list | horizontal, `border-bottom: 1px solid var(--color-border)` |
| Active tab | `text-text`, `font-medium`, bottom indicator `border-interactive-primary` |
| Inactive | `text-text-muted`, hover `text-text` |
| Indicator animation | sliding underline (CSS `transition-transform`), `--duration-fast` + `--ease-out` |
| Tab panel | `pt-4`, shares parent padding |

### 6.11 Sidebar primitives (shared, from oxios §5)

```ts
export const sidebarPrimitives = {
  itemBase:      "flex items-center w-full text-sm py-2 px-2 gap-3 rounded-md transition-colors",
  itemDense:     "flex items-center w-full text-xs py-1.5 px-2 gap-2 rounded-sm transition-colors",
  itemActive:    "bg-surface-muted text-text font-medium",
  itemInactive:  "text-text-muted hover:text-text hover:bg-surface-muted/50",
  itemCollapsed: "flex items-center justify-center w-9 h-9 rounded-md",
  sectionHeader: "px-2 py-1.5 text-2xs font-medium tracking-wider uppercase text-text-subtle",
  sectionGap:    "mt-2",
  sectionSep:    "my-2 border-t border-line/50",
} as const;
```

All three projects reuse these classes verbatim in their sidebar / navigation shells.

### 6.12 Do / Don't

**Do**
- Use `box-shadow: 0 0 0 1px` for input/select borders (no layout shift on state change).
- Use semantic utilities (`bg-surface`, `text-text-muted`) in components.
- Use `bg-status-success` / `text-status-warning` — not `bg-emerald-500` / `text-amber-700`.
- Reference `--color-hue-*` for user labels.
- Use shared sidebar primitives for any nav.
- Align Button/Input/Select sizes via the shared control scale (§6.1).
- Use `oklch()` *only* inside token files.
- Verify APCA contrast Lc ≥ 60 for body text/surface pairs; Lc ≥ 75 for small captions.

**Don't**
- Don't use CSS `border` on inputs/selects — use `box-shadow` to avoid layout shift.
- Don't hardcode hex / rgb / hsl in components.
- Don't use direct Tailwind palette utilities (`bg-zinc-100`, `text-blue-800`) in components.
- Don't write `dark:` anywhere outside the token layer.
- Don't use `[data-theme="dark"]` selectors (deprecated — use `.dark` class). Exception: `[data-theme="brand-x"]` variant axis (§8.4) is permitted as an independent future axis.
- Don't use inline `style={{}}` for colors.
- Don't use `onMouseEnter` / `onMouseLeave` for hover/active — CSS `:hover` / `:active` only.
- Don't use a serif font as the type system's identity (no Fraunces in v1+).
- Don't use `React.FC`, `React.ElementRef`, `defaultProps` — use modern React patterns (`ComponentPropsWithoutRef`, parameter defaults).

---

## 7. Layout & motion

### 7.1 Breakpoints

| Name | Width | Behavior |
|------|-------|----------|
| `sm`  | 640px | Stack 2-col → 1-col, sidebar → overlay |
| `md`  | 768px | Tablet layout shifts |
| `lg`  | 1024px | Desktop, full sidebar |
| `xl`  | 1280px | Wide content, optional third panel |

Touch target minimum: **44 × 44px**.

### 7.2 Grid

| Project | Pattern |
|---------|---------|
| oximemo card grid | `grid-cols-[repeat(auto-fill,minmax(240px,1fr))]` (§7.2) — uniform height, virtualized via `@tanstack/react-virtual` |
| oxibuilder lobby — `list` | Single column, hairline separators, no motion |
| oxibuilder lobby — `grid` | 1 / 2 / 3 columns responsive |
| oxibuilder lobby — `canvas` | Floating cards; default drift amplitude 12px / period 14s; seed `stable-per-day` for stable positions |
| oxios dashboard | 3-zone layout: sidebar + main + optional inspector panel |

### 7.3 Motion tokens

```css
:root {
  --duration-fast:  120ms;   /* hover, focus, toggle */
  --duration-base:  200ms;   /* dialog, popover */
  --duration-slow:  350ms;   /* modal, page transition */
  --ease-out:       cubic-bezier(0.16, 1, 0.3, 1);
  --ease-in-out:    cubic-bezier(0.4, 0, 0.2, 1);
}
```

`prefers-reduced-motion: reduce` collapses all durations to 0 and disables drift/parallax/lift. oxibuilder canvas mode auto-falls-back to `grid`. oximemo capture overlay shows instantly.

### 7.4 Captures & overlays (oximemo §6)

- Overlay warm-up: window created off-screen with `visible: true` so first show is **≤ 16ms**.
- Open path: `Option`-double-tap → `capture:show` event → overlay becomes focused + ready.
- Save path: input → `create_note` (Rust) → file written → overlay hidden → original focus restored. ≤ 50ms.
- Always dismissible via `Esc`. Always restorable via the menu bar icon.

### 7.5 Canvas drift (oxibuilder §3.6)

```ts
type CanvasParams = {
  drift_amplitude_px: number;   // default 12
  drift_period_s:     number;   // default 14
  seed:               string;   // default "stable-per-day"
};
```

Initial positions computed once per day via simple collision-avoidance pass (no full physics). Drift applied via CSS `transform: translate(...)` keyframes — never JS rAF loops.

---

## 8. Theming & dark mode

### 8.1 Trigger

**`.dark` class on `<html>` is the single light/dark trigger.** `[data-theme="dark"]` is **deprecated** and must be migrated.

```ts
// theme.ts — shared across oxibuilder + oxios; oximemo uses system-default override
export function applyTheme(theme: "light" | "dark") {
  document.documentElement.classList.toggle("dark", theme === "dark");
}

export function initTheme() {
  const saved = localStorage.getItem("oxi-theme");
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  applyTheme(saved === "dark" || (!saved && prefersDark) ? "dark" : "light");
}

export function watchSystemTheme(cb: (t: "light" | "dark") => void): () => void {
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  const listener = () => {
    if (localStorage.getItem("oxi-theme") === null) {
      const t: "light" | "dark" = mq.matches ? "dark" : "light";
      applyTheme(t);
      cb(t);
    }
  };
  mq.addEventListener("change", listener);
  return () => mq.removeEventListener("change", listener);
}
```

### 8.2 FOUC prevention (web projects)

Inline script in `<head>`, **before** Tailwind's CSS is requested:

```html
<script>
  (function () {
    try {
      var t = localStorage.getItem("oxi-theme");
      var d = t === "dark" || (t == null && matchMedia("(prefers-color-scheme: dark)").matches);
      document.documentElement.classList.toggle("dark", d);
    } catch (_) {}
  })();
</script>
```

### 8.3 Tailwind dark variant

Declared once in `theme.css`, **scoped to tokens only**:

```css
@custom-variant dark (&:where(.dark, .dark *));
```

Component code never writes `dark:bg-*`. All theme reactivity flows through semantic tokens that switch inside `.dark { ... }`.

### 8.4 Two-axis extension (future)

If a brand theme is added later (e.g. "low-contrast", "color-blind safe"), use `[data-theme="brand-x"]` as an **independent axis**:

- `.dark` → light/dark axis
- `[data-theme="..."]` → variant axis

Both selectors are orthogonal. The two-tier semantic-token system already absorbs both.

### 8.5 Desktop (oximemo)

Tauri WebKit. `tauri::Window` listens for `NSAppearanceChange` and emits `theme-changed`. JS applies `.dark` on `<html>` of the WebView. macOS menu bar icon flips automatically.

---

## 9. Accessibility

### 9.1 Contrast (APCA, calibrated for compliance)

| Text | Min Lc | Preferred Lc | WCAG 2.x fallback |
|------|--------|--------------|-------------------|
| Display (≥ 36px) | 45 | 60 | 3:1 |
| Heading (24–35px) | 55 | 68 | 3:1 |
| Body (16–23px) | 60 | 75 | 4.5:1 |
| Caption (14–15px, 400) | 75 | 90 | 4.5:1 |
| Caption (14–15px, 700) | 60 | 75 | 4.5:1 |
| Micro label (≤ 13px) | 90 | — | 4.5:1 |

**Dual-check rule.** For legally required accessibility (ADA, EN 301 549), also pass WCAG 2.x 4.5:1 (body) and 3:1 (large text). APCA Lc 60 ≠ WCAG 2.x 4.5:1.

### 9.2 Color independence

- Status is always paired with **icon + label** (oximemo §7.7 OKLCH label bars also have shape variation).
- Hue labels on cards carry text + bar color (never color alone).
- Hover and focus states use both `color` and `outline` / `box-shadow`.

### 9.3 Motion

All decorative motion is disabled under `prefers-reduced-motion: reduce` (oxibuilder §3.6). Functional motion (e.g. capture overlay show) is preserved but with duration 0.

### 9.4 Keyboard

- Focus ring is always visible: `outline: 2px solid var(--color-focus-ring); outline-offset: 2px` — **except form inputs/selects**, which use `--input-shadow-focus` (box-shadow, §6.4) to avoid layout shift.
- No element uses `outline: none` without a replacement focus indicator. Inputs use `focus-visible:outline-none` + `shadow-[var(--input-shadow-focus)]` as their replacement.
- Headless primitives (Base UI / Radix) own focus traps, `aria-*`, and keyboard navigation.

### 9.5 Gamut safety

OKLCH chroma is auto-clamped to sRGB for browsers that lack P3. When the user's display is P3-capable, the same tokens render more vivid without code changes.

---

## 10. Per-project surfaces

### 10.1 oximemo — card grid + macOS overlays

- Stack: React 19 + TypeScript 5 + Vite, **Base UI** (headless) + Tailwind v4, `@tanstack/react-virtual` + `react-query`, zustand, `lucide-react`, `motion`.
- Window chrome: `titleBarStyle: overlay`, custom toolbar with search inline (Arc/Linear pattern).
- Dark mode: follows system + manual toggle (`toggleTheme()`). macOS menu bar icon flips.
- Card grid: `repeat(auto-fill, minmax(240px, 1fr))`, uniform height, virtualized. (oximemo §7.2)
- Hue label: optional 2px left bar (`bg-hue-{name}`); clamp OKLCH input to safe ranges (oximemo §7.7).
- Status colors not used in the main app — oximemo has no "running/failed" semantics. They are defined in tokens for future use.
- **Typography:** SUIT (body), SUITE (display). No prior font in `doc/DESIGN.md` — this is the first definition.

### 10.2 oxibuilder — multi-extension personal site

- Stack: React 19 + Vite + Tailwind v4, Radix primitives (a11y), shadcn-style local-owned components.
- **Fonts: migrating** Pretendard → SUIT, Fraunces → SUITE. Pretendard/Fraunces references in `web/src/**` are replaced by SUIT/SUITE; line-heights re-tuned to SUIT's metrics (Pretendard and SUIT differ by ~2% x-height — verify the body line-height after swap).
- Lobby modes: `list`, `grid`, `canvas`. Canvas defaults: amplitude 12px / period 14s / seed "stable-per-day". Reduced-motion → `grid`.
- Status colors: not in primary use. Surface `success`/`error` for form feedback only.
- Console chrome sidebar: **deprecated legacy accent** (green `#22c55e`) — kept for v1, removed in v2. See §12.2.
- **Theme trigger migration:** `[data-theme]` → `.dark` (see §12.2).

### 10.3 oxios — agent operating system dashboard

- Stack: React 19 + Vite + Tailwind v4, **Base UI** + shadcn-style components, TanStack Query, TanStack Router, Zustand.
- Status colors: primary mechanism. Every agent state surfaces as `text-status-{success|warning|error|info}` with paired icon + label.
- Sidebar modes: Console / Knowledge / Chat share `sidebarPrimitives` (§6.5).
- Density: `gap-2` (8px) is the default rhythm; section-level spacing is `gap-4` (16px).
- **Fonts: migrating** Geist → SUIT (body), adding SUITE (display hero only). v1 keeps Geist as fallback. **Geist is referenced 10 times across 3 files** in `web/src/` (`index.css` ×5, `tokens/index.ts` ×2, `editor-prefs.ts` ×3); zero references in component `.tsx` files. Migration is low-risk — see §12.3.
- **Theme trigger:** `.dark` retained (already correct). Sweep scattered `dark:` literals into semantic tokens. **Storage key:** currently `oxios-theme`; migrate to `oxi-theme` alongside oxibuilder (§12.2 step 2).

### 10.4 Agent prompt guide

Quick token reference (paste into agent prompts):

| Need | Use |
|------|-----|
| Page background | `bg-surface text-text` |
| Card | `bg-surface-raised text-text border border-line rounded-lg shadow-sm` |
| Muted text | `text-text-muted` |
| Subtle text | `text-text-subtle` |
| Status success | `text-status-success bg-status-success-subtle` |
| Status warning | `text-status-warning bg-status-warning-subtle` |
| Status error | `text-status-error bg-status-error-subtle` |
| Status info | `text-status-info bg-status-info-subtle` |
| Note label | `bg-hue-{red\|amber\|green\|teal\|blue\|purple}` |
| Focus ring | Buttons/links: `focus-visible:outline-2 focus-visible:outline-focus-ring focus-visible:outline-offset-2`. **Form inputs/selects:** `focus-visible:outline-none shadow-[var(--input-shadow-focus)]` (§6.4) |
| Sidebar item | Use shared `sidebarPrimitives` (§6.5) |
| Display heading | `font-display` (resolves to SUITE) |

**Forbidden tokens in agent code:** `bg-zinc-*`, `text-gray-*`, `text-blue-800`, `dark:bg-*`, `style={{ color: ... }}` for theme values, `[data-theme="dark"]` (use `.dark` class instead; `[data-theme="brand-x"]` variant axis is permitted per §8.4), `'Geist'` sans (until migration lands in §12.3; `'Geist Mono'` is retained).

---

## 11. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-31 | Unified `DESIGN.md` authored in `project-oxi/` | Three projects converge on one grammar; per-project docs now reference this. |
| 2026-07-31 | Six-hue OKLCH label palette shared; site accent removed | oximemo §7.7 already defined this; oxios status uses the same hue families. No competing "brand color." |
| 2026-07-31 | `.dark` class is the single light/dark trigger | Tailwind/shadcn ecosystem default; `data-theme` selector deprecated. Two-tier semantic tokens absorb the switch. |
| 2026-07-31 | `dark:` variant forbidden in component dirs | All theme reactivity flows through semantic tokens. Lint rule to enforce. |
| 2026-07-31 | SUIT + SUITE as unified Korean type pairing — both sans, density contrast | oxibuilder migrates from Pretendard/Fraunces; oximemo + oxios adopt SUIT. No serif in the system. |
| 2026-07-31 | L inversion only — never adjust C for contrast | Maintains perceptual uniformity across themes. |
| 2026-07-31 | Fonts loaded via jsDelivr (sun-typeface GitHub mirror), not Google Fonts | Confirmed via live CSS inspection: SUIT/SUITE have no Google Fonts presence. |
| 2026-07-31 | oximemo typography defined for the first time | `doc/DESIGN.md` had no font/scale before; SUIT body + SUITE display + Geist Mono fills the gap. |
| 2026-07-31 | oxios Geist → SUIT migration scoped accurately | 10 Geist references in 3 files (`index.css`, `tokens/index.ts`, `editor-prefs.ts`); zero in `.tsx` components. Low-risk migration. |
| 2026-07-31 | `--color-interactive-primary` added as dedicated button fill | Label hues (L≈0.70) are too light for white-text button fills; dedicated token at L=0.45 (light) / L=0.70 (dark) passes APCA Lc 60. |
| 2026-07-31 | `--color-status-*-on-subtle` text variants added | Status text on subtle surfaces needs darker (light) / lighter (dark) variants to meet Lc 75+ for 10px micro labels. |
| 2026-07-31 | `--color-text-subtle` corrected: L=0.42 light, L=0.75 dark | Previous L=0.55 (light) and L=0.65 (dark) failed APCA Lc 60; dark comment falsely claimed "AA." |
| 2026-07-31 | Component → radius mapping added (§5.2.1) | Every component references an explicit `--{component}-radius` tier token, never a raw `px` value. |
| 2026-07-31 | L-only rule scoped: neutrals + label hues only; status hues APCA-optimized per-mode | Status colors shift C/H between light/dark by design (verbatim oxios dashboard values); the blanket "never adjust C/H" was contradicted by the doc's own values. |

---

## 12. Migration plan

### 12.1 oximemo (system light/dark → `.dark` parity, SUIT adoption)

1. **Tokens.** `apps/desktop/src/lib/color.ts` continues to host the OKLCH clamp helper. Move the hue palette into `tokens/primitives.css` so `bg-hue-*` utilities are available alongside.
2. **Tauri WebView.** On `NSAppearanceChange`, emit a Tauri event; JS applies `.dark` to `<html>`. macOS title bar follows the OS setting.
3. **Fonts.** Bundle SUIT + SUITE woff2 in `apps/desktop/src/assets/fonts/`. Declare `--font-sans` and `--font-display` per §4.3.
4. **No `dark:` sweep required** — oximemo's React code base is small enough that dark mode is mostly system-driven.

### 12.2 oxibuilder (`[data-theme]` → `.dark`, Pretendard/Fraunces → SUIT/SUITE, legacy green sidebar)

1. **Tokens.** Replace `web/src/shared/tokens.css` with the tier layout from §2.2.
   - Move `[data-theme="light"]` → `:root`.
   - Move `[data-theme="dark"]` → `.dark`.
   - Add 6-hue label palette + status tokens from §3.
2. **Theme module.** Replace `theme.ts` with the canonical version from §8.1 (storage key `oxi-theme`).
3. **FOUC.** Insert inline `<head>` script from §8.2.
4. **Fonts.** Replace `--font-body` with `SUIT Variable` (Pretendard → SUIT). Replace `--font-display` with `SUITE Variable` (Fraunces → SUITE). Bundle via `web/public/fonts/` and `@import` the jsDelivr CSS, or self-host woff2.
   - **Line-height recalibration.** Pretendard's x-height is ~2% taller than SUIT. Sweep `line-height` values: previous 1.55 → 1.50 on body, 1.5 → 1.45 on small body. Visual diff required.
5. **Component sweep.** Remove every `dark:` from `web/src/**/*.{tsx,ts}`. Replace `data-theme="dark"` listeners with `MutationObserver` on `class="dark"`.
6. **Legacy compatibility.** During transition, mirror `.dark` → `[data-theme="dark"]` via a `MutationObserver` on `<html>`. Remove in next minor release.
7. **Console chrome.** The hard-coded green sidebar (`#22c55e` etc.) is **kept for v1**; replaced with `bg-status-success` in v2. This is the only place oxibuilder keeps a project-local accent.

### 12.3 oxios (`.dark` retained, scattered `dark:` → semantic tokens, Geist → SUIT, storage key unification)

1. **Tokens.** Already structured; re-examine `web/src/index.css` to ensure semantic tokens are *the only* place `.dark` overrides appear.
2. **`dark:` sweep.** Audit every `dark:bg-*`, `dark:text-*`, `dark:border-*` in `web/src/components/**`. Replace with semantic utilities. Count unknown until sweep runs — but Geist references (10 total, 3 files, zero in `.tsx`) are separate from `dark:` variants. Do not conflate the two.
3. **Lint rule.** Add `no-restricted-syntax` for `dark:` literals in component files (allow in `tokens/` and `design-system/` only).
4. **Storage key unification.** oxios currently uses `oxios-theme` (default `'dark'`); the canonical module (§8.1) uses `oxi-theme`. Migrate `web/src/stores/theme.ts` to read/write `oxi-theme`. Ship a one-time migration: on boot, if `oxios-theme` exists in localStorage, copy to `oxi-theme` and delete the old key.
5. **Font migration — single-phase (low-risk).**
   - Geist is referenced **10 times across 3 files**: `index.css` (×5: comment + `--editor-font-body`, `--editor-font-mono`, body `font-family`, `kbd` font), `tokens/index.ts` (×2: `sans` + `mono`), `editor-prefs.ts` (×3: FONT_PRESETS entry, DEFAULTS `fontFamily`, version comment). **Zero references in component `.tsx` files.**
   - Replace all 10 in one pass. Update `--font-sans` to `'SUIT Variable', system-ui, -apple-system, sans-serif`. Update `--font-mono` to keep `'Geist Mono'` (mono stays — SUIT has no monospace variant).
   - **SUITE adoption.** Surface `--font-display` (SUITE) in dashboard hero areas only (dashboard title, empty-state hero text). Do **not** retrofit existing headings — visual diff is low value.
6. **Editor font preset.** `web/src/stores/editor-prefs.ts` exposes `FONT_PRESETS`. Actions:
   - Add `'SUIT Variable'` preset.
   - **Remove the `'Serif'` preset** (`ui-serif, Georgia, 'Times New Roman', serif`) — contradicts §4.1 "no serif in this system." If users need a serif reading font, scope it as a content-only preference with an explicit note that it is outside the design system.
   - Mark `'Geist Sans'` deprecated; remove in v2.
7. **CSS imports.** Replace Google Fonts `<link>` for Geist in `web/index.html` with the jsDelivr SUIT import. Keep the `Geist Mono` `<link>` (mono is retained).

### 12.4 Rollout order

| Step | Project | Verify |
|------|---------|--------|
| 1 | oxibuilder tokens rewrite | Smoke test lobby modes; APCA check on body/heading |
| 2 | oxibuilder font migration | Visual diff against Pretendard baseline; line-height recalibration pass |
| 3 | oxios lint rule + `dark:` sweep (per-component) | Component tests pass; visual snapshot diff |
| 4 | oxios font migration (single-pass, 10 refs in 3 files) | Latin UI unchanged; SUIT loads from jsDelivr |
| 5 | oxios storage key unification (`oxios-theme` → `oxi-theme`) | One-time migration on boot; old key deleted |
| 6 | oxios editor-prefs Serif preset removed | No serif option remains in FONT_PRESETS |
| 7 | oximemo `.dark` parity | Overlay warm-up timing unchanged |
| 8 | oximemo SUIT adoption | macOS native chrome unaffected |
| 9 | Remove `[data-theme="dark"]` legacy observers across all projects | All consumers updated to `.dark` only |
