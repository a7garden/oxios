// compressed-summary — client-side statistical digest for the collapsed
// message group. Fallback when no LLM summary exists or generation failed.

import type { ChatMessage } from '@/types'

export interface CompressedDigest {
  total: number
  userCount: number
  assistantCount: number
  toolCallCount: number
  firstAt?: string
  lastAt?: string
}

export function buildCompressedDigest(messages: ChatMessage[]): CompressedDigest {
  let userCount = 0
  let assistantCount = 0
  let toolCallCount = 0
  let firstAt: string | undefined
  let lastAt: string | undefined

  for (const m of messages) {
    if (m.role === 'user') userCount++
    else if (m.role === 'assistant') assistantCount++

    if (m.blocks) {
      toolCallCount += m.blocks.filter((b) => b.type === 'tool').length
    }

    const ts = m.timestamp
    if (ts) {
      if (!firstAt || ts < firstAt) firstAt = ts
      if (!lastAt || ts > lastAt) lastAt = ts
    }
  }

  return { total: messages.length, userCount, assistantCount, toolCallCount, firstAt, lastAt }
}
