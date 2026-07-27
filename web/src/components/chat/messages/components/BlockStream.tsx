// BlockStream — interleaved timeline renderer for a turn's ChatBlock[].
//
// Replaces the categorized Thinking + ToolCallList pipeline (2026-07-27).
// Each block is rendered in arrival order: a reasoning span, a tool card, or
// a text emission (preamble / terminal answer). This is the agent's flow of
// thought — reason → tool → reason → tool → answer — not a category bucket.
//
// Reasoning reuses the existing <Thinking> component per segment; tool cards
// reuse the exported <ToolCallCard>; text reuses <MarkdownMessage>.

import { memo } from 'react'
import { MarkdownMessage } from '@/components/chat/markdown-message'
import { Thinking } from '@/components/chat/thinking'
import type { ChatBlock } from '@/types'
import { ToolCallCard } from './ToolCallList'

interface BlockStreamProps {
  blocks: ChatBlock[]
  messageId: string
}

export const BlockStream = memo(function BlockStream({ blocks, messageId }: BlockStreamProps) {
  return (
    <div className="flex flex-col gap-2">
      {blocks.map((b) => {
        if (b.type === 'reasoning') {
          return (
            <Thinking
              key={b.id}
              content={b.text}
              thinking={b.status === 'streaming'}
              duration={b.status === 'streaming' ? Date.now() - b.startedAt : b.durationMs}
            />
          )
        }
        if (b.type === 'tool') {
          // ToolBlock = { type: 'tool' } & ChatToolPayload; assignable to the
          // ChatToolPayload prop (the discriminator is structurally compatible).
          return <ToolCallCard key={b.id} call={b} defaultExpanded={false} />
        }
        if (b.type === 'text') {
          return (
            <MarkdownMessage key={b.id} messageId={messageId} isStreaming={!!b.streaming}>
              {b.text}
            </MarkdownMessage>
          )
        }
        if (b.type === 'memory') {
          // Memory events are surfaced by the LiveActivityBar; the
          // in-timeline card is intentionally minimal so it doesn't compete
          // with the reasoning/tool call flow-of-thought.
          return (
            <div
              key={b.id}
              className="rounded-md border border-border/40 bg-muted/20 px-2.5 py-1.5 text-xs text-muted-foreground"
              data-memory-action={b.action}
            >
              {b.action === 'store'
                ? `Memory stored${b.count != null ? ` (${b.count})` : ''}`
                : `Memory recall${b.query ? `: ${b.query}` : ''}${b.count != null ? ` (${b.count})` : ''}`}
            </div>
          )
        }
        // usage
        return null
      })}
    </div>
  )
})
