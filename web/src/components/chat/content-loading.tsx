// ContentLoading — streaming indicator with operation label (ported from LobeHub)
//
// LobeHub original: src/features/Conversation/Messages/components/ContentLoading.tsx
// Shows the operation label + a live elapsed-seconds counter while the agent is
// working. Rendered inside the assistant bubble only during the pure pre-stream
// gap (no content / reasoning / tool calls yet) — see AssistantMessage.showLoading.
// The LiveActivityBar header owns the descriptive sentence once a concrete
// activity (tool/reasoning) is underway, so this component's label is always
// the generic "Thinking…".

import { Loader2 } from 'lucide-react'
import { memo, useEffect, useState } from 'react'

// LobeHub shows the elapsed counter only after the op has run a couple seconds,
// so a sub-second round-trip doesn't flash a "0s".
const ELAPSED_THRESHOLD_MS = 2000

// ── Props ──

export interface ContentLoadingProps {
  id: string
  /** The current operation label (e.g. "Thinking…"). */
  label?: string
  /** Epoch ms when the operation started (message timestamp). When provided,
   *  a live mm:ss counter is rendered next to the label after the threshold. */
  startTime?: number
}

// ── Component ──

export const ContentLoading = memo(function ContentLoading({
  label = 'Thinking...',
  startTime,
}: ContentLoadingProps) {
  const [elapsedMs, setElapsedMs] = useState(() =>
    startTime ? Math.max(0, Date.now() - startTime) : 0,
  )

  // Tick once per second so the counter stays live without re-rendering the
  // whole message list. Cleared on unmount / when the gap ends.
  useEffect(() => {
    if (!startTime) return
    setElapsedMs(Math.max(0, Date.now() - startTime))
    const handle = window.setInterval(
      () => setElapsedMs(Math.max(0, Date.now() - startTime)),
      1000,
    )
    return () => window.clearInterval(handle)
  }, [startTime])

  const showElapsed = elapsedMs >= ELAPSED_THRESHOLD_MS

  return (
    <div
      className="flex items-center gap-2 text-sm text-muted-foreground py-1"
      aria-live="polite"
    >
      <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden />
      <span>{label}</span>
      {showElapsed && (
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
