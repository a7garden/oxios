/**
 * Single source of truth for agent status visual treatment.
 *
 * All A2A / agent status displays (card list, node, inspector,
 * minimap) read from this palette so a status name maps to a
 * consistent color across surfaces. The original code defined
 * three near-duplicate `STATUS_COLOR` / `statusColors` / `statusBorder`
 * maps that disagreed on which keys existed (`active` only in
 * agent-inspector, `pending` only in agent-node, etc.) — see
 * review P1-1.
 */
import { cssVarToRgb } from '@/lib/utils'

/** Visual style for a single agent status. */
export interface StatusStyle {
  /** Tailwind class for the agent's border (e.g. `border-success`). */
  border: string
  /** Tailwind class for the agent's status dot (e.g. `bg-success animate-pulse`). */
  dot: string
  /**
   * Resolved CSS color (rgb string) suitable for canvases (React Flow's
   * `nodeColor` / `nodeStrokeColor`). Kept in sync with the Tailwind
   * classes above; resolved at module load via `cssVarToRgb` so it follows
   * the current theme (light/dark) automatically.
   */
  hex: string
}

/**
 * Canonical mapping of agent status → visual style.
 *
 * Keys are lowercase. Any status not in this map falls back to
 * [`DEFAULT_STATUS_STYLE`].
 *
 * NOTE: `hex` values are Tailwind v3 palette defaults kept for
 * React Flow's MiniMap (which needs plain CSS colors — not OKLCH).
 * They are intentionally kept in sync with the semantic tokens
 * in index.css (--success, --warning, --error, --info) by visual
 * equivalence, not exact value match.
 */
export const STATUS_PALETTE: Record<string, StatusStyle> = {
  running: {
    border: 'border-success',
    dot: 'bg-success animate-pulse',
    hex: cssVarToRgb('--color-status-success'),
  },
  active: {
    border: 'border-success',
    dot: 'bg-success animate-pulse',
    hex: cssVarToRgb('--color-status-success'),
  },
  idle: {
    border: 'border-warning',
    dot: 'bg-warning',
    hex: cssVarToRgb('--color-status-warning'),
  },
  pending: {
    border: 'border-warning',
    dot: 'bg-warning',
    hex: cssVarToRgb('--color-status-warning'),
  },
  starting: {
    border: 'border-info',
    dot: 'bg-info',
    hex: cssVarToRgb('--color-status-info'),
  },
  stopped: {
    border: 'border-error',
    dot: 'bg-error',
    hex: cssVarToRgb('--color-status-error'),
  },
  failed: {
    border: 'border-destructive',
    dot: 'bg-destructive',
    hex: cssVarToRgb('--color-status-error'),
  },
  error: {
    border: 'border-destructive',
    dot: 'bg-destructive',
    hex: cssVarToRgb('--color-status-error'),
  },
  archived: {
    border: 'border-muted-foreground',
    dot: 'bg-muted-foreground',
    hex: cssVarToRgb('--color-text-muted'),
  },
  rejected: {
    border: 'border-destructive',
    dot: 'bg-destructive',
    hex: cssVarToRgb('--color-status-error'),
  },
}

/** Fallback style used when a status is not in the palette. */
export const DEFAULT_STATUS_STYLE: StatusStyle = {
  border: 'border-border',
  dot: 'bg-muted-foreground',
  hex: cssVarToRgb('--color-text-muted'),
}

/** Tailwind class for the border of an agent with the given status. */
export function statusBorder(status: string): string {
  return STATUS_PALETTE[status]?.border ?? DEFAULT_STATUS_STYLE.border
}

/** Tailwind class for the dot/indicator of an agent with the given status. */
export function statusDot(status: string): string {
  return STATUS_PALETTE[status]?.dot ?? DEFAULT_STATUS_STYLE.dot
}

/**
 * Hex color for an agent with the given status. Used by React Flow's
 * `<MiniMap>` (which does not accept Tailwind classes — it needs a
 * CSS color value for canvas rendering).
 */
export function statusColor(status: string): string {
  return STATUS_PALETTE[status]?.hex ?? DEFAULT_STATUS_STYLE.hex
}
