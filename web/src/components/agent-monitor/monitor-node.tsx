/**
 * Monitor node — enhanced React Flow custom node for the unified agent canvas.
 *
 * Shows the agent name, a pulsing status dot, and duration. On hover the
 * card expands to reveal cost, tokens, model, and A2A capability chips.
 * Clicking opens the detail panel (via onSelect callback).
 */

import { memo } from 'react'
import { Handle, Position } from 'reactflow'
import { statusBorder, statusDot } from '@/components/shared/status-palette'
import { cn } from '@/lib/utils'
import type { MonitorNode as MonitorNodeType } from '@/types/agent-monitor'

/** Data injected by the canvas into each React Flow node. */
export interface MonitorNodeData extends MonitorNodeType {
  selected: boolean
  onSelect: (id: string) => void
}

/** Format seconds → human duration (e.g. "3m 12s"). */
function formatDuration(secs: number | null): string {
  if (secs == null) return '—'
  if (secs < 60) return `${Math.floor(secs)}s`
  const m = Math.floor(secs / 60)
  const s = Math.floor(secs % 60)
  if (m < 60) return s > 0 ? `${m}m ${s}s` : `${m}m`
  const h = Math.floor(m / 60)
  return `${h}h ${m % 60}m`
}

/** Compact cost formatter — $0.018, or "—" for zero. */
function formatCost(usd: number): string {
  return usd > 0 ? `$${usd.toFixed(usd < 0.01 ? 4 : 3)}` : '—'
}

/** Compact token formatter — 48.2k, 1.2M. */
function formatTokens(n: number): string {
  if (n === 0) return '—'
  if (n < 1000) return String(n)
  if (n < 1_000_000) return `${(n / 1000).toFixed(1)}k`
  return `${(n / 1_000_000).toFixed(1)}M`
}

function MonitorNodeInner({ data }: { data: MonitorNodeData }) {
  const { name, displayStatus, lifecycle, a2a, selected, onSelect, agentId } = data
  const isRunning = displayStatus === 'running'

  return (
    // biome-ignore lint/a11y/useSemanticElements: React Flow custom node container must remain a div to preserve connection handles and DnD
    <div
      className={cn(
        'group relative w-[200px] cursor-pointer rounded-lg border bg-card p-3 shadow-sm',
        'transition-all duration-200 ease-[var(--animate-in-easing)]',
        'hover:shadow-lg hover:border-primary/40',
        statusBorder(displayStatus),
        selected && 'ring-2 ring-primary/50 shadow-md',
      )}
      onClick={() => onSelect(agentId)}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          onSelect(agentId)
        }
      }}
      role="button"
      tabIndex={0}
      aria-label={`Agent ${name}, ${displayStatus}`}
    >
      {/* React Flow connection handles (hidden — edges are A2A-derived) */}
      <Handle type="target" position={Position.Top} className="!h-0 !w-0 !border-0 !opacity-0" />
      <Handle type="source" position={Position.Bottom} className="!h-0 !w-0 !border-0 !opacity-0" />

      {/* Header: status dot + name */}
      <div className="flex items-center gap-2">
        <span
          className={cn('h-2 w-2 shrink-0 rounded-full', statusDot(displayStatus))}
          aria-hidden="true"
        />
        <span className="truncate text-sm font-medium" title={name}>
          {name}
        </span>
      </div>

      {/* Subtitle: status + duration */}
      <div className="mt-1 flex items-center gap-1.5 text-2xs text-muted-foreground">
        <span className={cn('font-medium', isRunning && 'text-success')}>{displayStatus}</span>
        <span aria-hidden="true">·</span>
        <span className="font-mono">
          {isRunning && lifecycle.duration_secs == null ? (
            <span className="text-success">running…</span>
          ) : (
            formatDuration(lifecycle.duration_secs)
          )}
        </span>
      </div>

      {/* Hover-reveal metrics (expand on group-hover) */}
      <div className="grid grid-rows-[0fr] transition-all duration-200 ease-[var(--animate-in-easing)] group-hover:grid-rows-[1fr]">
        <div className="overflow-hidden">
          <div className="mt-2 space-y-1.5 border-t pt-2">
            {/* Cost + tokens */}
            <div className="flex items-center justify-between text-2xs">
              <span className="text-muted-foreground">Cost</span>
              <span className="font-mono">{formatCost(lifecycle.cost_usd)}</span>
            </div>
            <div className="flex items-center justify-between text-2xs">
              <span className="text-muted-foreground">Tokens</span>
              <span className="font-mono">{formatTokens(lifecycle.tokens_used)}</span>
            </div>
            {lifecycle.model_id && (
              <div className="flex items-center justify-between text-2xs">
                <span className="text-muted-foreground">Model</span>
                <span
                  className="truncate font-mono text-muted-foreground"
                  title={lifecycle.model_id}
                >
                  {lifecycle.model_id}
                </span>
              </div>
            )}

            {/* A2A capability chips (if registered) */}
            {a2a && a2a.capabilities.length > 0 && (
              <div className="flex flex-wrap gap-1 pt-1">
                {a2a.capabilities.slice(0, 4).map((cap) => (
                  <span
                    key={cap}
                    className="rounded bg-info-muted px-1.5 py-0.5 text-2xs font-medium text-info"
                  >
                    {cap}
                  </span>
                ))}
                {a2a.capabilities.length > 4 && (
                  <span className="text-2xs text-muted-foreground">
                    +{a2a.capabilities.length - 4}
                  </span>
                )}
              </div>
            )}

            {/* Error indicator */}
            {lifecycle.error && (
              <div className="truncate text-2xs text-error" title={lifecycle.error}>
                ⚠ {lifecycle.error}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}

export const MonitorNode = memo(MonitorNodeInner)
