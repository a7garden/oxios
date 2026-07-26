import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest'
import { useAuthStore } from '@/stores/auth'
import {
  __clearAuthCacheForTesting,
  __clearResolvedApprovalIdsForTesting,
  __clearStreamProcessorsForTesting,
  appendActivityToMessages,
  appendTokenToMessages,
  chunkToActivity,
  ensureLastAssistant,
  finalizeStreamingMessage,
  mergeOrAppendActivity,
  patchAssistantModel,
  useChatStore,
} from '@/stores/chat'
import { useSidebarStore } from '@/stores/sidebar'
import type { ChatMessage } from '@/types'

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
    // Phase 1: StreamProcessor instances are module-level and persist across
    // tests. Reset them so accumulation doesn't leak between tests.
    __clearStreamProcessorsForTesting()
    // Start each test with a single empty assistant message so chunks
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

  it('tool_call_delta accumulates partial args before tool_start (oxi 0.58+)', () => {
    // The LLM streams raw JSON fragments for a tool call it is still
    // constructing. Two deltas for the same tool_call_id must concatenate.
    useChatStore.getState().handleChunk({
      type: 'tool_call_delta',
      tool_call_id: 'c1',
      args_delta: '{"path": "/tm',
    })
    useChatStore.getState().handleChunk({
      type: 'tool_call_delta',
      tool_call_id: 'c1',
      args_delta: 'p/foo"}',
    })
    const last = useChatStore.getState().messages.at(-1)!
    // A placeholder toolCall exists with accumulated raw-JSON args.
    expect(last.toolCalls).toHaveLength(1)
    expect(last.toolCalls![0]).toMatchObject({
      id: 'c1',
      apiName: '(constructing…)',
    })
    expect(last.toolCalls![0]!.arguments).toBe('{"path": "/tmp/foo"}')
    // No activity yet — tool.start hasn't arrived.
    expect(last.activities ?? []).toHaveLength(0)
  })

  it('tool_call_delta placeholder is replaced by tool_start', () => {
    useChatStore.getState().handleChunk({
      type: 'tool_call_delta',
      tool_call_id: 'c1',
      args_delta: '{"q":"hi"}',
    })
    useChatStore.getState().handleChunk({
      type: 'tool_start',
      tool_name: 'web_search',
      tool_call_id: 'c1',
      tool_args: { q: 'hi' },
    })
    const last = useChatStore.getState().messages.at(-1)!
    // The placeholder is replaced by the real tool (parsed args + name).
    expect(last.toolCalls).toHaveLength(1)
    expect(last.toolCalls![0]).toMatchObject({
      id: 'c1',
      apiName: 'web_search',
    })
    expect(last.toolCalls![0]!.arguments).toEqual({ q: 'hi' })
    // tool.start creates the activity.
    expect(last.activities).toHaveLength(1)
    expect(last.activities![0]).toMatchObject({
      type: 'tool_call',
      toolName: 'web_search',
    })
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

  it('reasoning populates first-class reasoning field (Phase 1)', () => {
    // Phase 1 refactor (2026-07-21): reasoning is now a first-class field on
    // ChatMessage, no longer an activity entry. See
    // docs/designs/2026-07-21-lobehub-chat-port-design.md §6.1.
    useChatStore.getState().handleChunk({
      type: 'reasoning',
      content: 'compaction complete',
      source: 'compaction',
    })
    const last = useChatStore.getState().messages.at(-1)!
    expect(last.reasoning).toMatchObject({
      content: 'compaction complete',
      thinking: true,
    })
    expect(last.isReasoning).toBe(true)
  })

  it('accumulates reasoning deltas into single content string (Phase 1)', () => {
    // Phase 1: reasoning streams as per-token deltas that accumulate into
    // message.reasoning.content. Source-based grouping from the old activity
    // model is intentionally dropped — Phase 1 has one reasoning span per
    // message; multi-source reasoning is a Phase 2+ concern.
    useChatStore.getState().handleChunk({ type: 'reasoning', content: 'Thi', source: 'thinking' })
    useChatStore.getState().handleChunk({ type: 'reasoning', content: 's is', source: 'thinking' })
    useChatStore
      .getState()
      .handleChunk({ type: 'reasoning', content: ' a thought', source: 'thinking' })
    const last = useChatStore.getState().messages.at(-1)!
    expect(last.reasoning?.content).toBe('This is a thought')
    expect(last.reasoning?.thinking).toBe(true)
  })

  it('reasoning across sources still accumulates into one field (Phase 1)', () => {
    // Phase 1 simplification: source-based grouping is gone. All reasoning
    // chunks accumulate into message.reasoning.content regardless of source.
    // Multi-span reasoning UI is a Phase 2+ concern.
    useChatStore
      .getState()
      .handleChunk({ type: 'reasoning', content: 'thinking…', source: 'thinking' })
    useChatStore
      .getState()
      .handleChunk({ type: 'reasoning', content: 'compacting…', source: 'compaction' })
    const last = useChatStore.getState().messages.at(-1)!
    expect(last.reasoning?.content).toBe('thinking…compacting…')
  })
  it('tool_start closes an open reasoning span (reasoning_content models)', () => {
    // Reasoning_content providers (GLM/DeepSeek/Qwen) go straight from
    // reasoning to a tool call with no Text, so the gateway's first-Text
    // reasoning.end heuristic never fires. tool_start must close the
    // reasoning span itself or the Thinking spinner runs through the whole
    // tool execution. See StreamProcessor.closeReasoningIfOpen.
    const store = useChatStore.getState()
    store.handleChunk({ type: 'reasoning', content: 'Let me try a search. ' })
    store.handleChunk({ type: 'reasoning', content: 'Querying…' })
    expect(useChatStore.getState().messages.at(-1)!.isReasoning).toBe(true)
    store.handleChunk({
      type: 'tool_start',
      tool_name: 'web_search',
      tool_call_id: 'ws-1',
      tool_args: { q: 'weather' },
    })
    const last = useChatStore.getState().messages.at(-1)!
    // Reasoning closed by the tool transition — before any `done`.
    expect(last.isReasoning).toBe(false)
    expect(last.reasoning?.thinking).toBe(false)
    expect(last.reasoning?.content).toBe('Let me try a search. Querying…')
    expect(last.toolCalls).toHaveLength(1)
  })

  it('token chunk does not add an activity', async () => {
    // F9: tokens are batched via requestAnimationFrame; wait one frame for flush.
    useChatStore.getState().handleChunk({ type: 'token', content: 'hello' })
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
    const last = useChatStore.getState().messages.at(-1)!
    expect(last.content).toBe('hello')
    expect(last.activities ?? []).toEqual([])
  })

  it('mixed stream populates all first-class fields end-to-end (Phase 1+2 contract)', async () => {
    // Simulates a realistic stream: model announcement → reasoning deltas →
    // tool call (start/progress/end) → text tokens → usage → done.
    // Asserts the final ChatMessage shape matches the design's pipeline contract
    // (docs/designs/2026-07-21-lobehub-chat-port-design.md §6.1, §7).
    const store = useChatStore.getState()
    store.handleChunk({ type: 'model', model: 'zai/glm-5.2' })
    store.handleChunk({ type: 'reasoning', content: 'Thinking ' })
    store.handleChunk({ type: 'reasoning', content: 'about it…' })
    store.handleChunk({
      type: 'tool_start',
      tool_name: 'read_file',
      tool_call_id: 'tc-1',
      tool_args: { path: '/etc/hosts' },
    })
    store.handleChunk({
      type: 'tool_progress',
      tool_call_id: 'tc-1',
      progress: 'reading…',
    })
    store.handleChunk({
      type: 'tool_end',
      tool_call_id: 'tc-1',
      duration_ms: 42,
      output_summary: '127.0.0.1 localhost',
    })
    store.handleChunk({ type: 'token', content: 'Hello ' })
    store.handleChunk({ type: 'token', content: 'world' })
    // Allow RAF token flush.
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
    store.handleChunk({ type: 'usage', input_tokens: 12, output_tokens: 7 })
    store.handleChunk({ type: 'done', phase: 'Execute', evaluation_passed: true })

    const last = useChatStore.getState().messages.at(-1)!
    // First-class reasoning field — content accumulated, not in activities.
    expect(last.reasoning?.content).toBe('Thinking about it…')
    expect(last.isReasoning).toBe(false) // done cleared it
    // Structured toolCalls[] with full lifecycle.
    expect(last.toolCalls).toHaveLength(1)
    expect(last.toolCalls![0]).toMatchObject({
      apiName: 'read_file',
      status: 'success',
      durationMs: 42,
    })
    // Token stream accumulated into content.
    expect(last.content).toBe('Hello world')
    // Token totals carried through.
    expect(last.totalInputTokens).toBe(12)
    expect(last.totalOutputTokens).toBe(7)
    // Model badge stamped.
    expect(last.model).toBe('zai/glm-5.2')
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
    // Use tool_start — a chunk type that chunkToActivity actually maps to
    // a ChatActivity. (Previously used `phase`, but that case was never
    // wired up in chunkToActivity, so the conversion returned null and the
    // non-null assertion (`!`) hid the bug.)
    const toolStart = chunkToActivity({
      type: 'tool_start',
      tool_name: 'bash',
      tool_call_id: 'call-1',
      tool_args: { cmd: 'ls' },
    })!
    const out = appendActivityToMessages([userMsg()], toolStart, ctx)
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

  it('finalizeStreamingMessage drops an empty generating placeholder', () => {
    const msgs: ChatMessage[] = [userMsg(), assistant({ id: 'a1', generating: true })]
    // No content/reasoning/tools/activities → ghost → removed.
    expect(finalizeStreamingMessage(msgs)).toEqual([userMsg()])
  })

  it('finalizeStreamingMessage keeps a partial placeholder with generating cleared', () => {
    const msgs: ChatMessage[] = [
      userMsg(),
      assistant({ id: 'a1', content: 'partial answer', generating: true }),
    ]
    const out = finalizeStreamingMessage(msgs)
    expect(out).toHaveLength(2)
    expect(out[1]!.content).toBe('partial answer')
    expect(out[1]!.generating).toBe(false)
  })

  it('finalizeStreamingMessage is a no-op when the last message is not a generating assistant', () => {
    const done = [assistant({ id: 'a1', content: 'finished' })]
    expect(finalizeStreamingMessage(done)).toBe(done)
    const onlyUser = [userMsg()]
    expect(finalizeStreamingMessage(onlyUser)).toBe(onlyUser)
  })
  it('finalizeStreamingMessage clears isReasoning on an interrupted reasoning stream', () => {
    // Abnormal WS close mid-reasoning must not leave the Thinking spinner
    // stuck. Without clearing isReasoning/reasoning.thinking the block spins
    // forever — the reported "stuck Thinking..." symptom.
    const msgs: ChatMessage[] = [
      userMsg(),
      assistant({
        id: 'a1',
        generating: true,
        isReasoning: true,
        isToolCallGenerating: true,
        reasoning: { content: 'Let me try', duration: 100, thinking: true },
      }),
    ]
    const out = finalizeStreamingMessage(msgs)
    expect(out).toHaveLength(2)
    expect(out[1]!.generating).toBe(false)
    expect(out[1]!.isReasoning).toBe(false)
    expect(out[1]!.isToolCallGenerating).toBe(false)
    expect(out[1]!.reasoning?.thinking).toBe(false)
    // Reasoning content is preserved for display.
    expect(out[1]!.reasoning?.content).toBe('Let me try')
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

// sendMessage turn start (lobehub-style holder model): no optimistic
// assistant placeholder is created — the LiveActivityBar holder pinned above
// the input owns the "what's happening" indicator for the whole turn,
// including the pre-chunk gap. The assistant message is created lazily on the
// first reasoning/tool/token chunk. sendMessage just stamps the user message,
// isStreaming, and a streamStartedAt timestamp the holder reads for elapsed.
describe('useChatStore sendMessage turn start', () => {
  let sendSpy: Mock
  const mockWs = (): WebSocket =>
    ({ readyState: 1, send: sendSpy, close: vi.fn() }) as unknown as WebSocket

  beforeEach(() => {
    localStorage.clear()
    __clearStreamProcessorsForTesting()
    sendSpy = vi.fn()
    useChatStore.setState({
      messages: [],
      isStreaming: false,
      streamStartedAt: null,
      connected: true,
      _ws: mockWs(),
      _pendingQueue: [],
      _reconnectTimer: null,
      _pingTimer: null,
      activeSessionId: 's1',
      activeModelId: 'gpt-test',
    })
  })

  it('appends only the user message and marks the turn live with a start timestamp', () => {
    useChatStore.getState().sendMessage('hello')
    const s = useChatStore.getState()
    expect(s.isStreaming).toBe(true)
    expect(s.streamStartedAt).toBeTypeOf('number')
    // No optimistic assistant placeholder — the holder covers the gap.
    expect(s.messages).toHaveLength(1)
    expect(s.messages[0]).toMatchObject({ role: 'user', content: 'hello' })
    expect(sendSpy).toHaveBeenCalledTimes(1)
  })

  it('creates the assistant message lazily on the first reasoning chunk', () => {
    useChatStore.getState().sendMessage('hello')
    expect(useChatStore.getState().messages).toHaveLength(1)
    // A reasoning model streams reasoning before any token; the chunk handler
    // must create the assistant message on demand (not drop the delta).
    useChatStore.getState().handleChunk({
      type: 'reasoning',
      content: 'thinking…',
      source: 'thinking',
    })
    const msgs = useChatStore.getState().messages
    expect(msgs).toHaveLength(2)
    expect(msgs[1]).toMatchObject({ role: 'assistant' })
    expect(msgs[1]!.reasoning?.content).toBe('thinking…')
  })

  it('disconnect before any chunk leaves only the user message (no ghost bubble)', () => {
    useChatStore.getState().sendMessage('hello')
    useChatStore.getState().disconnect()
    const msgs = useChatStore.getState().messages
    expect(msgs).toHaveLength(1)
    expect(msgs[0]!.role).toBe('user')
    expect(useChatStore.getState().isStreaming).toBe(false)
  })
})

// Tool approval (RFC-017): resolveToolApproval must treat a 404 "already
// resolved" as benign idempotent success and NEVER re-arm a dead card. The
// prior code restored activeToolApproval on ANY non-OK and re-threw, so a
// single 404 re-armed the card → the next click 404ed again → the N×404
// cascade seen in the Web UI. A WS reconnect replay re-delivering a resolved
// approval id was the trigger; the restore-on-error was the amplifier.
describe('useChatStore tool approval (RFC-017)', () => {
  let fetchSpy: Mock

  beforeEach(() => {
    localStorage.clear()
    __clearStreamProcessorsForTesting()
    __clearResolvedApprovalIdsForTesting()
    fetchSpy = vi.fn()
    vi.stubGlobal('fetch', fetchSpy)
    useChatStore.setState({
      messages: [],
      activeToolApproval: { id: 'approval-1', toolName: 'exec', reason: 'shell command' },
      isStreaming: false,
    })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('dismisses the card on 200 success', async () => {
    fetchSpy.mockResolvedValueOnce({ ok: true, status: 200 })
    await useChatStore.getState().resolveToolApproval('approval-1', true)
    const s = useChatStore.getState()
    expect(s.activeToolApproval).toBeNull()
    expect(s.isStreaming).toBe(true)
  })

  it('treats 404 "already resolved" as benign — card stays dismissed, no throw', async () => {
    fetchSpy.mockResolvedValueOnce({
      ok: false,
      status: 404,
      text: () =>
        Promise.resolve('{"error":"tool approval approval-1 not found or already resolved"}'),
    })
    // Must not reject — a 404 is idempotent success (approval already gone).
    await expect(
      useChatStore.getState().resolveToolApproval('approval-1', true),
    ).resolves.toBeUndefined()
    const s = useChatStore.getState()
    // Card NOT restored — the old code set it back here, re-arming the loop.
    expect(s.activeToolApproval).toBeNull()
    expect(fetchSpy).toHaveBeenCalledTimes(1)
  })

  it('restores the card on a genuine server error (500) for retry, without throwing', async () => {
    fetchSpy.mockResolvedValueOnce({
      ok: false,
      status: 500,
      text: () => Promise.resolve('internal error'),
    })
    // Swallowed — no unhandled promise rejection.
    await expect(
      useChatStore.getState().resolveToolApproval('approval-1', true),
    ).resolves.toBeUndefined()
    const s = useChatStore.getState()
    expect(s.activeToolApproval?.id).toBe('approval-1')
    expect(s.isStreaming).toBe(false)
  })

  it('does not re-arm an already-resolved approval on WS replay', async () => {
    fetchSpy.mockResolvedValueOnce({ ok: true, status: 200 })
    await useChatStore.getState().resolveToolApproval('approval-1', true)
    expect(useChatStore.getState().activeToolApproval).toBeNull()
    // Reconnect replay re-delivers the same approval id.
    useChatStore.getState().handleChunk({
      type: 'tool_approval',
      id: 'approval-1',
      tool_name: 'exec',
      reason: 'shell command',
    })
    // Still dismissed — the dead card is NOT re-armed.
    expect(useChatStore.getState().activeToolApproval).toBeNull()
  })

  it('arms the card for a fresh tool_approval chunk', () => {
    useChatStore.setState({ activeToolApproval: null })
    useChatStore.getState().handleChunk({
      type: 'tool_approval',
      id: 'approval-2',
      tool_name: 'exec',
      reason: 'rm -rf',
    })
    const s = useChatStore.getState()
    expect(s.activeToolApproval?.id).toBe('approval-2')
    expect(s.isStreaming).toBe(false)
  })
})

// Reconnect state machine: the client must never permanently give up. After
// the fast exponential backoff exhausts it switches to a steady long-tail
// retry so a daemon restart that outlasts the ~31s backoff window still
// recovers instead of stranding the tab at connected === false forever.
describe('useChatStore reconnect (long-tail recovery)', () => {
  // Minimal fake WebSocket. The outcome is deferred via setTimeout(0) so fake
  // timers drive ordering deterministically and connect() can attach its
  // handlers before open/close fires. Statics are declared on the class body
  // (writable) because the DOM lib types `WebSocket.OPEN` &c. as readonly,
  // and the global is installed via vi.stubGlobal because jsdom exposes
  // `WebSocket` as a read-only property.
  let instances: unknown[]
  let FakeWS: ReturnType<typeof makeFakeWebSocket>

  function makeFakeWebSocket() {
    class FakeWebSocket {
      static OPEN = 1
      static CLOSED = 3
      static CONNECTING = 0
      static CLOSING = 2
      static mode: 'fail' | 'succeed' = 'fail'
      url: string
      readyState = 0
      onopen: (() => void) | null = null
      onclose: (() => void) | null = null
      onmessage: ((e: { data: string }) => void) | null = null
      onerror: (() => void) | null = null
      constructor(url: string) {
        this.url = url
        instances.push(this)
        setTimeout(() => {
          if (FakeWebSocket.mode === 'succeed') {
            this.readyState = 1
            this.onopen?.()
          } else {
            this.readyState = 3
            this.onclose?.()
          }
        }, 0)
      }
      close() {
        this.readyState = 3
      }
      send() {}
    }
    return FakeWebSocket
  }

  beforeEach(() => {
    localStorage.clear()
    sessionStorage.clear()
    __clearAuthCacheForTesting()
    __clearStreamProcessorsForTesting()
    __clearResolvedApprovalIdsForTesting()
    instances = []
    FakeWS = makeFakeWebSocket()
    vi.stubGlobal('WebSocket', FakeWS)
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: true,
        status: 200,
        json: async () => ({ auth_enabled: false }),
      })),
    )
    useChatStore.setState({
      connected: false,
      isStreaming: false,
      _ws: null,
      _sendQueue: [],
      _pendingQueue: [],
      _reconnectAttempts: 0,
      _reconnectTimer: null,
      _pingTimer: null,
      messages: [],
      activeSessionId: 's1',
      activeProjectId: 'p1',
    })
    vi.useFakeTimers()
  })

  afterEach(() => {
    try {
      useChatStore.getState().disconnect()
    } catch {
      // ignore — store may be mid-teardown
    }
    vi.useRealTimers()
    vi.unstubAllGlobals()
  })

  it('keeps retrying past the fast-backoff cap instead of giving up forever', async () => {
    useChatStore.getState().connect()
    // Fast backoff is 1+2+4+8+16 = 31s across 5 attempts. Advancing 60s
    // runs the full backoff plus two long-tail (10s) retries.
    await vi.advanceTimersByTimeAsync(60_000)
    // Old behaviour stopped creating sockets at the cap (6 total). Long-tail
    // recovery must keep creating fresh sockets.
    expect(instances.length).toBeGreaterThanOrEqual(7)
    expect(useChatStore.getState()._reconnectAttempts).toBeGreaterThanOrEqual(5)
    expect(useChatStore.getState().connected).toBe(false)
  })

  it('recovers and resets the counter once the connection succeeds', async () => {
    useChatStore.getState().connect()
    await vi.advanceTimersByTimeAsync(60_000)
    expect(useChatStore.getState().connected).toBe(false)
    // Daemon returns — the next long-tail attempt opens.
    FakeWS.mode = 'succeed'
    await vi.advanceTimersByTimeAsync(15_000)
    expect(useChatStore.getState().connected).toBe(true)
    expect(useChatStore.getState()._reconnectAttempts).toBe(0)
  })
})
