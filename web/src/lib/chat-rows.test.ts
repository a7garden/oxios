import { describe, expect, it } from 'vitest'
import type { ChatMessage } from '@/types'
import { buildChatRows } from './chat-rows'

const msg = (id: string, role: 'user' | 'assistant' = 'user'): ChatMessage => ({
  id,
  role,
  content: id,
  timestamp: '',
})

const base = {
  expanded: false,
  collapseThreshold: 40,
  visibleTail: 20,
  hasInterview: false,
  hasToolApproval: false,
  hasPathAccess: false,
}

describe('buildChatRows', () => {
  it('returns a single empty row when there are no messages or cards', () => {
    expect(buildChatRows({ ...base, messages: [] })).toEqual([{ kind: 'empty' }])
  })

  it('lists all messages below the collapse threshold (no bar)', () => {
    const messages = [msg('u1'), msg('a1', 'assistant'), msg('u2')]
    const rows = buildChatRows({ ...base, messages })
    expect(rows.map((r) => r.kind)).toEqual(['message', 'message', 'message'])
    expect(rows[0]).toMatchObject({ kind: 'message', index: 0 })
    expect(rows[2]).toMatchObject({ kind: 'message', index: 2 })
  })

  it('collapses older messages behind a bar when over threshold', () => {
    const messages = Array.from({ length: 45 }, (_, i) => msg(`m${i}`))
    const rows = buildChatRows({ ...base, messages })
    // collapseCount = 45 - 20 = 25 → bar + last 20 messages.
    expect(rows[0]).toEqual({ kind: 'collapse-bar', count: 25 })
    expect(rows).toHaveLength(21)
    expect(rows[1]).toMatchObject({ kind: 'message', index: 25 })
    expect(rows[20]).toMatchObject({ kind: 'message', index: 44 })
  })

  it('expands to all messages (bar stays, full list follows) when expanded', () => {
    const messages = Array.from({ length: 45 }, (_, i) => msg(`m${i}`))
    const rows = buildChatRows({ ...base, messages, expanded: true })
    expect(rows[0]).toEqual({ kind: 'collapse-bar', count: 25 })
    expect(rows).toHaveLength(46) // bar + 45 messages
    expect(rows[1]).toMatchObject({ kind: 'message', index: 0 })
  })

  it('appends interview/approval/path-access rows after messages', () => {
    const rows = buildChatRows({
      ...base,
      messages: [msg('u1')],
      hasInterview: true,
      hasToolApproval: true,
      hasPathAccess: true,
    })
    expect(rows.map((r) => r.kind)).toEqual([
      'message',
      'interview',
      'tool-approval',
      'path-access',
    ])
  })

  it('shows cards even with no messages (no empty row)', () => {
    const rows = buildChatRows({ ...base, messages: [], hasToolApproval: true })
    expect(rows.map((r) => r.kind)).toEqual(['tool-approval'])
  })
})
