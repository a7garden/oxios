import { Loader2, Sparkles } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { deriveCurrentActivity, describeLiveActivity } from '@/lib/live-activity'
import { useChatStore } from '@/stores/chat'

const ELAPSED_THRESHOLD_MS = 2000

/**
 * LiveActivityBar — in-input status row embedded at the top of the chat input
 * box. Visible for the ENTIRE assistant turn (`isStreaming`), across
 * every phase, and never fades when text starts:
 *
 *   gap (no chunk yet) → "생각하는 중… · 3s"
 *   reasoning          → "추론 중 · 5s"
 *   tool running       → "웹 검색 중 · rust async · 8s"
 *   text streaming     → "응답 작성 중… · 12s"
 *
 * Phase priority: running tool → text streaming (writing) → reasoning →
 * thinking. The in-bubble Thinking / ToolCallList panels carry the detail;
 * this row is the single always-on one-line status + elapsed timer, so the
 * user can tell the agent is still working even while reading the streamed
 * response. Reads `streamStartedAt` (set in sendMessage) for the timer.
 */
export function LiveActivityBar() {
  const { t } = useTranslation()
  const isStreaming = useChatStore((s) => s.isStreaming)
  const startedAt = useChatStore((s) => s.streamStartedAt)
  const last = useChatStore((s) => s.messages.at(-1))
  const activeInterview = useChatStore((s) => s.activeInterview)
  const activeToolApproval = useChatStore((s) => s.activeToolApproval)

  const [elapsedMs, setElapsedMs] = useState(0)
  useEffect(() => {
    if (!isStreaming || !startedAt) {
      setElapsedMs(0)
      return
    }
    const tick = () => setElapsedMs(Math.max(0, Date.now() - startedAt))
    tick()
    const handle = window.setInterval(tick, 1000)
    return () => window.clearInterval(handle)
  }, [isStreaming, startedAt])

  if (!isStreaming || activeInterview || activeToolApproval) return null

  // Only the trailing assistant message carries this turn's activities/content.
  const lastAssistant = last?.role === 'assistant' ? last : undefined
  const descriptor = deriveCurrentActivity(lastAssistant?.activities)
  const streamingText = !!(lastAssistant?.generating && (lastAssistant?.content ?? '').trim())

  let label: string
  let detail: string | undefined
  let icon: 'spinner' | 'sparkles' | 'pulse'

  if (descriptor.kind === 'tool_running') {
    const desc = describeLiveActivity(descriptor, t)
    label = desc.label
    detail = desc.detail
    icon = 'spinner'
  } else if (streamingText) {
    label = t('chat.liveActivity.writing')
    icon = 'spinner'
  } else if (descriptor.kind === 'reasoning') {
    label = t('chat.liveActivity.reasoning')
    icon = 'sparkles'
  } else {
    label = t('chat.liveActivity.thinking')
    icon = 'pulse'
  }

  return (
    <div
      className="flex items-center gap-2 px-4 pt-2.5 pb-1.5 text-xs text-muted-foreground border-b border-border/50 animate-fade-in-up"
      aria-live="polite"
    >
      {icon === 'spinner' ? (
        <Loader2 className="h-3.5 w-3.5 animate-spin shrink-0" aria-hidden />
      ) : icon === 'sparkles' ? (
        <Sparkles className="h-3.5 w-3.5 animate-pulse shrink-0" aria-hidden />
      ) : (
        <span
          className="h-2 w-2 rounded-full bg-muted-foreground/60 animate-pulse shrink-0"
          aria-hidden
        />
      )}
      <span className="truncate">{label}</span>
      {detail && (
        <>
          <span className="text-muted-foreground/40 shrink-0">·</span>
          <span className="truncate text-muted-foreground/70 max-w-[40ch]">{detail}</span>
        </>
      )}
      {elapsedMs >= ELAPSED_THRESHOLD_MS && (
        <span className="ml-auto shrink-0 tabular-nums text-muted-foreground/60">
          {formatElapsed(elapsedMs)}
        </span>
      )}
    </div>
  )
}

function formatElapsed(ms: number): string {
  const seconds = ms / 1000
  if (seconds < 60) return `${seconds.toFixed(0)}s`
  const minutes = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${minutes}m ${secs}s`
}
