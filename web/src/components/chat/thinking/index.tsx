// Thinking block — collapsible reasoning display (ported from LobeHub).
//
// Visual tier (2026-07-28 redesign): reasoning is a RECESSED aside, not a peer
// of the answer. The in-bubble hierarchy is answer (plain text) > tool card
// (muted, bordered) > reasoning (left-rail accent, no full border, muted bg).
// This keeps the agent's flow-of-thought readable without competing with the
// conclusion. The body renders as markdown — the Phase 3 upgrade of the
// original monospace <pre> (lists/code/emphasis now render, not raw glyphs).

import { Brain, Loader2 } from 'lucide-react'
import { memo, useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { MarkdownMessage } from '@/components/chat/markdown-message'
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion'
import { ScrollArea } from '@/components/ui/scroll-area'
import { cn } from '@/lib/utils'

// ── Props ──

export interface ThinkingProps {
  /** Markdown content of the reasoning block. */
  content?: string
  /** Whether the agent is currently thinking (streaming). Auto-expands. */
  thinking?: boolean
  /** Elapsed duration in milliseconds. */
  duration?: number
  /** Owning message id — forwarded to MarkdownMessage for artifact context. */
  messageId?: string
  /** Extra class on the outer wrapper. */
  className?: string
}

// ── Component ──

export const Thinking = memo(function Thinking({
  content,
  thinking = false,
  duration,
  messageId = '',
  className,
}: ThinkingProps) {
  const [open, setOpen] = useState(thinking)

  // Auto-expand while streaming, collapse when done.
  useEffect(() => {
    setOpen(thinking)
  }, [thinking])

  const hasContent = !!content && content.trim().length > 0
  if (!hasContent && !thinking) return null

  return (
    <Accordion
      type="single"
      collapsible
      value={open ? 'thinking' : ''}
      onValueChange={(v) => setOpen(v === 'thinking')}
      className={cn('border-0', className)}
    >
      <AccordionItem
        value="thinking"
        className="rounded-md border-0 border-l-2 border-l-muted-foreground/20 bg-muted/25"
      >
        <AccordionTrigger className="py-1.5 px-2.5 hover:no-underline">
          <ThinkingTitle thinking={thinking} duration={duration} />
        </AccordionTrigger>
        <AccordionContent className="px-2.5 pb-2.5">
          <ScrollArea className="max-h-[min(40vh,320px)]">
            <MarkdownMessage messageId={messageId} isStreaming={thinking} className="text-xs">
              {content ?? ''}
            </MarkdownMessage>
          </ScrollArea>
        </AccordionContent>
      </AccordionItem>
    </Accordion>
  )
})

// ── Title ──

function ThinkingTitle({ thinking, duration }: { thinking: boolean; duration?: number }) {
  const { t } = useTranslation()
  return (
    <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
      {thinking ? <Loader2 className="w-3 h-3 animate-spin" /> : <Brain className="w-3 h-3" />}
      <span className={cn('font-medium', thinking && 'thinking-shiny')}>
        {thinking ? t('chat.thinking') : t('chat.thought')}
      </span>
      {duration != null && (
        <span className="ml-auto tabular-nums text-muted-foreground/60">
          {formatDuration(duration)}
        </span>
      )}
    </div>
  )
}

// ── Helpers ──

function formatDuration(ms: number): string {
  const seconds = ms / 1000
  if (seconds < 60) return `${seconds.toFixed(1)}s`
  const minutes = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${minutes}m ${secs}s`
}
