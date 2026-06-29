import { Loader2, Sparkles } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { deriveCurrentActivity } from '@/lib/live-activity'
import { cn } from '@/lib/utils'
import { useChatStore } from '@/stores/chat'

/**
 * RFC-015 §4.3 — LiveActivityBar.
 *
 * Replaces the legacy 3-dot typing indicator with a header that reflects
 * the single most recent in-flight activity for the assistant turn:
 *
 *   thinking      → 💭 pulse + "Thinking..."
 *   tool_running  → 🔍 Loader2  + "Running {toolName}"
 *   reasoning     → ✨ pulse    + "Reasoning..."
 *
 * Activity cards below remain as the historical timeline (see
 * `ActivityTimeline`). The bar fades out the moment the assistant starts
 * streaming text, so the typewriter takes over.
 *
 * Mounted only while an assistant turn is being built — i.e. `isStreaming`
 * is true AND the most recent message in the store is an assistant
 * message. This matches the live-activity UX table in the design doc.
 */
export function LiveActivityBar() {
  const { t } = useTranslation()
  const isStreaming = useChatStore((s) => s.isStreaming)
  const last = useChatStore((s) => s.messages.at(-1))

  if (!isStreaming || last?.role !== 'assistant') return null

  const descriptor = deriveCurrentActivity(last.activities)
  const streamingTextStarted = (last.content ?? '').trim().length > 0

  const label =
    descriptor.kind === 'tool_running'
      ? t('chat.liveActivity.toolRunning', {
          name: descriptor.toolName ?? 'tool',
        })
      : descriptor.kind === 'reasoning'
        ? t('chat.liveActivity.reasoning')
        : t('chat.liveActivity.thinking')

  return (
    <div
      className={cn(
        'flex my-1.5 animate-fade-in-up transition-opacity duration-300',
        streamingTextStarted && 'opacity-0 pointer-events-none',
      )}
      aria-live="polite"
      data-state={streamingTextStarted ? 'fading' : 'live'}
    >
      <div className="max-w-[80%]">
        <div className="rounded-lg px-4 py-2.5 bg-muted">
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            {descriptor.kind === 'tool_running' ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" aria-hidden />
            ) : descriptor.kind === 'reasoning' ? (
              <Sparkles className="h-3.5 w-3.5 animate-pulse" aria-hidden />
            ) : (
              <span
                className="h-2 w-2 rounded-full bg-muted-foreground/60 animate-pulse"
                aria-hidden
              />
            )}
            <span>{label}</span>
          </div>
        </div>
      </div>
    </div>
  )
}
