# Design System Token Unification

**Date:** 2026-06-06
**Scope:** `surface/oxios-web/web`
**Status:** Approved

## Problem

The design system has CSS variables (`--destructive`, `--message-*`) and an OKLCH color foundation, but 120+ components bypass semantic tokens by using Tailwind color classes directly (`text-emerald-500`, `bg-red-500`, etc.). This:

- Breaks theme consistency (dark mode requires manual `dark:` pairs per color)
- Makes global color changes impossible without find-and-replace
- Undermines the token system that's already in place

Additionally, 64 instances use non-standard font sizes (`text-[10px]` × 59, `text-[11px]` × 5) instead of a named token.

## Decisions

### 1. Add 4 semantic color tokens — 3 levels each

Add `success`, `warning`, `error`, `info` to the CSS variable system, each at 3 intensity levels:

- **base** — text, progress bars, solid backgrounds, dots
- **subtle** — info panels, highlighted blocks (replaces `bg-{color}-50 dark:bg-{color}-950`)
- **muted** — badge backgrounds, subtle fills (replaces `bg-{color}-500/15`)

Each level has `text-*`, `bg-*`, and `border-*` variants via Tailwind `@theme`.

#### Text (base) — primary foreground for each status

| Token      | Light (OKLCH)                            | Dark (OKLCH)                             |
|------------|------------------------------------------|------------------------------------------|
| `success`  | `oklch(0.596 0.145 163.)` ≈ emerald-600 | `oklch(0.723 0.219 149.579)` ≈ emerald-400 |
| `warning`  | `oklch(0.669 0.162 70.)` ≈ amber-600    | `oklch(0.769 0.188 70.08)` ≈ amber-400    |
| `error`    | `oklch(0.577 0.245 27.325)` ≈ red-500   | `oklch(0.704 0.191 22.216)` ≈ red-400     |
| `info`     | `oklch(0.623 0.214 259.815)` ≈ blue-500 | `oklch(0.685 0.196 259.)` ≈ blue-400      |

#### Background — base (solid, for progress bars, dots, indicators)

| Token          | Light                        | Dark                         |
|----------------|------------------------------|------------------------------|
| `success`      | `oklch(0.696 0.17 162.48)` ≈ emerald-500 | `oklch(0.696 0.17 162.48)` |
| `warning`      | `oklch(0.769 0.188 70.08)` ≈ amber-500   | `oklch(0.769 0.188 70.08)` |
| `error`        | `oklch(0.577 0.245 27.325)` ≈ red-500    | `oklch(0.577 0.245 27.325)` |
| `info`         | `oklch(0.623 0.214 259.815)` ≈ blue-500  | `oklch(0.623 0.214 259.815)` |

#### Background — subtle (for info panels, callout blocks)

Replaces `bg-{color}-50 dark:bg-{color}-950` pattern.

| Token            | Light                         | Dark                                  |
|------------------|-------------------------------|---------------------------------------|
| `success-subtle` | `oklch(0.97 0.02 163.)` ≈ emerald-50  | `oklch(0.18 0.02 163.)` ≈ emerald-950  |
| `warning-subtle` | `oklch(0.97 0.02 70.)` ≈ amber-50     | `oklch(0.18 0.02 70.)` ≈ amber-950     |
| `error-subtle`   | `oklch(0.97 0.02 27.)` ≈ red-50       | `oklch(0.18 0.02 27.)` ≈ red-950       |
| `info-subtle`    | `oklch(0.97 0.02 259.)` ≈ blue-50     | `oklch(0.18 0.02 259.)` ≈ blue-950     |

#### Background — muted (for badge fills, subtle highlights)

Replaces `bg-{color}-500/15` pattern. Uses the base color at 15% opacity via CSS `color-mix()`.

