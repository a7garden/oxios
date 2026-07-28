import { describe, expect, it } from 'vitest'
import type { ChatMessage } from '@/types'
import { buildCompressedDigest } from './compressed-summary'

describe('buildCompressedDigest', () => {
  it('counts roles and tool calls across folded messages', () => {
    const messages: ChatMessage[] = [
      { id: 'u1', role: 'user', content: 'q', timestamp: '2026-01-01T00:00:00Z' },
      {
        id: 'a1',
        role: 'assistant',
        content: 'r',
        timestamp: '2026-01-01T00:01:00Z',
        blocks: [
          {
            type: 'tool',
            id: 't1',
            identifier: 'k',
            apiName: 'grep',
            arguments: {},
            status: 'success',
          },
          {
            type: 'tool',
            id: 't2',
            identifier: 'k',
            apiName: 'read',
            arguments: {},
            status: 'success',
          },
          { type: 'text', id: 'x1', text: 'answer' },
        ],
      },
      { id: 'u2', role: 'user', content: 'q2', timestamp: '2026-01-01T00:02:00Z' },
    ]
    const d = buildCompressedDigest(messages)
    expect(d.total).toBe(3)
    expect(d.userCount).toBe(2)
    expect(d.assistantCount).toBe(1)
    expect(d.toolCallCount).toBe(2)
    expect(d.firstAt).toBe('2026-01-01T00:00:00Z')
    expect(d.lastAt).toBe('2026-01-01T00:02:00Z')
  })

  it('returns zeros for an empty list', () => {
    expect(buildCompressedDigest([])).toMatchObject({
      total: 0,
      userCount: 0,
      assistantCount: 0,
      toolCallCount: 0,
    })
  })
})
