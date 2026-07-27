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
        return (
          <MarkdownMessage key={b.id} messageId={messageId} isStreaming={!!b.streaming}>
            {b.text}
          </MarkdownMessage>
        )
      })}
    </div>
  )
})