| Token            | Value                                      |
|------------------|--------------------------------------------|
| `success-muted`  | `color-mix(in oklch, var(--color-success) 15%, transparent)` |
| `warning-muted`  | `color-mix(in oklch, var(--color-warning) 15%, transparent)` |
| `error-muted`    | `color-mix(in oklch, var(--color-error) 15%, transparent)`   |
| `info-muted`     | `color-mix(in oklch, var(--color-info) 15%, transparent)`    |

Note: muted backgrounds are the same in both light and dark mode (15% of the base color).

#### Border — subtle (for info panels)

Replaces `border-{color}-200 dark:border-{color}-900`.

| Token             | Light                              | Dark                                |
|-------------------|------------------------------------|-------------------------------------|
| `success-subtle-border` | ≈ emerald-200               | ≈ emerald-900                       |
| `warning-subtle-border` | ≈ amber-200                  | ≈ amber-900                         |
| `error-subtle-border`   | ≈ red-200                    | ≈ red-900                           |
| `info-subtle-border`    | ≈ blue-200                   | ≈ blue-900                          |

**Total new tokens: 4 colors × (base-text + base-bg + subtle-bg + muted-bg + subtle-border) = 20 CSS variables.**

### 2. Add `text-2xs` custom size token

```css
--text-2xs: 10px;
```

Maps to Tailwind utility `text-2xs`. Replaces all `text-[10px]` (59) and `text-[11px]` (5).

> ⚠️ **Usage guideline**: `text-2xs` is for micro labels, badges, and inline metadata only.
> Do NOT use for body text, button labels, or any text intended for comfortable reading.
> Prefer `text-xs` (12px) for most small text.

### 3. Update component variants

**badge.tsx:**
```diff
- success: 'border-transparent bg-emerald-500/15 text-emerald-700 dark:text-emerald-400'
+ success: 'border-transparent bg-success-muted text-success'
- warning: 'border-transparent bg-amber-500/15 text-amber-700 dark:text-amber-400'
+ warning: 'border-transparent bg-warning-muted text-warning'
```

**sonner.tsx:**
```diff
- 'border-emerald-500/50 bg-emerald-500/15 text-emerald-700 dark:text-emerald-400'
+ 'border-success/50 bg-success-muted text-success'
```

### 4. Replacement rules

#### 4a. Status color replacements (UI status only)

Applied to components where the color means success/warning/error/info status.

**Full panel pattern** (`bg-{c}-50 dark:bg-{c}-950 border-{c}-200 dark:border-{c}-900 text-{c}-700 dark:text-{c}-400`):

This 4-class pattern appears in `settings.tsx`, `system-update.tsx`, `system-tools.tsx`, `restart-badge.tsx`, `diff-preview.tsx`. Collapses to:
```
bg-success-subtle border-success-subtle text-success
```

**Badge/text pattern** (`text-{c}-{N} dark:text-{c}-{M}`):

| Light + Dark pair                          | Replacement        |
|--------------------------------------------|--------------------|
| `text-emerald-600 dark:text-emerald-400`   | `text-success`     |
| `text-emerald-700 dark:text-emerald-400`   | `text-success`     |
| `text-emerald-500` (no dark pair)          | `text-success`     |
| `text-amber-600 dark:text-amber-400`       | `text-warning`     |
| `text-amber-700 dark:text-amber-400`       | `text-warning`     |
| `text-amber-500` (no dark pair)            | `text-warning`     |
| `text-red-600 dark:text-red-400`           | `text-error`       |
| `text-red-500` (no dark pair)              | `text-error`       |
| `text-blue-500 dark:text-blue-400`         | `text-info`        |
| `text-blue-500` (no dark pair)             | `text-info`        |

**Background (solid)** — progress bars, dots, indicators:

| Pattern            | Replacement    |
|--------------------|----------------|
| `bg-emerald-500`   | `bg-success`   |
| `bg-amber-500`     | `bg-warning`   |
| `bg-red-500`       | `bg-error`     |
| `bg-blue-500`      | `bg-info`      |

**Background (subtle)** — info panels, callout blocks:

