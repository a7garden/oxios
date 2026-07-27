import { describe, expect, it } from 'vitest'
import type { ChatMessage } from '@/types'
import { hydrateBlocks } from './chat'

describe('hydrateBlocks', () => {
  it('returns existing blocks unchanged (same ref)', () => {
    const blocks = [{ type: 'text' as const, id: 't1', text: 'hi' }]
    const msg = { id: 'm1', role: 'assistant', content: '', blocks } as ChatMessage
    expect(hydrateBlocks(msg)).toBe(blocks)
  })

  it('synthesizes reasoning + tool + text from legacy activities/content, in order', () => {
    const msg = {
      id: 'm1',
      role: 'assistant',
      content: 'Answer',
      activities: [
        { id: 'a1', type: 'reasoning', timestamp: '2026-01-01T00:00:00Z', content: 'hmm' },
        {
          id: 'a2',
          type: 'tool_call',
          timestamp: '2026-01-01T00:00:01Z',
          toolName: 'grep',
          toolCallId: 'tc1',
          toolArgs: { q: 'x' },
          durationMs: 5,
        },
      ],
    } as unknown as ChatMessage

    const blocks = hydrateBlocks(msg)!
    expect(blocks.map((b) => b.type)).toEqual(['reasoning', 'tool', 'text'])
    expect(blocks[1]).toMatchObject({ type: 'tool', apiName: 'grep', status: 'success', durationMs: 5 })
    expect(blocks[2]).toMatchObject({ type: 'text', text: 'Answer' })
  })

  it('interleaves multiple reasoning spans with tool calls in order', () => {
    // P2 reopen fidelity: reasoning split across tools (reason → tool →
    // reason → tool → text), mirroring the live block-stream.
    const msg = {
      id: 'm1',
      role: 'assistant',
      content: 'Answer',
      activities: [
        { id: 'r0', type: 'reasoning', timestamp: '2026-01-01T00:00:00Z', content: 'first think' },
        {
          id: 'a1',
          type: 'tool_call',
          timestamp: '2026-01-01T00:00:01Z',
          toolName: 'grep',
          toolCallId: 'tc1',
          toolArgs: {},
        },
        { id: 'r1', type: 'reasoning', timestamp: '2026-01-01T00:00:02Z', content: 'then think' },
        {
          id: 'a2',
          type: 'tool_call',
          timestamp: '2026-01-01T00:00:03Z',
          toolName: 'read',
          toolCallId: 'tc2',
          toolArgs: {},
        },
      ],
    } as unknown as ChatMessage

    const blocks = hydrateBlocks(msg)!
    expect(blocks.map((b) => b.type)).toEqual(['reasoning', 'tool', 'reasoning', 'tool', 'text'])
  })

  it('returns undefined when there is nothing to show', () => {
    expect(hydrateBlocks({ id: 'm1', role: 'assistant', content: '' } as ChatMessage)).toBeUndefined()
  })
})
