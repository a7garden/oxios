// Component test: LiveActivityBar visibility, phase priority, and timer.
// The bar reads from useChatStore — we drive those selectors directly via
// store.setState() so we exercise the real production component without
// spinning up the full chat pipeline.

import { render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { LiveActivityBar } from '@/components/chat/live-activity-bar'
import type { ChatMessage } from '@/types'
import type { ChatBlock, ReasoningBlock, ToolBlock } from '@/types/chat'
import { useChatStore } from '@/stores/chat'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (opts && 'name' in opts) return `${key}:${String(opts['name'])}`
      return key
    },
    i18n: { language: 'en' },
  }),
}))

const NOW = Date.now()

/** Build a single trailing assistant message carrying the given blocks. */
function assistantWithBlocks(
  blocks: ChatBlock[] | undefined,
  overrides: Partial<ChatMessage> = {},
): ChatMessage {
  return {
    id: 'm1',
    role: 'assistant',
    content: '',
    timestamp: new Date(NOW).toISOString(),
    blocks,
    ...overrides,
  }
}

function loadingTool(args: Partial<ToolBlock> = {}): ToolBlock {
  return {
    type: 'tool',
    id: 't1',
    identifier: 'kernel',
    apiName: 'read',
    arguments: { path: 'src/main.ts' },
    status: 'loading',
    ...args,
  } as ToolBlock
}

function streamingReasoning(): ReasoningBlock {
  return {
    type: 'reasoning',
    id: 'r1',
    text: 'thinking…',
    status: 'streaming',
    source: 'thinking',
    startedAt: 0,
  }
}

function resetStore() {
  useChatStore.setState({
    messages: [],
    isStreaming: false,
    streamStartedAt: null,
    activeInterview: null,
    activeToolApproval: null,
    activePathAccess: null,
  })
}