| Pattern (light + dark)                        | Replacement           |
|------------------------------------------------|-----------------------|
| `bg-emerald-50 dark:bg-emerald-950`            | `bg-success-subtle`   |
| `bg-amber-50 dark:bg-amber-950`                | `bg-warning-subtle`   |
| `bg-red-50 dark:bg-red-950`                    | `bg-error-subtle`     |
| `bg-blue-50 dark:bg-blue-950`                  | `bg-info-subtle`      |

**Background (muted)** — badge fills:

| Pattern              | Replacement         |
|----------------------|---------------------|
| `bg-emerald-500/15`  | `bg-success-muted`  |
| `bg-amber-500/15`    | `bg-warning-muted`  |
| `bg-red-500/15`      | `bg-error-muted`    |
| `bg-blue-500/10`     | `bg-info-muted`     |

**Border (subtle)**:

| Pattern (light + dark)                        | Replacement                  |
|------------------------------------------------|------------------------------|
| `border-emerald-200 dark:border-emerald-900`   | `border-success-subtle`      |
| `border-amber-200 dark:border-amber-900`       | `border-warning-subtle`      |
| `border-red-200 dark:border-red-900`           | `border-error-subtle`        |
| `border-blue-200 dark:border-blue-900`         | `border-info-subtle`         |

#### 4b. Opacity modifier preservation

For cases where a non-standard opacity is used on a status background:

| Pattern                  | Replacement          |
|--------------------------|----------------------|
| `bg-emerald-500/80`      | `bg-success/80`      |
| `bg-emerald-500/10`      | `bg-success/10`      |
| `border-emerald-500/40`  | `border-success/40`  |
| `border-emerald-500/50`  | `border-success/50`  |

These use the base token + Tailwind's opacity modifier. Works because the token is a solid color.

#### 4c. Exceptions — NOT replaced

These use emerald/amber/red/blue for **domain-specific** purposes, not UI status:

| Component | Color | Meaning | Reason |
|-----------|-------|---------|--------|
| `habits.tsx` mood colors array | `bg-emerald-400` | 5th mood level (gradient) | Mood scale, not success |
| `habits.tsx` habit dot | `bg-emerald-500/80` | Habit completion | Borderline — kept for now (see §5) |
| `stat-card.tsx` SparkColor | `emerald` stroke/fill | Chart series color | Data visualization |
| `calendar-view.tsx` event source | `bg-blue-500` | Agent-sourced events | Domain color |
| `calendar/event-chip.tsx` | `text-purple-800` | Calendar event type | Domain color |
| `tier-badge.tsx` | hot=red, warm=yellow, cold=blue | Memory tier | Domain color |
| `agent-logs.tsx` | info=blue, warn=yellow, error=red | Log level | Domain color |
| `cluster-legend.tsx` | `bg-amber-500` | Cluster category | Domain color |
| `notification-bell.tsx` severity | `bg-blue-500` etc | Notification type | Already maps to status semantically — **DO replace** |
| `project-card.tsx` source map | `bg-emerald-100 text-emerald-700` | Source type | Domain color (manual vs auto_detected) |
| `engine/api-key-input.tsx` status | `text-emerald-600` etc | Key validation status | **DO replace** — this IS status |
| `budget/agent-budget-card.tsx` progress | gradient red→amber→blue | Threshold levels | **DO replace** — these ARE status indicators |
| `a2a/message-log.tsx` | `text-emerald-600` | Result message | Already covered by `--message-result` token |

### 5. Replacement per-file checklist

Each file categorized as **replace** (status semantics) or **keep** (domain):

