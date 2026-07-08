import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest'
import { useAuthStore } from '@/stores/auth'
import {
  appendActivityToMessages,
  appendTokenToMessages,
  chunkToActivity,
  ensureLastAssistant,
  mergeOrAppendActivity,
  patchAssistantModel,
  useChatStore,
} from '@/stores/chat'
import type { ChatMessage } from '@/types'
import { useSidebarStore } from '@/stores/sidebar'

describe('useAuthStore', () => {
  beforeEach(() => {
    localStorage.clear()
    sessionStorage.clear()
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
    expect(sessionStorage.getItem('oxios-api-key')).toBe('test-key')
  })

  it('logout clears token', () => {
    useAuthStore.getState().setToken('test-key')
    useAuthStore.getState().logout()
    const state = useAuthStore.getState()
    expect(state.isAuthenticated).toBe(false)
    expect(state.token).toBeNull()
    expect(sessionStorage.getItem('oxios-api-key')).toBeNull()
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

  it('tool_start marks the tool_call as running', () => {
    useChatStore.getState().handleChunk({
      type: 'tool_start',
      tool_name: 'browse',
      tool_call_id: 'c1',
      tool_args: {},
    })
    const last = useChatStore.getState().messages.at(-1)!
    expect(last.activities![0]).toMatchObject({
      type: 'tool_call',
      toolName: 'browse',
      toolCallId: 'c1',
      isRunning: true,
    })
  })

  it('tool_progress updates the existing tool_call in place (RFC-015 v0.12)', () => {
    // Start a tool, then stream a progress update.
    useChatStore.getState().handleChunk({
      type: 'tool_start',
      tool_name: 'browse',
      tool_call_id: 'c1',
      tool_args: { url: 'https://example.com' },
    })
    useChatStore.getState().handleChunk({
      type: 'tool_progress',
      tool_name: 'browse',
      tool_call_id: 'c1',
      progress: 'navigating to example.com',
      tab_id: 'tab-abc-123',
    })
    const last = useChatStore.getState().messages.at(-1)!
    // Progress must merge into the existing tool_call (not append a new one).
    const toolActivities = last.activities!.filter((a) => a.type === 'tool_call')
    expect(toolActivities).toHaveLength(1)
    expect(toolActivities[0]).toMatchObject({
      type: 'tool_call',
      toolName: 'browse',
      toolCallId: 'c1',
      progress: 'navigating to example.com',
      isRunning: true,
      tabId: 'tab-abc-123',
    })
    // Original toolArgs from tool_start are preserved across the merge.
    expect(toolActivities[0]!.toolArgs).toEqual({ url: 'https://example.com' })
  })

  it('tool_progress chunk without tab_id omits tabId on the activity', () => {
    // Legacy oxi-agent versions don't emit tab_id; the resulting activity
    // must not have tabId at all (not tabId: undefined), so the frontend
    // ActivityCard doesn't render a badge.
    useChatStore.getState().handleChunk({
      type: 'tool_start',
      tool_name: 'browse',
      tool_call_id: 'c1',
      tool_args: {},
    })
    useChatStore.getState().handleChunk({
      type: 'tool_progress',
      tool_name: 'browse',
      tool_call_id: 'c1',
      progress: 'step 1',
    })
    const last = useChatStore.getState().messages.at(-1)!
    const toolActivities = last.activities!.filter((a) => a.type === 'tool_call')
    expect(toolActivities).toHaveLength(1)
    expect(toolActivities[0]!.tabId).toBeUndefined()
    // Defensive: the key should not even be present on the object literal.
    expect('tabId' in toolActivities[0]!).toBe(false)
  })

  it('subsequent tool_progress replaces the prior progress text', () => {
    useChatStore.getState().handleChunk({
      type: 'tool_start',
      tool_name: 'browse',
      tool_call_id: 'c1',
      tool_args: {},
    })
    useChatStore.getState().handleChunk({
      type: 'tool_progress',
      tool_name: 'browse',
      tool_call_id: 'c1',
      progress: 'step 1',
    })
    useChatStore.getState().handleChunk({
      type: 'tool_progress',
      tool_name: 'browse',
      tool_call_id: 'c1',
      progress: 'step 2',
    })
    const last = useChatStore.getState().messages.at(-1)!
    const toolActivities = last.activities!.filter((a) => a.type === 'tool_call')
    expect(toolActivities).toHaveLength(1)
    expect(toolActivities[0]!.progress).toBe('step 2')
  })

  it('tool_end clears isRunning on the matching tool_call', () => {
    useChatStore.getState().handleChunk({
      type: 'tool_start',
      tool_name: 'browse',
      tool_call_id: 'c1',
      tool_args: {},
    })
    useChatStore.getState().handleChunk({
      type: 'tool_end',
      tool_name: 'browse',
      tool_call_id: 'c1',
      duration_ms: 100,
      is_error: false,
      output_summary: 'done',
    })
    const last = useChatStore.getState().messages.at(-1)!
    const toolActivities = last.activities!.filter((a) => a.type === 'tool_call')
    expect(toolActivities).toHaveLength(1)
    expect(toolActivities[0]).toMatchObject({
      type: 'tool_call',
      toolCallId: 'c1',
      isRunning: false,
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

  it('merges consecutive same-source reasoning fragments into one activity', () => {
    // Reasoning streams as per-token deltas; each delta must append to the
    // existing reasoning activity's content rather than create a new card
    // (the "one block per word" explosion the user reported).
    useChatStore.getState().handleChunk({ type: 'reasoning', content: 'Thi', source: 'thinking' })
    useChatStore.getState().handleChunk({ type: 'reasoning', content: 's is', source: 'thinking' })
    useChatStore
      .getState()
      .handleChunk({ type: 'reasoning', content: ' a thought', source: 'thinking' })
    const last = useChatStore.getState().messages.at(-1)!
    const reasoning = last.activities!.filter((a) => a.type === 'reasoning')
    expect(reasoning).toHaveLength(1)
    expect(reasoning[0]).toMatchObject({
      type: 'reasoning',
      content: 'This is a thought',
      reasoningSource: 'thinking',
    })
  })

  it('starts a new reasoning activity when the source changes', () => {
    useChatStore
      .getState()
      .handleChunk({ type: 'reasoning', content: 'thinking…', source: 'thinking' })
    useChatStore
      .getState()
      .handleChunk({ type: 'reasoning', content: 'compacting…', source: 'compaction' })
    const last = useChatStore.getState().messages.at(-1)!
    const reasoning = last.activities!.filter((a) => a.type === 'reasoning')
    expect(reasoning).toHaveLength(2)
    expect(reasoning[0]).toMatchObject({ content: 'thinking…', reasoningSource: 'thinking' })
    expect(reasoning[1]).toMatchObject({ content: 'compacting…', reasoningSource: 'compaction' })
  })

  it('token chunk does not add an activity', async () => {
    useChatStore.getState().handleChunk({ type: 'token', content: 'hello' })
    // F9: tokens are batched via requestAnimationFrame; wait one frame for flush.
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
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

  it('error chunk appends an error message with isError metadata and resets streaming', () => {
    // Pre-seed a user message so the error path can place the assistant
    // error after it (mirrors the production layout).
    useChatStore.setState({
      messages: [
        { id: 'u1', role: 'user' as const, content: 'hello', timestamp: new Date().toISOString() },
      ],
      isStreaming: true,
    })
    const errorChunk = {
      type: 'error',
      message: 'rate limit exceeded',
      kind: 'quota_exceeded',
      suggestion: 'try a different model',
    }
    useChatStore
      .getState()
      .handleChunk(
        errorChunk as unknown as Parameters<
          ReturnType<typeof useChatStore.getState>['handleChunk']
        >[0],
      )
    const state = useChatStore.getState()
    expect(state.isStreaming).toBe(false)
    const errMsg = state.messages.at(-1)!
    expect(errMsg.role).toBe('assistant')
    expect(errMsg.metadata?.isError).toBe(true)
    expect(errMsg.metadata?.errorKind).toBe('quota_exceeded')
    expect(errMsg.content).toContain('rate limit exceeded')
    expect(errMsg.content).toContain('try a different model')
  })

  it('removeMessage drops a single message by id and leaves siblings intact', () => {
    useChatStore.setState({
      messages: [
        { id: 'u1', role: 'user' as const, content: 'first', timestamp: new Date().toISOString() },
        {
          id: 'a1',
          role: 'assistant' as const,
          content: 'first reply',
          timestamp: new Date().toISOString(),
        },
        { id: 'u2', role: 'user' as const, content: 'second', timestamp: new Date().toISOString() },
      ],
      isStreaming: false,
    })
    useChatStore.getState().removeMessage('a1')
    const ids = useChatStore.getState().messages.map((m) => m.id)
    expect(ids).toEqual(['u1', 'u2'])
  })

  it('removeMessage resets isStreaming when removing the streaming target', () => {
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
    useChatStore.getState().removeMessage('a1')
    expect(useChatStore.getState().isStreaming).toBe(false)
    expect(useChatStore.getState().messages).toHaveLength(0)
  })
})

// mergeOrAppendActivity is the single pure helper both the chat store and the
// one-shot QuickAsk store route activity append/merge through. Testing it
// directly guards the Cmd+J path (which has no store-level tests) and prevents
// the two stores from drifting apart again.
describe('mergeOrAppendActivity (shared by chat + quick-ask stores)', () => {
  const act = (chunk: Parameters<typeof chunkToActivity>[0]) => chunkToActivity(chunk)!

  it('folds tool_start/progress/end into one tool_call by toolCallId', () => {
    const start = act({ type: 'tool_start', tool_name: 'bash', tool_call_id: 'c1', tool_args: {} })
    const progress = act({
      type: 'tool_progress',
      tool_name: 'bash',
      tool_call_id: 'c1',
      progress: 'halfway',
    })
    const end = act({
      type: 'tool_end',
      tool_name: 'bash',
      tool_call_id: 'c1',
      output_summary: 'ok',
      duration_ms: 9,
      is_error: false,
    })

    let activities = mergeOrAppendActivity([], start)
    activities = mergeOrAppendActivity(activities, progress)
    activities = mergeOrAppendActivity(activities, end)

    expect(activities.filter((a) => a.type === 'tool_call')).toHaveLength(1)
    expect(activities[0]).toMatchObject({
      type: 'tool_call',
      toolCallId: 'c1',
      progress: 'halfway',
      outputSummary: 'ok',
      durationMs: 9,
      isRunning: false,
    })
  })

  it('keeps distinct toolCallIds as separate activities', () => {
    const a = act({ type: 'tool_start', tool_name: 'read', tool_call_id: 'c1', tool_args: {} })
    const b = act({ type: 'tool_start', tool_name: 'write', tool_call_id: 'c2', tool_args: {} })
    expect(mergeOrAppendActivity([a], b)).toHaveLength(2)
  })

  it('concatenates consecutive same-source reasoning deltas', () => {
    const d1 = act({ type: 'reasoning', content: 'Thi', source: 'thinking' })
    const d2 = act({ type: 'reasoning', content: 's is', source: 'thinking' })
    const d3 = act({ type: 'reasoning', content: ' fine', source: 'thinking' })
    const activities = mergeOrAppendActivity(mergeOrAppendActivity([d1], d2), d3)
    expect(activities).toHaveLength(1)
    expect(activities[0]).toMatchObject({
      type: 'reasoning',
      content: 'This is fine',
      reasoningSource: 'thinking',
    })
  })

  it('starts a new reasoning activity when the source changes', () => {
    const a = act({ type: 'reasoning', content: 'thinking…', source: 'thinking' })
    const b = act({ type: 'reasoning', content: 'compacting…', source: 'compaction' })
    const activities = mergeOrAppendActivity([a], b)
    expect(activities).toHaveLength(2)
    expect(activities[1]).toMatchObject({ content: 'compacting…', reasoningSource: 'compaction' })
  })

  it('appends a tool_call after a reasoning span (no cross-type merge)', () => {
    const r = act({ type: 'reasoning', content: 'hmm', source: 'thinking' })
    const t = act({ type: 'tool_start', tool_name: 'bash', tool_call_id: 'c1', tool_args: {} })
    expect(mergeOrAppendActivity([r], t)).toHaveLength(2)
  })

  it('does not mutate the input array', () => {
    const a = act({ type: 'reasoning', content: 'x', source: 'thinking' })
    const b = act({ type: 'reasoning', content: 'y', source: 'thinking' })
    const input = [a]
    const out = mergeOrAppendActivity(input, b)
    expect(input).toHaveLength(1)
    expect(input[0]).toBe(a)
    expect(out).not.toBe(input)
  })
})

// The message-transform primitives route every chunk through a single shared
// path in chat.ts; both the chat store and the quick-ask store call them, so
// these tests guard the Cmd+J (one-shot) path too (which has no store tests).
describe('message-transform primitives (shared by chat + quick-ask stores)', () => {
  const ctx = { placeholderModel: 'gpt-x' }
  const assistant = (over: Partial<ChatMessage> = {}): ChatMessage => ({
    id: 'a1',
    role: 'assistant',
    content: '',
    timestamp: 't',
    ...over,
  })
  const userMsg = (): ChatMessage => ({ id: 'u1', role: 'user', content: 'hi', timestamp: 't' })

  it('ensureLastAssistant returns the same array when last is already assistant', () => {
    const input = [assistant()]
    const { messages, index } = ensureLastAssistant(input, ctx)
    expect(messages).toBe(input)
    expect(index).toBe(0)
  })

  it('ensureLastAssistant appends a ctx-modelled placeholder when last is not assistant', () => {
    const { messages, index } = ensureLastAssistant([userMsg()], ctx)
    expect(messages).toHaveLength(2)
    expect(messages[1]).toMatchObject({ role: 'assistant', content: '', model: 'gpt-x' })
    expect(index).toBe(1)
  })

  it('appendTokenToMessages appends to the last assistant content', () => {
    const out = appendTokenToMessages([assistant({ content: 'foo' })], 'bar', ctx)
    expect(out[0]!.content).toBe('foobar')
  })

  it('appendTokenToMessages creates a placeholder when no assistant exists', () => {
    const out = appendTokenToMessages([userMsg()], 'hi', ctx)
    expect(out).toHaveLength(2)
    expect(out[1]).toMatchObject({ role: 'assistant', content: 'hi', model: 'gpt-x' })
  })

  it('appendTokenToMessages is a no-op returning the same array on empty content', () => {
    const input = [assistant({ content: 'foo' })]
    expect(appendTokenToMessages(input, '', ctx)).toBe(input)
  })

  it('appendActivityToMessages merges the activity and accumulates token counts', () => {
    const usage = chunkToActivity({ type: 'usage', input_tokens: 10, output_tokens: 5 })!
    const out = appendActivityToMessages([assistant({ content: 'x' })], usage, ctx)
    expect(out[0]!.activities).toHaveLength(1)
    expect(out[0]!.totalInputTokens).toBe(10)
    expect(out[0]!.totalOutputTokens).toBe(5)
  })

  it('appendActivityToMessages creates a placeholder when no assistant exists', () => {
    const phase = chunkToActivity({ type: 'phase', phase: 'assess', status: 'started', summary: '' })!
    const out = appendActivityToMessages([userMsg()], phase, ctx)
    expect(out).toHaveLength(2)
    expect(out[1]!.activities).toHaveLength(1)
  })

  it('patchAssistantModel patches the last assistant model', () => {
    const out = patchAssistantModel([assistant({ model: 'old' })], 'new')
    expect(out.messages[0]!.model).toBe('new')
    expect(out.pendingModel).toBeUndefined()
  })

  it('patchAssistantModel returns pendingModel when no assistant exists', () => {
    const input = [userMsg()]
    const out = patchAssistantModel(input, 'm')
    expect(out.messages).toBe(input)
    expect(out.pendingModel).toBe('m')
  })
})

describe('useChatStore message queueing (while streaming)', () => {
  let sendSpy: Mock
  const mockWs = (): WebSocket =>
    ({ readyState: 1, send: sendSpy, close: vi.fn() }) as unknown as WebSocket

  beforeEach(() => {
    localStorage.clear()
    sendSpy = vi.fn()
    useChatStore.setState({
      messages: [
        { id: 'a1', role: 'assistant' as const, content: '', timestamp: new Date().toISOString() },
      ],
      isStreaming: true,
      connected: true,
      _ws: mockWs(),
      _pendingQueue: [],
      _reconnectTimer: null,
      _pingTimer: null,
      activeSessionId: 's1',
    })
  })

  it('queues a message sent while streaming instead of dispatching', () => {
    useChatStore.getState().sendMessage('follow-up')
    const s = useChatStore.getState()
    // Stashed in the pending queue — not yet on the wire or in the list.
    expect(s._pendingQueue).toEqual(['follow-up'])
    expect(s.messages.some((m) => m.role === 'user')).toBe(false)
    expect(sendSpy).not.toHaveBeenCalled()
  })

  it('drains the queue in order when the turn completes (done)', () => {
    useChatStore.getState().sendMessage('first')
    useChatStore.getState().sendMessage('second')
    // Turn ends → drain dispatches 'first', leaves 'second' queued.
    useChatStore.getState().handleChunk({ type: 'done', session_id: 's1', phase: 'execute' })
    const s = useChatStore.getState()
    expect(s._pendingQueue).toEqual(['second'])
    expect(s.isStreaming).toBe(true)
    expect(s.messages.some((m) => m.role === 'user' && m.content === 'first')).toBe(true)
    expect(sendSpy).toHaveBeenCalledTimes(1)
    expect(JSON.parse(sendSpy.mock.calls[0]![0] as string).content).toBe('first')
  })

  it('clears the queue on disconnect (cancel drops unsent messages)', () => {
    useChatStore.getState().sendMessage('ghost')
    useChatStore.getState().disconnect()
    expect(useChatStore.getState()._pendingQueue).toEqual([])
  })
})
