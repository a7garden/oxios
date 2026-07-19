// Thinking block — collapsible reasoning display (ported from LobeHub)
//
// LobeHub original: /tmp/lobehub/src/features/Conversation/components/Thinking/index.tsx
// Dependencies removed: @lobehub/ui (Accordion, AccordionItem, ScrollArea),
//   antd-style (createStaticStyles, cssVar)
// Replaced with: shadcn/ui Accordion, ScrollArea + Tailwind

import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion'
import { ScrollArea } from '@/components/ui/scroll-area'
import { cn } from '@/lib/utils'
import { Brain, Loader2 } from 'lucide-react'
import { memo, useEffect, useState } from 'react'

// ── Props ──

export interface ThinkingProps {
  /** Markdown content of the reasoning block. */
  content?: string
  /** Whether the agent is currently thinking (streaming). Auto-expands accordion. */
  thinking?: boolean
  /** Elapsed duration in milliseconds. */
  duration?: number
  /** Extra class on the outer wrapper. */
  className?: string
}

// ── Component ──

export const Thinking = memo(function Thinking({
  content,
  thinking = false,
  duration,
  className,
}: ThinkingProps) {
  const [open, setOpen] = useState(thinking)

  // Auto-expand while streaming, collapse when done
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
      <AccordionItem value="thinking" className="border rounded-lg px-3">
        <AccordionTrigger className="py-2 hover:no-underline group">
          <ThinkingTitle thinking={thinking} duration={duration} />
        </AccordionTrigger>
        <AccordionContent>
          <ScrollArea className="max-h-[min(40vh,320px)]">
            <div className="px-2 pb-2 text-sm text-muted-foreground">
              {/* Content rendered as markdown by parent — passed as children concept.
                  For now, render as monospace preformatted text.
                  Will be upgraded to MarkdownMessage in Phase 3. */}
              <pre className="whitespace-pre-wrap font-mono text-xs leading-relaxed">
                {content ?? 'Thinking...'}
              </pre>
            </div>
          </ScrollArea>
        </AccordionContent>
      </AccordionItem>
    </Accordion>
  )
})

// ── Title ──

function ThinkingTitle({
  thinking,
  duration,
}: {
  thinking: boolean
  duration?: number
}) {
  return (
    <div className="flex items-center gap-2 text-sm">
      {thinking ? (
        <Loader2 className="w-3.5 h-3.5 animate-spin text-muted-foreground" />
      ) : (
        <Brain className="w-3.5 h-3.5 text-muted-foreground" />
      )}
      <span className="font-medium text-muted-foreground">
        {thinking ? 'Thinking...' : 'Thought'}
      </span>
      {duration != null && (
        <span className="text-xs text-muted-foreground/60 tabular-nums ml-auto">
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
