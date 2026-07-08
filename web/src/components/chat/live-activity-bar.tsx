import { Loader2, Sparkles } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { describeLiveActivity, deriveCurrentActivity } from '@/lib/live-activity'
import { cn } from '@/lib/utils'
import { useChatStore } from '@/stores/chat'

/**
 * RFC-015 §4.3 — LiveActivityBar.
 *
 * Replaces the legacy 3-dot typing indicator with a header that reflects
 * the single most recent in-flight activity for the assistant turn.
 * Instead of a generic "Thinking…" the bar now shows a sentence-level
 * description of what is actually happening:
 *
 *   thinking      → pulse + "Thinking…"
 *   tool_running  → spinner + "Searching the web · rust async"
 *                             "Reading file · …/src/main.rs"
 *                             "Running command · cargo build"
 *   reasoning     → pulse  + "Reasoning…"
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
  const { label, detail } = describeLiveActivity(descriptor, t)

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
              <Loader2 className="h-3.5 w-3.5 animate-spin shrink-0" aria-hidden />
            ) : descriptor.kind === 'reasoning' ? (
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
                <span className="text-muted-foreground/70 truncate max-w-[40ch]">
                  {detail}
                </span>
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
