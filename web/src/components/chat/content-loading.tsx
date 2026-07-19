// ContentLoading — streaming indicator with operation label (ported from LobeHub)
//
// LobeHub original: /tmp/lobehub/src/features/Conversation/Messages/components/ContentLoading.tsx
// Shows operation descriptor + elapsed time while the agent is working.
// Hides for 'reasoning' operations (Thinking component handles that).

'use client'

import { Loader2 } from 'lucide-react'

import { memo } from 'react'

// ── Props ──

export interface ContentLoadingProps {
  id: string
  /** The current operation label (e.g. "Generating…", "Searching…"). */
  label?: string
  /** Elapsed time in milliseconds since operation started. */
  elapsedMs?: number
}

// ── Component ──

export const ContentLoading = memo(function ContentLoading({
  label = 'Thinking...',
  elapsedMs,
}: ContentLoadingProps) {
  return (
    <div className="flex items-center gap-2 text-sm text-muted-foreground py-1">
      <Loader2 className="w-3.5 h-3.5 animate-spin" />
      <span>{label}</span>
      {elapsedMs != null && elapsedMs > 0 && (
        <span className="text-xs text-muted-foreground/60 tabular-nums">
          {formatElapsed(elapsedMs)}
        </span>
      )}
    </div>
  )
})

// ── Helpers ──

function formatElapsed(ms: number): string {
  const seconds = ms / 1000
  if (seconds < 60) return `${seconds.toFixed(0)}s`
  const minutes = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${minutes}m ${secs}s`
}