describe('LiveActivityBar', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date(NOW))
    resetStore()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('renders nothing when the chat is not streaming', () => {
    useChatStore.setState({
      isStreaming: false,
      streamStartedAt: null,
      messages: [assistantWithBlocks([loadingTool()])],
    })
    const { container } = render(<LiveActivityBar />)
    expect(container.firstChild).toBeNull()
  })

  it('renders nothing while an interview is active (modal takes priority)', () => {
    useChatStore.setState({
      isStreaming: true,
      streamStartedAt: NOW,
      activeInterview: [{ id: 'q1', text: '?', kind: 'free_text' } as never],
      messages: [assistantWithBlocks([loadingTool()])],
    })
    const { container } = render(<LiveActivityBar />)
    expect(container.firstChild).toBeNull()
  })

  it('renders nothing while a tool approval is pending', () => {
    useChatStore.setState({
      isStreaming: true,
      streamStartedAt: NOW,
      activeToolApproval: { id: 'a1' } as never,
      messages: [assistantWithBlocks([loadingTool()])],
    })
    const { container } = render(<LiveActivityBar />)
    expect(container.firstChild).toBeNull()
  })

  it('renders nothing while a path-access prompt is pending', () => {
    useChatStore.setState({
      isStreaming: true,
      streamStartedAt: NOW,
      activePathAccess: { id: 'p1' } as never,
      messages: [assistantWithBlocks([loadingTool()])],
    })
    const { container } = render(<LiveActivityBar />)
    expect(container.firstChild).toBeNull()
  })

  it('shows the thinking label when streaming with no blocks yet', () => {
    useChatStore.setState({
      isStreaming: true,
      streamStartedAt: NOW,
      messages: [assistantWithBlocks(undefined)],
    })
    render(<LiveActivityBar />)
    expect(screen.getByText('chat.liveActivity.thinking')).toBeTruthy()
  })

  it('shows the writing label when the assistant has streaming text but no other activity', () => {
    useChatStore.setState({
      isStreaming: true,
      streamStartedAt: NOW,
      messages: [
        assistantWithBlocks(undefined, {
          content: 'partial answer…',
          generating: true,
        }),
      ],
    })
    render(<LiveActivityBar />)
    expect(screen.getByText('chat.liveActivity.writing')).toBeTruthy()
  })

  it('prioritizes a running tool over streaming text', () => {
    // Both a loading tool block AND streaming text exist. Per the production
    // priority order, tool_running wins — the bar shows the tool label.
    useChatStore.setState({
      isStreaming: true,
      streamStartedAt: NOW,
      messages: [
        assistantWithBlocks([loadingTool({ apiName: 'browser' })], {
          content: 'streaming…',
          generating: true,
        }),
      ],
    })
    render(<LiveActivityBar />)
    // toolVerb maps browser → chat.liveActivity.browser
    expect(screen.getByText('chat.liveActivity.browser')).toBeTruthy()
    // Detail is the shortened path from the tool's arguments.
    // Detail is the `path` argument; production's shortenPath keeps the last
    // two segments (short inputs are returned as-is), so for "src/main.ts"
    // the detail is "src/main.ts" itself.
    expect(screen.getByText('src/main.ts')).toBeTruthy()
  })

  it('prioritizes a running tool over a streaming reasoning block', () => {
    useChatStore.setState({
      isStreaming: true,
      streamStartedAt: NOW,
      messages: [
        assistantWithBlocks([streamingReasoning(), loadingTool({ apiName: 'exec' })]),
      ],
    })
    render(<LiveActivityBar />)
    // toolVerb maps exec → chat.liveActivity.exec
    expect(screen.getByText('chat.liveActivity.exec')).toBeTruthy()
    // Reasoning label must NOT be present — tool wins.
    expect(screen.queryByText('chat.liveActivity.reasoning')).toBeNull()
  })

  it('shows the reasoning label when the most recent live block is reasoning', () => {
    useChatStore.setState({
      isStreaming: true,
      streamStartedAt: NOW,
      messages: [assistantWithBlocks([streamingReasoning()])],
    })
    render(<LiveActivityBar />)
    expect(screen.getByText('chat.liveActivity.reasoning')).toBeTruthy()
  })

  it('ignores the user role and only looks at the trailing assistant message', () => {
    // user message at the tail — must NOT count. Falls back to thinking.
    useChatStore.setState({
      isStreaming: true,
      streamStartedAt: NOW,
      messages: [
        assistantWithBlocks([loadingTool()]),
        {
          id: 'm2',
          role: 'user',
          content: 'follow up',
          timestamp: new Date(NOW).toISOString(),
        },
      ],
    })
    render(<LiveActivityBar />)
    expect(screen.getByText('chat.liveActivity.thinking')).toBeTruthy()
    expect(screen.queryByText('chat.liveActivity.read')).toBeNull()
  })

  it('hides the elapsed timer before the 2s threshold', () => {
    useChatStore.setState({
      isStreaming: true,
      streamStartedAt: NOW,
      messages: [assistantWithBlocks([loadingTool()])],
    })
    render(<LiveActivityBar />)
    // First tick fires synchronously in the effect — elapsedMs starts at 0.
    expect(screen.queryByText(/^\d+s$/)).toBeNull()
  })

  it('renders the elapsed timer once 2s have elapsed (deterministic via fake timers)', () => {
    useChatStore.setState({
      isStreaming: true,
      streamStartedAt: NOW - 5_000, // already 5s in
      messages: [assistantWithBlocks([loadingTool()])],
    })
    render(<LiveActivityBar />)
    // The effect's initial tick recomputes against the new "now", so the
    // 5s gap shows immediately.
    expect(screen.getByText('5s')).toBeTruthy()
  })

  it('formats minutes + seconds once elapsed crosses 60s', () => {
    useChatStore.setState({
      isStreaming: true,
      streamStartedAt: NOW - 125_000, // 2m 5s
      messages: [assistantWithBlocks(undefined)],
    })
    render(<LiveActivityBar />)
    expect(screen.getByText('2m 5s')).toBeTruthy()
  })

  it('clears the elapsed timer when isStreaming goes false', () => {
    useChatStore.setState({
      isStreaming: true,
      streamStartedAt: NOW - 5_000,
      messages: [assistantWithBlocks(undefined)],
    })
    const { rerender } = render(<LiveActivityBar />)
    expect(screen.getByText('5s')).toBeTruthy()

    // Stop the stream and re-render — the bar unmounts (returns null).
    useChatStore.setState({ isStreaming: false })
    rerender(<LiveActivityBar />)
    expect(screen.queryByText('5s')).toBeNull()
  })
})