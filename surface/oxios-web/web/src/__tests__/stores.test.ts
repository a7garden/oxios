import { beforeEach, describe, expect, it } from 'vitest'
import { useAuthStore } from '@/stores/auth'
import { useChatStore } from '@/stores/chat'
import { useSidebarStore } from '@/stores/sidebar'

describe('useAuthStore', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('starts unauthenticated when no token', () => {
    const state = useAuthStore.getState()
    expect(state.isAuthenticated).toBe(false)
    expect(state.token).toBeNull()
  })

  it('sets token and authenticates', () => {
    useAuthStore.getState().setToken('test-key')
    const state = useAuthStore.getState()
    expect(state.isAuthenticated).toBe(true)
    expect(state.token).toBe('test-key')
    expect(localStorage.getItem('oxios-api-key')).toBe('test-key')
  })

  it('logout clears token', () => {
    useAuthStore.getState().setToken('test-key')
    useAuthStore.getState().logout()
    const state = useAuthStore.getState()
    expect(state.isAuthenticated).toBe(false)
    expect(state.token).toBeNull()
    expect(localStorage.getItem('oxios-api-key')).toBeNull()
  })

  it('setToken(null) clears authentication', () => {
    useAuthStore.getState().setToken('test-key')
    useAuthStore.getState().setToken(null)
    expect(useAuthStore.getState().isAuthenticated).toBe(false)
  })
})

describe('useSidebarStore', () => {
  beforeEach(() => {
    localStorage.clear()
    useSidebarStore.setState({ collapsed: false, mobileOpen: false })
  })

  it('toggles collapsed state', () => {
    expect(useSidebarStore.getState().collapsed).toBe(false)
    useSidebarStore.getState().toggle()
    expect(useSidebarStore.getState().collapsed).toBe(true)
    useSidebarStore.getState().toggle()
    expect(useSidebarStore.getState().collapsed).toBe(false)
  })

  it('sets mobile open state', () => {
    useSidebarStore.getState().setMobileOpen(true)
    expect(useSidebarStore.getState().mobileOpen).toBe(true)
    useSidebarStore.getState().setMobileOpen(false)
    expect(useSidebarStore.getState().mobileOpen).toBe(false)
  })
})

// RFC-015: chat transparency event handling
describe('useChatStore handleChunk (RFC-015)', () => {
  beforeEach(() => {
    localStorage.clear()
    // Start each test with a single empty assistant message so chunks
    // have a target to attach to.
    useChatStore.setState({
      messages: [
        {
          id: 'a1',
          role: 'assistant' as const,
          content: '',
          timestamp: new Date().toISOString(),
        },
      ],
      isStreaming: true,
    })
  })

  it('tool_start appends a tool_call activity', () => {
    useChatStore.getState().handleChunk({
      type: 'tool_start',
      tool_name: 'read_file',
      tool_call_id: 'c1',
      tool_args: { path: '/x' },
    })
    const last = useChatStore.getState().messages.at(-1)!
    expect(last.activities).toHaveLength(1)
    expect(last.activities![0]).toMatchObject({
      type: 'tool_call',
      toolName: 'read_file',
      toolCallId: 'c1',
    })
  })

  it('tool_end attaches duration and output to the same tool_call', () => {
    useChatStore.getState().handleChunk({
      type: 'tool_start',
      tool_name: 'bash',
      tool_call_id: 'c1',
      tool_args: {},
    })
    useChatStore.getState().handleChunk({
      type: 'tool_end',
      tool_name: 'bash',
      tool_call_id: 'c1',
      duration_ms: 50,
      is_error: false,
      output_summary: 'ok',
    })
    const last = useChatStore.getState().messages.at(-1)!
    // tool_end collapses into the same tool_call (no duplicate activity).
    const toolActivities = last.activities!.filter((a) => a.type === 'tool_call')
    expect(toolActivities).toHaveLength(1)
    expect(toolActivities[0]).toMatchObject({
      type: 'tool_call',
      toolName: 'bash',
      toolCallId: 'c1',
      durationMs: 50,
      outputSummary: 'ok',
    })
  })

  it('memory recall appends a memory activity', () => {
    useChatStore.getState().handleChunk({
      type: 'memory',
      action: 'recall',
      query: 'rust errors',
      count: 3,
      source: 'warm',
    })
    const last = useChatStore.getState().messages.at(-1)!
    expect(last.activities![0]).toMatchObject({
      type: 'memory',
      memoryAction: 'recall',
      query: 'rust errors',
      count: 3,
      memorySource: 'warm',
    })
  })

  it('usage accumulates input/output tokens on the assistant message', () => {
    useChatStore.getState().handleChunk({
      type: 'usage',
      input_tokens: 100,
      output_tokens: 30,
    })
    useChatStore.getState().handleChunk({
      type: 'usage',
      input_tokens: 50,
      output_tokens: 20,
    })
    const last = useChatStore.getState().messages.at(-1)!
    expect(last.totalInputTokens).toBe(150)
    expect(last.totalOutputTokens).toBe(50)
  })

  it('reasoning appends a reasoning activity', () => {
    useChatStore.getState().handleChunk({
      type: 'reasoning',
      content: 'compaction complete',
      source: 'compaction',
    })
    const last = useChatStore.getState().messages.at(-1)!
    expect(last.activities![0]).toMatchObject({
      type: 'reasoning',
      content: 'compaction complete',
      reasoningSource: 'compaction',
    })
  })

  it('token chunk does not add an activity', () => {
    useChatStore.getState().handleChunk({ type: 'token', content: 'hello' })
    const last = useChatStore.getState().messages.at(-1)!
    expect(last.content).toBe('hello')
    expect(last.activities ?? []).toEqual([])
  })

  it('done chunk keeps accumulated activities and sets isStreaming=false', () => {
    useChatStore.getState().handleChunk({
      type: 'tool_start',
      tool_name: 'grep',
      tool_call_id: 'g1',
      tool_args: {},
    })
    useChatStore.getState().handleChunk({
      type: 'done',
      session_id: 's1',
      phase: 'execute',
    })
    const state = useChatStore.getState()
    expect(state.isStreaming).toBe(false)
    const last = state.messages.at(-1)!
    expect(last.activities).toHaveLength(1)
    expect(last.metadata?.phase).toBe('execute')
  })
})
