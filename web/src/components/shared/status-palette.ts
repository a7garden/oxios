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

/** Visual style for a single agent status. */
export interface StatusStyle {
  /** Tailwind class for the agent's border (e.g. `border-emerald-500`). */
  border: string
  /** Tailwind class for the agent's status dot (e.g. `bg-emerald-500 animate-pulse`). */
  dot: string
  /**
   * Hex color suitable for canvases (React Flow's `nodeColor` /
   * `nodeStrokeColor`). Kept in sync with the Tailwind classes
   * above; uses the Tailwind v3 palette defaults.
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
    hex: '#10b981',
  },
  active: {
    border: 'border-success',
    dot: 'bg-success animate-pulse',
    hex: '#10b981',
  },
  idle: {
    border: 'border-warning',
    dot: 'bg-warning',
    hex: '#f59e0b',
  },
  pending: {
    border: 'border-warning',
    dot: 'bg-warning',
    hex: '#f59e0b',
  },
  starting: {
    border: 'border-info',
    dot: 'bg-info',
    hex: '#3b82f6',
  },
  stopped: {
    border: 'border-error',
    dot: 'bg-error',
    hex: '#ef4444',
  },
  failed: {
    border: 'border-destructive',
    dot: 'bg-destructive',
    hex: '#ef4444',
  },
  error: {
    border: 'border-destructive',
    dot: 'bg-destructive',
    hex: '#ef4444',
  },
  archived: {
    border: 'border-muted-foreground',
    dot: 'bg-muted-foreground',
    hex: '#a1a1aa',
  },
  rejected: {
    border: 'border-destructive',
    dot: 'bg-destructive',
    hex: '#ef4444',
  },
}

/** Fallback style used when a status is not in the palette. */
export const DEFAULT_STATUS_STYLE: StatusStyle = {
  border: 'border-border',
  dot: 'bg-muted-foreground',
  hex: '#a1a1aa',
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
