import { describe, expect, it } from 'vitest'
import { deriveCurrentActivity, type LiveActivityDescriptor } from '@/lib/live-activity'
import type { ChatActivity } from '@/types'

const NOW = new Date().toISOString()

/** Convenience: build a minimal tool_call activity with the running flag. */
function toolCall(overrides: Partial<ChatActivity> & { isRunning?: boolean }): ChatActivity {
  return {
    id: 'tool-' + Math.random().toString(36).slice(2),
    type: 'tool_call',
    timestamp: NOW,
    toolName: 'read_file',
    toolCallId: 'c1',
    ...overrides,
  }
}

function reasoning(): ChatActivity {
  return {
    id: 'reason-' + Math.random().toString(36).slice(2),
    type: 'reasoning',
    timestamp: NOW,
    content: 'thinking about it',
    reasoningSource: 'compaction',
  }
}

function usage(): ChatActivity {
  return {
    id: 'usage-' + Math.random().toString(36).slice(2),
    type: 'usage',
    timestamp: NOW,
    inputTokens: 10,
    outputTokens: 4,
  }
}

describe('deriveCurrentActivity (RFC-015 §4.3)', () => {
  it('falls back to thinking when activities is undefined', () => {
    expect(deriveCurrentActivity(undefined)).toEqual<LiveActivityDescriptor>({
      kind: 'thinking',
    })
  })

  it('falls back to thinking when activities is empty', () => {
    expect(deriveCurrentActivity([])).toEqual<LiveActivityDescriptor>({
      kind: 'thinking',
    })
  })

  it('reports a running tool_call as tool_running with the tool name', () => {
    const activities: ChatActivity[] = [toolCall({ isRunning: true, toolName: 'bash' })]
    expect(deriveCurrentActivity(activities)).toEqual<LiveActivityDescriptor>({
      kind: 'tool_running',
      toolName: 'bash',
    })
  })

  it('ignores tool_call activities that are not running', () => {
    // tool_end clears isRunning — those must NOT keep the bar pinned to the
    // tool label. The bar should fall back to thinking (or the next-most-
    // recent running activity if any).
    const activities: ChatActivity[] = [
      toolCall({ isRunning: false, toolName: 'bash', outputSummary: 'ok' }),
    ]
    expect(deriveCurrentActivity(activities)).toEqual<LiveActivityDescriptor>({
      kind: 'thinking',
    })
  })

  it('returns tool_running with undefined name when toolName is missing', () => {
    // Defensive: chunkToActivity always sets toolName, but the type allows
    // it to be absent; the component falls back to the literal "tool".
    const activities: ChatActivity[] = [toolCall({ isRunning: true, toolName: undefined })]
    expect(deriveCurrentActivity(activities)).toEqual<LiveActivityDescriptor>({
      kind: 'tool_running',
      toolName: undefined,
    })
  })

  it('reports the most recent reasoning as in-progress (no completion flag exists)', () => {
    // Reasoning fragments are fire-and-forget — they carry no running/end
    // marker. The latest reasoning entry is by definition the live one.
    const activities: ChatActivity[] = [reasoning(), reasoning()]
    expect(deriveCurrentActivity(activities)).toEqual<LiveActivityDescriptor>({
      kind: 'reasoning',
    })
  })

  it('prefers a running tool_call over an older reasoning entry', () => {
    // Order: reasoning → tool_start → tool_progress → ... The backwards
    // walk must find the running tool first.
    const activities: ChatActivity[] = [
      reasoning(),
      toolCall({ isRunning: true, toolName: 'browse' }),
    ]
    expect(deriveCurrentActivity(activities)).toEqual<LiveActivityDescriptor>({
      kind: 'tool_running',
      toolName: 'browse',
    })
  })

  it('returns reasoning when the most recent activity is reasoning', () => {
    // [completed tool, running tool, reasoning] — reasoning is the most
    // recent fire-and-forget fragment, so the bar shows "Reasoning...".
    const activities: ChatActivity[] = [
      toolCall({ isRunning: false, toolName: 'bash', outputSummary: 'ok' }),
      toolCall({ isRunning: true, toolName: 'browse' }),
      reasoning(),
    ]
    expect(deriveCurrentActivity(activities)).toEqual<LiveActivityDescriptor>({
      kind: 'reasoning',
    })
  })

  it('skips trailing non-running / non-reasoning activities (e.g. usage)', () => {
    // [completed tool, usage] — neither matches a "live" state, so the
    // selector must keep walking and land on thinking rather than surface
    // a stale tool label.
    const activities: ChatActivity[] = [
      toolCall({ isRunning: false, toolName: 'bash', outputSummary: 'ok' }),
      usage(),
    ]
    expect(deriveCurrentActivity(activities)).toEqual<LiveActivityDescriptor>({
      kind: 'thinking',
    })
  })
})
