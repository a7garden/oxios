// BlockStream — interleaved timeline renderer for a turn's ChatBlock[].
//
// Replaces the categorized Thinking + ToolCallList pipeline (2026-07-27).
// Each block renders in arrival order: a reasoning span, a tool card, or a
// text emission (preamble / terminal answer). This is the agent's flow of
// thought — reason → tool → reason → tool → answer — not a category bucket.
//
// Spacing encodes hierarchy: process blocks (reasoning/tool) cluster tightly
// so the flow reads as one rhythm, while the terminal text answer gets extra
// air above it so the conclusion stands apart from the process that produced
// it.

import { memo, type ReactNode } from 'react'
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
    <div className="flex flex-col">
      {blocks.map((b, i) => {
        const prev = blocks[i - 1]
        // First block: no top margin. The terminal text answer, when it
        // follows the reasoning/tool process, gets extra air above; every
        // other transition clusters tight.
        const spacer = i === 0 ? '' : b.type === 'text' && prev?.type !== 'text' ? 'mt-3' : 'mt-1.5'

        let node: ReactNode = null
        if (b.type === 'reasoning') {
          node = (
            <Thinking
              messageId={messageId}
              content={b.text}
              thinking={b.status === 'streaming'}
              duration={b.status === 'streaming' ? Date.now() - b.startedAt : b.durationMs}
            />
          )
        } else if (b.type === 'tool') {
          // ToolBlock = { type: 'tool' } & ChatToolPayload; assignable to the
          // ChatToolPayload prop (the discriminator is structurally compatible).
          node = <ToolCallCard call={b} defaultExpanded={false} />
        } else if (b.type === 'text') {
          node = (
            <MarkdownMessage key={b.id} messageId={messageId} isStreaming={!!b.streaming}>
              {b.text}
            </MarkdownMessage>
          )
        } else if (b.type === 'memory') {
          // Memory events are surfaced by the LiveActivityBar; the in-timeline
          // card is intentionally minimal so it doesn't compete with the
          // reasoning/tool call flow-of-thought.
          node = (
            <div
              className="rounded-md border border-border/40 bg-muted/20 px-2.5 py-1.5 text-xs text-muted-foreground"
              data-memory-action={b.action}
            >
              {b.action === 'store'
                ? `Memory stored${b.count != null ? ` (${b.count})` : ''}`
                : `Memory recall${b.query ? `: ${b.query}` : ''}${b.count != null ? ` (${b.count})` : ''}`}
            </div>
          )
        }
        // Non-visual blocks (usage) render nothing in-timeline — bail before
        // the wrapper so they don't add a spurious gap at the turn's tail.
        if (!node) return null

        return (
          <div key={b.id} className={spacer}>
            {node}
          </div>
        )
      })}
    </div>
  )
})