**REPLACE (status color → token):**
- `badge.tsx` — success/warning variants
- `sonner.tsx` — success variant
- `browse-context-badge.tsx` — HTTP status colors, search/extraction context colors
- `browse-context-detail.tsx` — progress bar
- `group-card.tsx` — Completed/Failed progress bar
- `group-progress.tsx` — completion progress bar
- `chat-metadata.tsx` — passed/failed status
- `connection-status.tsx` — connected indicator
- `notification-bell.tsx` — severity dots
- `live-activity-feed.tsx` — active indicator
- `header.tsx` — connection status dot
- `restart-badge.tsx` — restart status panel
- `system-update.tsx` — update status panel
- `system-tools.tsx` — tool status panel
- `settings.tsx` — info panel
- `diff-preview.tsx` — diff status labels
- `scheduler.tsx` — status icons
- `approvals-queue.tsx` — approval status
- `skills.tsx` — skill status badges, info panels
- `skill-detail.tsx` — requirement check marks
- `update-badge.tsx` — update status
- `routes/index.tsx` — health indicator dots
- `routes/events.tsx` — active event dot
- `routes/chat.tsx` — message status
- `routes/approvals.tsx` — approval status
- `budget/budget-summary.tsx` — threshold progress bar
- `budget/agent-budget-card.tsx` — budget threshold progress bars
- `engine/api-key-input.tsx` — key validation status
- `engine/model-select.tsx` — model capability icons
- `engine/provider-select.tsx` — provider status icon
- `a2a/message-log.tsx` — result/query message kinds
- `knowledge/knowledge-chat.tsx` — citation check icon

**KEEP (domain color, no change):**
- `habits.tsx` — mood color gradient array, habit grid dots (borderline, kept for domain consistency)
- `stat-card.tsx` — SparkColor chart series, delta text (data viz)
- `calendar/calendar-view.tsx` — event source colors, grid layout
- `calendar/event-chip.tsx` — event type colors
- `calendar/conflict-warning.tsx` — conflict panel
- `memory/tier-badge.tsx` — memory tier colors
- `memory/cluster-legend.tsx` — cluster colors
- `agent/agent-logs.tsx` — log level colors
- `project/project-card.tsx` — source type map
- `project/ai-detection-badge.tsx` — AI detection badge
- `project/edit-project-dialog.tsx` — (inherits project-card source map)
- `routes/projects/$projectId.tsx` — source type map

### 6. Inline style cleanup

Replace static hex colors in `style={{}}` with CSS classes:

| File                  | Before                          | After               |
|-----------------------|---------------------------------|---------------------|
| `calendar-view.tsx`   | `style={{ background: '#10b981' }}` | `className="bg-success"` |
| `calendar-view.tsx`   | `style={{ background: '#f59e0b' }}` | `className="bg-warning"` |
| `cluster-legend.tsx`  | `style={{ background: '#71717a' }}` | `className="bg-zinc-500"` |

Dynamic styles (`width: ${pct}%`, `paddingLeft: ${depth * 16}px`, grid layout) are left as-is.

### 7. Scope exclusions

- **border-radius**: Current distribution is acceptable. No changes.
- **emoji.ts / emoji-shortcodes.ts**: Content-layer (user markdown), not UI. Excluded.
- **markdown-editor.tsx**: Autocomplete for user content. Excluded.

## Implementation plan

Single-pass execution:

1. Add CSS variables to `src/index.css` (`:root` and `.dark`) — 20 color vars + 1 size var
2. Register all new tokens in `@theme inline` block
3. Update `badge.tsx` success/warning variants
4. Update `sonner.tsx` success variant
5. Bulk replace colors across REPLACE-listed files only (not KEEP files)
6. Replace `text-[10px]` / `text-[11px]` → `text-2xs` across all files
7. Clean inline styles in calendar-view, cluster-legend
8. `tsc --noEmit` + `bun run build` verification

## Files touched

- `src/index.css` — token definitions
- `src/components/ui/badge.tsx` — variant update
- `src/components/ui/sonner.tsx` — variant update
- ~30 component/route files — color replacements (REPLACE list)
- ~30 files — `text-[10px]` → `text-2xs` replacement
- `src/components/calendar/calendar-view.tsx` — inline style cleanup
- `src/components/memory/cluster-legend.tsx` — inline style cleanup
